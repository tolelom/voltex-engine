//! Vertex types used by the renderer's GPU pipelines.

use bytemuck::{Pod, Zeroable};

/// Position + color vertex used by the triangle example pipeline.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    /// World-space position `[x, y, z]`.
    pub position: [f32; 3],
    /// RGB color `[r, g, b]` in linear space.
    pub color: [f32; 3],
}

impl Vertex {
    /// wgpu vertex buffer layout for [`Vertex`].
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
        ],
    };
}

/// PBR-ready mesh vertex used by all the engine's mesh pipelines.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MeshVertex {
    /// Local-space position.
    pub position: [f32; 3],
    /// Surface normal (unit vector).
    pub normal: [f32; 3],
    /// UV coordinates `[u, v]`.
    pub uv: [f32; 2],
    /// Tangent vector + handedness `[tx, ty, tz, w]` for normal mapping.
    pub tangent: [f32; 4],
}

impl MeshVertex {
    /// wgpu vertex buffer layout for [`MeshVertex`].
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<MeshVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };
}
