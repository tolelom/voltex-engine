//! Batched tilemap rendering for the `BasicRenderer` path. A whole map is
//! expanded on the CPU into world-space quads ([`TilemapVertex`]) and drawn
//! with one texture bind + one indexed draw, instead of one billboard sprite
//! (and one dynamic-UBO slot) per cell.

use crate::gpu::DEPTH_FORMAT;
use bytemuck::{Pod, Zeroable};

/// One corner of a tile quad, already in world space.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub struct TilemapVertex {
    /// World-space position.
    pub position: [f32; 3],
    /// Atlas texture coordinate.
    pub uv: [f32; 2],
    /// Linear RGBA tint multiplied with the texture sample.
    pub color: [f32; 4],
}

/// Vertex buffer layout for [`TilemapVertex`].
pub const TILEMAP_VERTEX_LAYOUT: wgpu::VertexBufferLayout = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<TilemapVertex>() as wgpu::BufferAddress,
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
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: 20,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x4,
        },
    ],
};

/// Indices for quad `q` of a tilemap vertex stream (4 vertices per quad,
/// corner order bottom-left, bottom-right, top-right, top-left).
pub fn tilemap_quad_indices(q: u32) -> [u32; 6] {
    let b = q * 4;
    [b, b + 1, b + 2, b, b + 2, b + 3]
}

/// Per-frame tilemap uniform (std140).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TilemapUniform {
    /// Camera view-projection matrix (column-major).
    pub view_proj: [[f32; 4]; 4],
}

/// Bind group layout for [`TilemapUniform`] (group 0, vertex visible).
pub fn tilemap_uniform_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Tilemap Uniform BGL"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(
                    std::mem::size_of::<TilemapUniform>() as u64,
                ),
            },
            count: None,
        }],
    })
}

/// Build the tilemap pipeline. Same blend/depth state as the sprite pipeline
/// (alpha-blended, depth-tested, no depth write, no culling) so a map drawn
/// here composites exactly like per-cell sprites did. `texture_layout` =
/// [`GpuTexture::bind_group_layout`](crate::GpuTexture).
pub fn create_tilemap_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Tilemap Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("tilemap.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Tilemap Pipeline Layout"),
        bind_group_layouts: &[uniform_layout, texture_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Tilemap Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[TILEMAP_VERTEX_LAYOUT],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
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
    use std::mem;

    #[test]
    fn tilemap_vertex_matches_layout() {
        assert_eq!(mem::size_of::<TilemapVertex>(), 36);
        assert_eq!(TILEMAP_VERTEX_LAYOUT.array_stride, 36);
        // Attribute offsets line up with the struct fields.
        assert_eq!(mem::offset_of!(TilemapVertex, uv), 12);
        assert_eq!(mem::offset_of!(TilemapVertex, color), 20);
    }

    #[test]
    fn tilemap_uniform_is_std140_sized() {
        assert_eq!(mem::size_of::<TilemapUniform>(), 64);
    }

    #[test]
    fn quad_indices_are_two_ccw_triangles_per_quad() {
        assert_eq!(tilemap_quad_indices(0), [0, 1, 2, 0, 2, 3]);
        assert_eq!(tilemap_quad_indices(2), [8, 9, 10, 8, 10, 11]);
    }
}
