//! Forward transparency pass: alpha-blended meshes drawn into the HDR target
//! without depth writes, plus a back-to-front sort helper.
#![allow(missing_docs)]

use crate::gpu::DEPTH_FORMAT;
use crate::hdr::HDR_FORMAT;
use crate::mesh::Mesh;
use crate::vertex::MESH_VERTEX_LAYOUT;

pub struct ForwardPass {
    pipeline: wgpu::RenderPipeline,
}

impl ForwardPass {
    pub fn new(
        device: &wgpu::Device,
        camera_light_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Forward Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("forward_shader.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Forward Pipeline Layout"),
            bind_group_layouts: &[Some(camera_light_layout), Some(texture_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Forward Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(MESH_VERTEX_LAYOUT)],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
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
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for transparent objects (see both sides)
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false), // Don't write depth (preserve opaque depth)
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        ForwardPass { pipeline }
    }

    pub fn render<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        hdr_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        camera_light_bg: &'a wgpu::BindGroup,
        texture_bg: &'a wgpu::BindGroup,
        meshes: &'a [&Mesh],
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Forward Transparency Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: hdr_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, camera_light_bg, &[]);
        rpass.set_bind_group(1, texture_bg, &[]);

        for mesh in meshes {
            rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }
}

/// Sort transparent objects back-to-front by distance from camera.
pub fn sort_transparent_back_to_front(
    items: &mut [(usize, [f32; 3])], // (index, center_position)
    camera_pos: [f32; 3],
) {
    items.sort_by(|a, b| {
        let da = dist_sq(a.1, camera_pos);
        let db = dist_sq(b.1, camera_pos);
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_back_to_front() {
        let mut items = vec![
            (0, [1.0, 0.0, 0.0]),  // close
            (1, [10.0, 0.0, 0.0]), // far
            (2, [5.0, 0.0, 0.0]),  // mid
        ];
        sort_transparent_back_to_front(&mut items, [0.0, 0.0, 0.0]);
        assert_eq!(items[0].0, 1); // farthest first
        assert_eq!(items[1].0, 2);
        assert_eq!(items[2].0, 0); // closest last
    }

    #[test]
    fn test_sort_equal_distance() {
        let mut items = vec![
            (0, [1.0, 0.0, 0.0]),
            (1, [0.0, 1.0, 0.0]),
            (2, [0.0, 0.0, 1.0]),
        ];
        // All at distance 1.0 from origin — should not crash
        sort_transparent_back_to_front(&mut items, [0.0, 0.0, 0.0]);
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_sort_empty() {
        let mut items: Vec<(usize, [f32; 3])> = vec![];
        sort_transparent_back_to_front(&mut items, [0.0, 0.0, 0.0]);
        assert!(items.is_empty());
    }
}
