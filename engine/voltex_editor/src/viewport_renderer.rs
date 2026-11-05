//! Renders a [`ViewportTexture`](crate::ViewportTexture)'s color attachment
//! as a textured quad inside the UI pipeline.
#![allow(missing_docs)]

use bytemuck::{Pod, Zeroable};

use crate::dock::Rect;
use crate::ui_renderer::ScreenTarget;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct RectUniform {
    rect: [f32; 4],
    screen: [f32; 2],
    _pad: [f32; 2],
}

pub struct ViewportRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
}

impl ViewportRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Viewport Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("viewport_shader.wgsl").into()),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Viewport Blit BGL"),
                entries: &[
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Viewport Blit PL"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Viewport Blit Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Viewport Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Viewport Rect Uniform"),
            size: std::mem::size_of::<RectUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        ViewportRenderer {
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buffer,
        }
    }

    /// Old form of [`Self::render_to`] with the screen size and viewport
    /// rect spelled out.
    #[deprecated(note = "use `render_to` with a `ScreenTarget` and a `Rect`")]
    #[allow(clippy::too_many_arguments)] // reason: legacy signature kept for compatibility
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        viewport_color_view: &wgpu::TextureView,
        screen_w: f32,
        screen_h: f32,
        rect_x: f32,
        rect_y: f32,
        rect_w: f32,
        rect_h: f32,
    ) {
        let target = ScreenTarget { view: target_view, width: screen_w, height: screen_h };
        let rect = Rect { x: rect_x, y: rect_y, w: rect_w, h: rect_h };
        self.render_to(device, queue, encoder, &target, viewport_color_view, rect);
    }

    /// Blits `viewport_color_view` into `rect` (screen pixels) of `target`.
    pub fn render_to(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &ScreenTarget,
        viewport_color_view: &wgpu::TextureView,
        rect: Rect,
    ) {
        let ScreenTarget { view: target_view, width: screen_w, height: screen_h } = *target;
        let Rect { x: rect_x, y: rect_y, w: rect_w, h: rect_h } = rect;
        let uniform = RectUniform {
            rect: [rect_x, rect_y, rect_w, rect_h],
            screen: [screen_w, screen_h],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Viewport Blit BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(viewport_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Viewport Blit Pass"),
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

        rpass.set_scissor_rect(
            rect_x.max(0.0) as u32,
            rect_y.max(0.0) as u32,
            rect_w.ceil().max(1.0) as u32,
            rect_h.ceil().max(1.0) as u32,
        );
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..6, 0..1);
    }
}
