//! Camera-facing billboard sprites for the `BasicRenderer` path: textured (or
//! solid-color) quads that always face the camera — pickups, particles,
//! world-space markers, simple 2D-in-3D enemies. The quad is expanded in the
//! vertex shader from a center point along the camera's right/up axes (passed
//! in the per-sprite uniform), so it stays screen-aligned from any angle.
#![allow(missing_docs)]

use crate::gpu::DEPTH_FORMAT;
use bytemuck::{Pod, Zeroable};

/// A unit-quad corner: 2D offset in [-0.5, 0.5] + its UV. Four of these +
/// [`SPRITE_INDICES`] make one billboard.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SpriteVertex {
    pub corner: [f32; 2],
    pub uv: [f32; 2],
}

/// The four corners of the unit billboard quad (CCW), with UVs.
pub const SPRITE_QUAD: [SpriteVertex; 4] = [
    SpriteVertex { corner: [-0.5, -0.5], uv: [0.0, 1.0] },
    SpriteVertex { corner: [0.5, -0.5], uv: [1.0, 1.0] },
    SpriteVertex { corner: [0.5, 0.5], uv: [1.0, 0.0] },
    SpriteVertex { corner: [-0.5, 0.5], uv: [0.0, 0.0] },
];

/// Indices for the two triangles of [`SPRITE_QUAD`].
pub const SPRITE_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

/// Vertex buffer layout for [`SpriteVertex`].
pub const SPRITE_VERTEX_LAYOUT: wgpu::VertexBufferLayout = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<SpriteVertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &[
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: 8,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
    ],
};

/// Per-sprite uniform (std140: every field padded to a vec4). Camera right/up
/// are duplicated per sprite for simplicity — they're the same each frame but
/// this keeps the bind group a single dynamic-offset buffer.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SpriteUniform {
    pub view_proj: [[f32; 4]; 4],
    pub camera_right: [f32; 4],
    pub camera_up: [f32; 4],
    pub center: [f32; 4],
    pub color: [f32; 4],
    /// `xy` = world-space width/height; `zw` padding.
    pub size: [f32; 4],
    /// Texture sub-rect `[u_min, v_min, u_max, v_max]` for sprite sheets /
    /// atlases. `[0, 0, 1, 1]` samples the whole texture.
    pub uv_rect: [f32; 4],
}

/// Bind group layout for [`SpriteUniform`] (group 0, dynamic offset, vertex +
/// fragment visible).
pub fn sprite_uniform_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Sprite Uniform BGL"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<SpriteUniform>() as u64),
            },
            count: None,
        }],
    })
}

/// Build the billboard pipeline (alpha-blended, depth-tested, no depth write,
/// no culling). `uniform_layout` = [`sprite_uniform_bind_group_layout`],
/// `texture_layout` = [`GpuTexture::bind_group_layout`](crate::GpuTexture).
pub fn create_sprite_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Sprite Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("sprite.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Sprite Pipeline Layout"),
        bind_group_layouts: &[uniform_layout, texture_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Sprite Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[SPRITE_VERTEX_LAYOUT],
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
    fn sprite_uniform_is_std140_sized() {
        // 4×vec4 mat (64) + 6 vec4 (96) = 160, a multiple of 16.
        assert_eq!(mem::size_of::<SpriteUniform>(), 160);
        assert_eq!(mem::size_of::<SpriteUniform>() % 16, 0);
    }

    #[test]
    fn sprite_vertex_is_packed() {
        assert_eq!(mem::size_of::<SpriteVertex>(), 16);
        assert_eq!(SPRITE_QUAD.len(), 4);
        assert_eq!(SPRITE_INDICES.len(), 6);
    }
}
