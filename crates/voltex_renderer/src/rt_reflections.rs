use bytemuck::{Pod, Zeroable};

/// Uniform parameters for the RT reflections compute pass.
///
/// Layout (96 bytes, 16-byte aligned):
///   view_proj: mat4x4   → 64 bytes
///   camera_pos [f32; 3] + max_distance: f32 → 16 bytes
///   num_samples: u32 + _pad: [u32; 3]       → 16 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct RtReflParams {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub max_distance: f32,
    pub num_samples: u32,
    pub _pad: [u32; 3],
}

impl Default for RtReflParams {
    fn default() -> Self {
        Self::new()
    }
}

impl RtReflParams {
    pub fn new() -> Self {
        RtReflParams {
            view_proj: [[0.0; 4]; 4],
            camera_pos: [0.0; 3],
            max_distance: 50.0,
            num_samples: 32,
            _pad: [0; 3],
        }
    }
}

/// RT Reflections compute pipeline.
pub struct RtReflections {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    pub enabled: bool,
}

impl RtReflections {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RT Reflections Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("rt_reflections.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RT Reflections BGL"),
            entries: &[
                // binding 0: position texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } },
                    count: None,
                },
                // binding 1: normal texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } },
                    count: None,
                },
                // binding 2: material texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } },
                    count: None,
                },
                // binding 3: color texture (lit scene)
                wgpu::BindGroupLayoutEntry {
                    binding: 3, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } },
                    count: None,
                },
                // binding 4: output storage texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba16Float, view_dimension: wgpu::TextureViewDimension::D2 },
                    count: None,
                },
                // binding 5: params uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 5, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RT Reflections PL"), bind_group_layouts: &[&bind_group_layout], immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("RT Reflections Pipeline"), layout: Some(&pipeline_layout),
            module: &shader, entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(), cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RT Reflections Params"),
            size: std::mem::size_of::<RtReflParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        RtReflections { pipeline, bind_group_layout, params_buffer, enabled: true }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        position_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
        material_view: &wgpu::TextureView,
        color_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        params: &RtReflParams,
        width: u32,
        height: u32,
    ) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RT Reflections BG"), layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(position_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(normal_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(material_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(color_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(output_view) },
                wgpu::BindGroupEntry { binding: 5, resource: self.params_buffer.as_entire_binding() },
            ],
        });

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("RT Reflections Pass"), timestamp_writes: None });
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
        let size = std::mem::size_of::<RtReflParams>();
        assert_eq!(size % 16, 0, "RtReflParams size must be 16-byte aligned, got {}", size);
    }

    #[test]
    fn test_params_default() {
        let p = RtReflParams::new();
        assert!((p.max_distance - 50.0).abs() < 1e-6);
        assert_eq!(p.num_samples, 32);
        assert_eq!(p.camera_pos, [0.0; 3]);
    }
}
