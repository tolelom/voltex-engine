#![allow(missing_docs)]

use crate::gpu::DEPTH_FORMAT;
use crate::vertex::MESH_VERTEX_LAYOUT;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub model: [[f32; 4]; 4], // 64 bytes
    pub color: [f32; 4],      // 16 bytes
}

impl InstanceData {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<InstanceData>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 6,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 7,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 64,
                shader_location: 8,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };
}

pub struct InstanceBuffer {
    pub buffer: wgpu::Buffer,
    pub capacity: usize,
    pub count: usize,
}

impl InstanceBuffer {
    pub fn new(device: &wgpu::Device, capacity: usize) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (capacity * std::mem::size_of::<InstanceData>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        InstanceBuffer {
            buffer,
            capacity,
            count: 0,
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[InstanceData],
    ) {
        self.count = instances.len();
        if instances.is_empty() {
            return;
        }

        if instances.len() > self.capacity {
            self.capacity = instances.len().next_power_of_two();
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Instance Buffer"),
                size: (self.capacity * std::mem::size_of::<InstanceData>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(instances));
    }
}

pub fn create_instanced_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_light_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Instanced Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("instanced_shader.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Instanced Pipeline Layout"),
        bind_group_layouts: &[Some(camera_light_layout), Some(texture_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Instanced Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(MESH_VERTEX_LAYOUT), Some(InstanceData::LAYOUT)],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_data_size() {
        assert_eq!(std::mem::size_of::<InstanceData>(), 80);
    }

    #[test]
    fn test_instance_data_layout_attributes() {
        assert_eq!(InstanceData::LAYOUT.attributes.len(), 5);
        assert_eq!(InstanceData::LAYOUT.array_stride, 80);
        assert_eq!(
            InstanceData::LAYOUT.step_mode,
            wgpu::VertexStepMode::Instance
        );
    }

    #[test]
    fn test_instance_data_layout_locations() {
        let attrs = InstanceData::LAYOUT.attributes;
        assert_eq!(attrs[0].shader_location, 4);
        assert_eq!(attrs[1].shader_location, 5);
        assert_eq!(attrs[2].shader_location, 6);
        assert_eq!(attrs[3].shader_location, 7);
        assert_eq!(attrs[4].shader_location, 8);
    }

    #[test]
    fn test_instance_data_layout_offsets() {
        let attrs = InstanceData::LAYOUT.attributes;
        assert_eq!(attrs[0].offset, 0);
        assert_eq!(attrs[1].offset, 16);
        assert_eq!(attrs[2].offset, 32);
        assert_eq!(attrs[3].offset, 48);
        assert_eq!(attrs[4].offset, 64);
    }

    #[test]
    fn test_instance_data_pod() {
        let data = InstanceData {
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            color: [1.0, 1.0, 1.0, 1.0],
        };
        let bytes: &[u8] = bytemuck::bytes_of(&data);
        assert_eq!(bytes.len(), 80);
    }
}
