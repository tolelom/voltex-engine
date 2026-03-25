use wgpu::util::DeviceExt;

use crate::draw_list::{DrawList, DrawVertex};
use crate::font::FontAtlas;

/// Vertex buffer layout for DrawVertex: position(Float32x2), uv(Float32x2), color(Unorm8x4).
const VERTEX_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<DrawVertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &[
        // position: vec2<f32>
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x2,
        },
        // uv: vec2<f32>
        wgpu::VertexAttribute {
            offset: 8,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
        // color: vec4<f32> (from [u8;4] via Unorm8x4)
        wgpu::VertexAttribute {
            offset: 16,
            shader_location: 2,
            format: wgpu::VertexFormat::Unorm8x4,
        },
    ],
};

/// GPU-side UI renderer that turns a DrawList into a rendered frame.
pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    bind_group_layout: wgpu::BindGroupLayout,
    font_bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    projection: [f32; 16],
    last_screen_w: f32,
    last_screen_h: f32,
}

impl UiRenderer {
    /// Create a new UI renderer.
    ///
    /// `font` is used to build the font atlas GPU texture. Pixel (0,0) must be 255
    /// so solid-color rects can sample UV (0,0) for white.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        font: &FontAtlas,
    ) -> Self {
        // --- Shader ---
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui_shader.wgsl").into()),
        });

        // --- Bind group layout: uniform + texture + sampler ---
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI Bind Group Layout"),
            entries: &[
                // @binding(0): uniform buffer (projection mat4x4)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // @binding(1): texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // @binding(2): sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // --- Pipeline layout ---
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        // --- Render pipeline: alpha blend, no depth ---
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[VERTEX_LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // --- Font atlas GPU texture (R8Unorm) ---
        let atlas_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("UI Font Atlas"),
                size: wgpu::Extent3d {
                    width: font.width,
                    height: font.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &font.pixels,
        );
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("UI Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // --- Uniform buffer (projection) ---
        let projection = ortho_projection(800.0, 600.0);
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI Uniform Buffer"),
            contents: bytemuck::cast_slice(&projection),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // --- Bind group ---
        let font_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Font Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        UiRenderer {
            pipeline,
            bind_group_layout: bind_group_layout,
            font_bind_group,
            uniform_buffer,
            projection,
            last_screen_w: 800.0,
            last_screen_h: 600.0,
        }
    }

    /// Render the draw list onto the given target view.
    ///
    /// Uses `LoadOp::Load` so the UI overlays whatever was previously rendered.
    /// For a standalone UI demo, clear the surface before calling this.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        draw_list: &DrawList,
        screen_w: f32,
        screen_h: f32,
    ) {
        if draw_list.vertices.is_empty() || draw_list.indices.is_empty() {
            return;
        }

        // Update projection if screen size changed
        if (screen_w - self.last_screen_w).abs() > 0.5
            || (screen_h - self.last_screen_h).abs() > 0.5
        {
            self.projection = ortho_projection(screen_w, screen_h);
            queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::cast_slice(&self.projection),
            );
            self.last_screen_w = screen_w;
            self.last_screen_h = screen_h;
        }

        // Create vertex and index buffers from DrawList
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI Vertex Buffer"),
            contents: bytemuck::cast_slice(&draw_list.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI Index Buffer"),
            contents: bytemuck::cast_slice(&draw_list.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Begin render pass (Load to overlay on existing content)
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.font_bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            // Draw each command
            for cmd in &draw_list.commands {
                pass.draw_indexed(
                    cmd.index_offset..cmd.index_offset + cmd.index_count,
                    0,
                    0..1,
                );
            }
        }
    }
}

/// Orthographic projection: left=0, right=w, top=0, bottom=h (Y down), near=-1, far=1.
/// Returns a column-major 4x4 matrix as [f32; 16].
fn ortho_projection(w: f32, h: f32) -> [f32; 16] {
    let l = 0.0_f32;
    let r = w;
    let t = 0.0_f32;
    let b = h;
    let n = -1.0_f32;
    let f = 1.0_f32;

    // Column-major layout for wgpu/WGSL mat4x4
    [
        2.0 / (r - l),          0.0,                    0.0,                0.0,
        0.0,                    2.0 / (t - b),          0.0,                0.0,
        0.0,                    0.0,                    1.0 / (f - n),      0.0,
        -(r + l) / (r - l),    -(t + b) / (t - b),     -n / (f - n),       1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ortho_projection_corners() {
        let proj = ortho_projection(800.0, 600.0);
        // Top-left (0,0) should map to NDC (-1, 1)
        let x = proj[0] * 0.0 + proj[4] * 0.0 + proj[8] * 0.0 + proj[12];
        let y = proj[1] * 0.0 + proj[5] * 0.0 + proj[9] * 0.0 + proj[13];
        assert!((x - (-1.0)).abs() < 1e-5, "top-left x: {}", x);
        assert!((y - 1.0).abs() < 1e-5, "top-left y: {}", y);

        // Bottom-right (800, 600) should map to NDC (1, -1)
        let x2 = proj[0] * 800.0 + proj[4] * 600.0 + proj[8] * 0.0 + proj[12];
        let y2 = proj[1] * 800.0 + proj[5] * 600.0 + proj[9] * 0.0 + proj[13];
        assert!((x2 - 1.0).abs() < 1e-5, "bot-right x: {}", x2);
        assert!((y2 - (-1.0)).abs() < 1e-5, "bot-right y: {}", y2);
    }
}
