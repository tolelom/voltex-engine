//! UI renderer — uploads a [`DrawList`] to wgpu buffers and emits per-frame
//! draw calls behind the textured-quad UI pipeline.
#![allow(missing_docs)]

use wgpu::util::DeviceExt;

use crate::draw_list::{DrawList, DrawVertex};
use crate::bitmap_font::FontAtlas;

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

/// A color target to draw into and its size in pixels (the size sets the
/// projection and the default scissor).
#[derive(Clone, Copy)]
pub struct ScreenTarget<'a> {
    pub view: &'a wgpu::TextureView,
    pub width: f32,
    pub height: f32,
}

/// GPU-side UI renderer that turns a DrawList into a rendered frame.
pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    /// Same layout/blend as `pipeline` but samples full RGBA (`fs_image`) —
    /// used for commands carrying a registered texture slot.
    image_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    font_bind_group: wgpu::BindGroup,
    /// Bind groups for textures registered via [`Self::register_texture`],
    /// indexed by the returned slot. Never freed (editor-lifetime scale).
    textures: Vec<wgpu::BindGroup>,
    uniform_buffer: wgpu::Buffer,
    projection: [f32; 16],
    last_screen_w: f32,
    last_screen_h: f32,
    /// Set to `true` when the font atlas should be replaced via `update_font_atlas`.
    pub atlas_dirty: bool,
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

        let bind_group_layout = create_bind_group_layout(device);

        // --- Pipeline layout ---
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        // --- Render pipelines: alpha blend, no depth. `fs_main` (font-alpha)
        // for text/rects, `fs_image` (full RGBA) for registered textures. ---
        let pipeline = build_ui_pipeline(device, &shader, &pipeline_layout, surface_format, "fs_main");
        let image_pipeline =
            build_ui_pipeline(device, &shader, &pipeline_layout, surface_format, "fs_image");

        let atlas_view = upload_font_atlas(device, queue, font);

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

        let font_bind_group = create_texture_bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &atlas_view,
            &sampler,
            "UI Font Bind Group",
        );

        UiRenderer {
            pipeline,
            image_pipeline,
            bind_group_layout,
            sampler,
            font_bind_group,
            textures: Vec::new(),
            uniform_buffer,
            projection,
            last_screen_w: 800.0,
            last_screen_h: 600.0,
            atlas_dirty: false,
        }
    }

    /// Register a texture view for UI image widgets and return its slot for
    /// [`DrawList::add_image`] / [`UiContext::image`](crate::UiContext::image).
    /// The view must outlive… nothing: wgpu bind groups keep the underlying
    /// texture alive. Slots are never freed (editor-lifetime scale — see
    /// design doc); registering per frame would leak, cache per asset instead.
    pub fn register_texture(&mut self, device: &wgpu::Device, view: &wgpu::TextureView) -> u32 {
        let bind_group = create_texture_bind_group(
            device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            view,
            &self.sampler,
            "UI Image Bind Group",
        );
        let slot = self.textures.len() as u32;
        self.textures.push(bind_group);
        slot
    }

    /// Replace the font atlas texture with new data (e.g., TTF atlas).
    /// Creates a new texture, view, and bind group.
    pub fn update_font_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas_data: &[u8],
        width: u32,
        height: u32,
    ) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TTF Font Atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            atlas_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.font_bind_group = create_texture_bind_group(
            device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &view,
            &self.sampler,
            "UI Font Bind Group",
        );

        self.atlas_dirty = false;
    }

    /// Old form of [`Self::render_to`] with the screen size spelled out.
    #[deprecated(note = "use `render_to` with a `ScreenTarget`")]
    #[allow(clippy::too_many_arguments)] // reason: legacy signature kept for compatibility
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
        let target = ScreenTarget { view: target_view, width: screen_w, height: screen_h };
        self.render_to(device, queue, encoder, &target, draw_list);
    }

    /// Render the draw list onto `target`.
    ///
    /// Uses `LoadOp::Load` so the UI overlays whatever was previously rendered.
    /// For a standalone UI demo, clear the surface before calling this.
    pub fn render_to(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &ScreenTarget,
        draw_list: &DrawList,
    ) {
        let ScreenTarget { view: target_view, width: screen_w, height: screen_h } = *target;
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

            // Draw each command (with optional scissor clipping). Commands
            // carrying a registered texture slot switch to the RGBA image
            // pipeline + that texture's bind group; an unregistered slot
            // falls back to the font path (visible glitch, never a panic).
            // `was_image` tracks state so consecutive font commands (the
            // overwhelming majority) don't re-bind.
            let mut was_image = false;
            for cmd in &draw_list.commands {
                let image_group = cmd.texture.and_then(|slot| self.textures.get(slot as usize));
                match image_group {
                    Some(group) => {
                        pass.set_pipeline(&self.image_pipeline);
                        pass.set_bind_group(0, group, &[]);
                        was_image = true;
                    }
                    None => {
                        if was_image {
                            pass.set_pipeline(&self.pipeline);
                            pass.set_bind_group(0, &self.font_bind_group, &[]);
                            was_image = false;
                        }
                    }
                }
                if let Some(scissor) = &cmd.scissor {
                    pass.set_scissor_rect(scissor.x, scissor.y, scissor.w, scissor.h);
                } else {
                    pass.set_scissor_rect(0, 0, screen_w as u32, screen_h as u32);
                }
                pass.draw_indexed(
                    cmd.index_offset..cmd.index_offset + cmd.index_count,
                    0,
                    0..1,
                );
            }
        }
    }
}

/// Bind group layout shared by every UI bind group: @binding(0) projection
/// uniform, @binding(1) texture, @binding(2) sampler.
fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
    })
}

/// Uploads the bitmap font atlas as an R8Unorm texture and returns its view.
fn upload_font_atlas(device: &wgpu::Device, queue: &wgpu::Queue, font: &FontAtlas) -> wgpu::TextureView {
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
    atlas_texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Bind group for one sampled texture: the shared projection uniform, `view`
/// and the UI sampler.
fn create_texture_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform_buffer: &wgpu::Buffer,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// Build one UI render pipeline (alpha blend, no depth) with the given
/// fragment entry point — shared by the font (`fs_main`) and image
/// (`fs_image`) pipelines so their state stays identical.
fn build_ui_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    pipeline_layout: &wgpu::PipelineLayout,
    surface_format: wgpu::TextureFormat,
    fragment_entry: &str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("UI Render Pipeline"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(VERTEX_LAYOUT)],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry),
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
    })
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
