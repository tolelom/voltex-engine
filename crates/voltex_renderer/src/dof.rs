use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct DofParams {
    pub focus_distance: f32,
    pub focus_range: f32,
    pub max_blur: f32,
    pub _pad: f32,
}

impl DofParams {
    pub fn new() -> Self {
        DofParams { focus_distance: 5.0, focus_range: 3.0, max_blur: 5.0, _pad: 0.0 }
    }
}

pub struct DepthOfField {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    pub enabled: bool,
}

impl DepthOfField {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DOF Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("dof.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DOF BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Depth },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba16Float, view_dimension: wgpu::TextureViewDimension::D2 },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DOF PL"), bind_group_layouts: &[&bind_group_layout], immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DOF Pipeline"), layout: Some(&pipeline_layout),
            module: &shader, entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(), cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DOF Params"),
            size: std::mem::size_of::<DofParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        DepthOfField { pipeline, bind_group_layout, params_buffer, enabled: true }
    }

    pub fn dispatch(
        &self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView, depth_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView, params: &DofParams, width: u32, height: u32,
    ) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DOF BG"), layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(color_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(output_view) },
                wgpu::BindGroupEntry { binding: 3, resource: self.params_buffer.as_entire_binding() },
            ],
        });
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("DOF Pass"), timestamp_writes: None });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups((width + 15) / 16, (height + 15) / 16, 1);
    }
}

/// CPU circle-of-confusion calculation (for testing).
pub fn circle_of_confusion(depth: f32, focus_distance: f32, focus_range: f32, max_blur: f32) -> f32 {
    let diff = (depth - focus_distance).abs();
    (diff / focus_range).clamp(0.0, 1.0) * max_blur
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coc_in_focus() {
        let coc = circle_of_confusion(5.0, 5.0, 3.0, 5.0);
        assert!((coc - 0.0).abs() < 1e-6); // at focus distance
    }

    #[test]
    fn test_coc_near() {
        let coc = circle_of_confusion(2.0, 5.0, 3.0, 5.0);
        assert!((coc - 5.0).abs() < 1e-6); // diff=3, range=3 -> full blur
    }

    #[test]
    fn test_coc_far() {
        let coc = circle_of_confusion(8.0, 5.0, 3.0, 5.0);
        assert!((coc - 5.0).abs() < 1e-6); // diff=3, range=3 -> full blur
    }

    #[test]
    fn test_coc_partial() {
        let coc = circle_of_confusion(6.5, 5.0, 3.0, 5.0);
        assert!((coc - 2.5).abs() < 1e-6); // diff=1.5, range=3 -> 0.5 * 5 = 2.5
    }

    #[test]
    fn test_params_default() {
        let p = DofParams::new();
        assert!((p.focus_distance - 5.0).abs() < 1e-6);
        assert!((p.focus_range - 3.0).abs() < 1e-6);
    }
}
