//! Vertex types used by the renderer's GPU pipelines.

use bytemuck::{Pod, Zeroable};

/// PBR mesh vertex (POD geometry) — re-exported from `voltex_image`.
pub use voltex_image::MeshVertex;

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

/// wgpu vertex buffer layout for [`MeshVertex`].
///
/// `MeshVertex` itself lives in `voltex_image` (GPU-free); this const keeps
/// the wgpu layout in the renderer where wgpu is a dependency. Rust's orphan
/// rule forbids an inherent `impl MeshVertex` here, so the layout is a free
/// module-level constant rather than `MeshVertex::LAYOUT`.
pub const MESH_VERTEX_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
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
