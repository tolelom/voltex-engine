#![allow(missing_docs)]

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct FullscreenVertex {
    pub position: [f32; 2],
}

impl FullscreenVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<FullscreenVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
        ],
    };
}

/// Oversized triangle covering the entire screen (clip-space).
/// Three vertices: (-1,-1), (3,-1), (-1,3)
pub const FULLSCREEN_VERTICES: [FullscreenVertex; 3] = [
    FullscreenVertex { position: [-1.0, -1.0] },
    FullscreenVertex { position: [ 3.0, -1.0] },
    FullscreenVertex { position: [-1.0,  3.0] },
];

pub fn create_fullscreen_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Fullscreen Triangle Vertex Buffer"),
        contents: bytemuck::cast_slice(&FULLSCREEN_VERTICES),
        usage: wgpu::BufferUsages::VERTEX,
    })
}
