#![allow(missing_docs)]

use crate::gpu::DEPTH_FORMAT;
use crate::shadow::SHADOW_FORMAT;
use crate::skinned_vertex::{BonePalette, SkinnedMeshVertex};
use crate::vertex::{PositionColorVertex, MESH_VERTEX_LAYOUT};

/// Unlit pipeline for [`PositionColorVertex`] (clip-space position + vertex
/// color, no bind groups, no depth) — the triangle example.
pub fn create_position_color_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Position Color Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("position_color.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Position Color Pipeline Layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Position Color Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(PositionColorVertex::LAYOUT)],
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
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

/// Old name of [`create_position_color_pipeline`].
#[deprecated(note = "use `create_position_color_pipeline`")]
pub fn create_render_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    create_position_color_pipeline(device, format)
}

/// Bind group layout for the skinned shader's camera UBO (group 0): a single
/// `SkinCamera { view_proj, model }` uniform, vertex-visible, no dynamic offset.
pub fn skinned_camera_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Skinned Camera BGL"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                // 2 × mat4 = 128 bytes.
                min_binding_size: wgpu::BufferSize::new(128),
            },
            count: None,
        }],
    })
}

/// Bind group layout for the bone palette UBO (group 1 of the skinned shader):
/// `array<mat4x4, MAX_BONES>`, vertex-visible.
pub fn bone_palette_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bone Palette BGL"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(
                    std::mem::size_of::<BonePalette>() as u64
                ),
            },
            count: None,
        }],
    })
}

/// Build the skinned-mesh render pipeline (`mesh_skinned.wgsl`): GPU vertex
/// skinning via a bone palette. `camera_layout` is group 0
/// ([`skinned_camera_bind_group_layout`]), `palette_layout` is group 1
/// ([`bone_palette_bind_group_layout`]). Consumes [`SkinnedMeshVertex`].
pub fn create_skinned_mesh_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_layout: &wgpu::BindGroupLayout,
    palette_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Skinned Mesh Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("mesh_skinned.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Skinned Mesh Pipeline Layout"),
        bind_group_layouts: &[Some(camera_layout), Some(palette_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Skinned Mesh Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(SkinnedMeshVertex::LAYOUT)],
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
            // Skinning can flip winding under extreme poses; don't cull so the
            // deformation stays visible from either side in the demo.
            cull_mode: None,
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

/// Like [`create_mesh_pipeline`] but uses `mesh_colored.wgsl`, whose per-entity
/// camera uniform carries a `base_color` tint (see
/// [`crate::ColoredCameraUniform`]). Same bind group layout shape as the plain
/// mesh pipeline; the camera UBO is just larger (extra `vec4`).
pub fn create_colored_mesh_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_light_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
    point_lights_layout: &wgpu::BindGroupLayout,
    shadow_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Mesh Colored Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("mesh_colored.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Mesh Colored Pipeline Layout"),
        bind_group_layouts: &[
            Some(camera_light_layout),
            Some(texture_layout),
            Some(point_lights_layout),
            Some(shadow_layout),
        ],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Mesh Colored Pipeline"),
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

pub fn create_mesh_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_light_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Mesh Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("mesh_shader.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Mesh Pipeline Layout"),
        bind_group_layouts: &[Some(camera_light_layout), Some(texture_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Mesh Pipeline"),
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

/// Depth-only skinned shadow caster pipeline. Mirrors [`create_shadow_pipeline`]
/// (no fragment stage, [`SHADOW_FORMAT`] depth, front-face cull + slope bias to
/// curb shadow acne) but takes the skinned vertex layout and the colored skinned
/// bind-group layouts (`camera_layout` = [`crate::SkinnedCameraUniform`],
/// `palette_layout` = [`BonePalette`]). The caller writes the light
/// view-projection into the camera UBO's `view_proj` slot so skinned meshes
/// write into the directional shadow map. Uses `skinned_shadow_shader.wgsl`.
pub fn create_skinned_shadow_pipeline(
    device: &wgpu::Device,
    camera_layout: &wgpu::BindGroupLayout,
    palette_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Skinned Shadow Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("skinned_shadow_shader.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Skinned Shadow Pipeline Layout"),
        bind_group_layouts: &[Some(camera_layout), Some(palette_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Skinned Shadow Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(SkinnedMeshVertex::LAYOUT)],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: None,
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            // Skinning can flip winding under extreme poses; don't cull so a
            // deformed limb never drops out of the shadow map.
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: SHADOW_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
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
