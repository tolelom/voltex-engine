//! Immediate-mode debug line rendering: a thin GPU layer for drawing colored
//! world-space line segments (collider bounds, nav paths, raycasts, velocity
//! vectors). The pipeline uses `LineList` topology and depth-tests against the
//! scene (so lines are occluded by geometry) without writing depth.
#![allow(missing_docs)]

use crate::gpu::DEPTH_FORMAT;
use bytemuck::{Pod, Zeroable};

/// One endpoint of a debug line segment: world-space position + RGB color.
/// Plain vertex data (no std140 padding — this is a vertex buffer, not a UBO).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl LineVertex {
    pub fn new(position: [f32; 3], color: [f32; 3]) -> Self {
        Self { position, color }
    }
}

/// Vertex buffer layout for [`LineVertex`] (location 0 = position, 1 = color).
pub const LINE_VERTEX_LAYOUT: wgpu::VertexBufferLayout = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<LineVertex>() as wgpu::BufferAddress,
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
    ],
};

/// Camera uniform for the line pipeline (just the combined view-projection).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LineCameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

/// Bind group layout for [`LineCameraUniform`] (group 0, vertex-visible).
pub fn line_camera_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Debug Line Camera BGL"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(
                    std::mem::size_of::<LineCameraUniform>() as u64
                ),
            },
            count: None,
        }],
    })
}

/// Build the debug-line render pipeline (`LineList`, depth-tested, no depth
/// write). `camera_layout` must be a [`line_camera_bind_group_layout`].
pub fn create_line_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Debug Line Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("debug_line.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Debug Line Pipeline Layout"),
        bind_group_layouts: &[camera_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Debug Line Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[LINE_VERTEX_LAYOUT],
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
            topology: wgpu::PrimitiveTopology::LineList,
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
    fn line_vertex_is_tightly_packed() {
        assert_eq!(mem::size_of::<LineVertex>(), 24, "3+3 f32, no padding");
    }

    #[test]
    fn line_camera_uniform_is_a_mat4() {
        assert_eq!(mem::size_of::<LineCameraUniform>(), 64);
    }
}
