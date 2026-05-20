use crate::rt_shadow::{RtShadowUniform, RT_SHADOW_FORMAT};

// ── RT Shadow pipeline ────────────────────────────────────────────────────────

/// Bind group layout for the RT shadow compute pass G-Buffer inputs (group 0).
/// Bindings: position (non-filterable), normal (filterable) — both COMPUTE visibility.
pub fn rt_shadow_gbuffer_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("RT Shadow GBuffer Bind Group Layout"),
        entries: &[
            // binding 0: position texture (Rgba32Float → non-filterable)
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // binding 1: normal texture (filterable)
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    })
}

/// Bind group layout for the RT shadow compute pass data (group 1).
/// Bindings: TLAS, storage texture (R32Float write), RtShadowUniform — all COMPUTE visibility.
pub fn rt_shadow_data_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("RT Shadow Data Bind Group Layout"),
        entries: &[
            // binding 0: TLAS (acceleration structure)
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::AccelerationStructure { vertex_return: false },
                count: None,
            },
            // binding 1: shadow output texture (R32Float, write-only storage)
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: RT_SHADOW_FORMAT,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            // binding 2: RtShadowUniform
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<RtShadowUniform>() as u64,
                    ),
                },
                count: None,
            },
        ],
    })
}

/// Create the RT shadow compute pipeline.
pub fn create_rt_shadow_pipeline(
    device: &wgpu::Device,
    gbuffer_layout: &wgpu::BindGroupLayout,
    data_layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("RT Shadow Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../rt_shadow_shader.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("RT Shadow Pipeline Layout"),
        bind_group_layouts: &[gbuffer_layout, data_layout],
        immediate_size: 0,
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("RT Shadow Compute Pipeline"),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("cs_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}
