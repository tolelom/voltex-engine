use bytemuck::{Pod, Zeroable};

/// Texture format used for the RT point/spot shadow output (single-channel float).
pub const RT_POINT_SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

/// Uniform parameters for the RT point/spot shadow compute pass.
///
/// Layout (32 bytes, 16-byte aligned):
///   light_position [f32; 3] + radius: f32   → 16 bytes
///   width: u32, height: u32, _pad: [u32; 2] → 16 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct RtPointShadowParams {
    pub light_position: [f32; 3],
    pub radius: f32,
    pub width: u32,
    pub height: u32,
    pub _pad: [u32; 2],
}

impl Default for RtPointShadowParams {
    fn default() -> Self {
        Self::new()
    }
}

impl RtPointShadowParams {
    pub fn new() -> Self {
        RtPointShadowParams {
            light_position: [0.0; 3],
            radius: 25.0,
            width: 0,
            height: 0,
            _pad: [0; 2],
        }
    }
}

/// RT Point/Spot Shadow compute pipeline.
pub struct RtPointShadow {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    pub enabled: bool,
}

impl RtPointShadow {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RT Point Shadow Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("rt_point_shadow.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RT Point Shadow BGL"),
            entries: &[
                // binding 0: position texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } },
                    count: None,
                },
                // binding 1: output storage texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: RT_POINT_SHADOW_FORMAT, view_dimension: wgpu::TextureViewDimension::D2 },
                    count: None,
                },
                // binding 2: params uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RT Point Shadow PL"), bind_group_layouts: &[&bind_group_layout], immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("RT Point Shadow Pipeline"), layout: Some(&pipeline_layout),
            module: &shader, entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(), cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RT Point Shadow Params"),
            size: std::mem::size_of::<RtPointShadowParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        RtPointShadow { pipeline, bind_group_layout, params_buffer, enabled: true }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        position_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        params: &RtPointShadowParams,
        width: u32,
        height: u32,
    ) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RT Point Shadow BG"), layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(position_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(output_view) },
                wgpu::BindGroupEntry { binding: 2, resource: self.params_buffer.as_entire_binding() },
            ],
        });

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("RT Point Shadow Pass"), timestamp_writes: None });
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
        let size = std::mem::size_of::<RtPointShadowParams>();
        assert_eq!(size % 16, 0, "RtPointShadowParams size must be 16-byte aligned, got {}", size);
    }

    #[test]
    fn test_params_default() {
        let p = RtPointShadowParams::new();
        assert_eq!(p.light_position, [0.0; 3]);
        assert!((p.radius - 25.0).abs() < 1e-6);
        assert_eq!(p.width, 0);
        assert_eq!(p.height, 0);
    }
}
