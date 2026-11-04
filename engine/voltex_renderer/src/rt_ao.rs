#![allow(missing_docs)]

use bytemuck::{Pod, Zeroable};

/// Uniform parameters for the RT ambient occlusion compute pass.
///
/// Layout (16 bytes, 16-byte aligned):
///   num_samples: u32, radius: f32, bias: f32, intensity: f32 → 16 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct RtAoParams {
    pub num_samples: u32,
    pub radius: f32,
    pub bias: f32,
    pub intensity: f32,
}

impl Default for RtAoParams {
    fn default() -> Self {
        Self::new()
    }
}

impl RtAoParams {
    pub fn new() -> Self {
        RtAoParams {
            num_samples: 16,
            radius: 0.5,
            bias: 0.025,
            intensity: 1.0,
        }
    }
}

/// RT Ambient Occlusion compute pipeline.
pub struct RtAo {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    pub enabled: bool,
}

impl RtAo {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RT AO Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("rt_ao.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RT AO BGL"),
            entries: &[
                // binding 0: position texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                // binding 1: normal texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                // binding 2: output storage texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 3: params uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RT AO PL"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("RT AO Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RT AO Params"),
            size: std::mem::size_of::<RtAoParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        RtAo {
            pipeline,
            bind_group_layout,
            params_buffer,
            enabled: true,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        position_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        params: &RtAoParams,
        width: u32,
        height: u32,
    ) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RT AO BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(position_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("RT AO Pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups(width.div_ceil(16), height.div_ceil(16), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_params_size_aligned() {
        let size = std::mem::size_of::<RtAoParams>();
        assert_eq!(
            size % 16,
            0,
            "RtAoParams size must be 16-byte aligned, got {}",
            size
        );
    }

    #[test]
    fn test_params_default() {
        let p = RtAoParams::new();
        assert_eq!(p.num_samples, 16);
        assert!((p.radius - 0.5).abs() < 1e-6);
        assert!((p.bias - 0.025).abs() < 1e-6);
        assert!((p.intensity - 1.0).abs() < 1e-6);
    }
}
