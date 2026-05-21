#![allow(missing_docs)]

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SsrParams {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub max_steps: u32,
    pub step_size: f32,
    pub thickness: f32,
    pub _pad: [f32; 2],
}

impl Default for SsrParams {
    fn default() -> Self {
        Self::new()
    }
}

impl SsrParams {
    pub fn new() -> Self {
        SsrParams {
            view_proj: [[0.0; 4]; 4],
            inv_view_proj: [[0.0; 4]; 4],
            camera_pos: [0.0; 3],
            max_steps: 64,
            step_size: 0.1,
            thickness: 0.05,
            _pad: [0.0; 2],
        }
    }
}

pub struct Ssr {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    pub enabled: bool,
}

impl Ssr {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SSR Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ssr.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR BGL"),
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
                // binding 2: color texture (lit scene)
                wgpu::BindGroupLayoutEntry {
                    binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } },
                    count: None,
                },
                // binding 3: output
                wgpu::BindGroupLayoutEntry {
                    binding: 3, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba16Float, view_dimension: wgpu::TextureViewDimension::D2 },
                    count: None,
                },
                // binding 4: params uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 4, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSR PL"), bind_group_layouts: &[&bind_group_layout], immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SSR Pipeline"), layout: Some(&pipeline_layout),
            module: &shader, entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(), cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSR Params"),
            size: std::mem::size_of::<SsrParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ssr { pipeline, bind_group_layout, params_buffer, enabled: true }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        position_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
        color_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        params: &SsrParams,
        width: u32,
        height: u32,
    ) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSR BG"), layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(position_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(normal_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(color_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(output_view) },
                wgpu::BindGroupEntry { binding: 4, resource: self.params_buffer.as_entire_binding() },
            ],
        });

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("SSR Pass"), timestamp_writes: None });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups(width.div_ceil(16), height.div_ceil(16), 1);
    }
}

/// Reflect a vector about a normal (CPU version for testing).
pub fn reflect(incident: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    let d = 2.0 * (incident[0]*normal[0] + incident[1]*normal[1] + incident[2]*normal[2]);
    [
        incident[0] - d * normal[0],
        incident[1] - d * normal[1],
        incident[2] - d * normal[2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reflect_horizontal() {
        // Ray going down, reflected off horizontal surface (normal up)
        let r = reflect([0.0, -1.0, 0.0], [0.0, 1.0, 0.0]);
        assert!((r[0] - 0.0).abs() < 1e-6);
        assert!((r[1] - 1.0).abs() < 1e-6);
        assert!((r[2] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_reflect_45_degrees() {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let r = reflect([s, -s, 0.0], [0.0, 1.0, 0.0]);
        assert!((r[0] - s).abs() < 1e-5);
        assert!((r[1] - s).abs() < 1e-5);
    }

    #[test]
    fn test_ssr_params_default() {
        let p = SsrParams::new();
        assert_eq!(p.max_steps, 64);
        assert!((p.step_size - 0.1).abs() < 1e-6);
        assert!((p.thickness - 0.05).abs() < 1e-6);
    }

    #[test]
    fn test_ssr_params_size() {
        // Must be aligned for uniform buffer
        let size = std::mem::size_of::<SsrParams>();
        assert_eq!(size % 16, 0, "SsrParams size must be 16-byte aligned, got {}", size);
    }
}
