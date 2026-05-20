use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct TaaParams {
    jitter: [f32; 2],
    prev_jitter: [f32; 2],
    blend_factor: f32,
    _pad: [f32; 3],
}

pub struct Taa {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    jitter_index: usize,
    prev_jitter: [f32; 2],
    pub blend_factor: f32,
    pub enabled: bool,
}

/// Halton sequence for jitter offsets (base 2 and 3).
pub fn halton(index: usize, base: usize) -> f32 {
    let mut result = 0.0_f32;
    let mut f = 1.0 / base as f32;
    let mut i = index;
    while i > 0 {
        result += f * (i % base) as f32;
        i /= base;
        f /= base as f32;
    }
    result
}

/// Generate jitter offset for frame N (in pixel units, centered around 0).
pub fn jitter_offset(frame: usize) -> [f32; 2] {
    let idx = (frame % 16) + 1; // avoid index 0
    [halton(idx, 2) - 0.5, halton(idx, 3) - 0.5]
}

impl Taa {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TAA Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("taa.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TAA BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } },
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
            label: Some("TAA PL"), bind_group_layouts: &[&bind_group_layout], immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("TAA Pipeline"), layout: Some(&pipeline_layout),
            module: &shader, entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(), cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TAA Params"), size: std::mem::size_of::<TaaParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Taa {
            pipeline, bind_group_layout, params_buffer,
            jitter_index: 0, prev_jitter: [0.0; 2],
            blend_factor: 0.1, enabled: true,
        }
    }

    /// Get current jitter and advance frame index.
    pub fn next_jitter(&mut self) -> [f32; 2] {
        self.prev_jitter = jitter_offset(self.jitter_index);
        self.jitter_index += 1;
        
        jitter_offset(self.jitter_index)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        current_view: &wgpu::TextureView,
        history_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        jitter: [f32; 2],
        width: u32,
        height: u32,
    ) {
        let params = TaaParams {
            jitter: [jitter[0] / width as f32, jitter[1] / height as f32],
            prev_jitter: [self.prev_jitter[0] / width as f32, self.prev_jitter[1] / height as f32],
            blend_factor: self.blend_factor,
            _pad: [0.0; 3],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[params]));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TAA BG"), layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(current_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(history_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(output_view) },
                wgpu::BindGroupEntry { binding: 3, resource: self.params_buffer.as_entire_binding() },
            ],
        });

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("TAA Pass"), timestamp_writes: None });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups(width.div_ceil(16), height.div_ceil(16), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_halton_base2() {
        assert!((halton(1, 2) - 0.5).abs() < 1e-6);
        assert!((halton(2, 2) - 0.25).abs() < 1e-6);
        assert!((halton(3, 2) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_halton_base3() {
        assert!((halton(1, 3) - 1.0/3.0).abs() < 1e-6);
        assert!((halton(2, 3) - 2.0/3.0).abs() < 1e-6);
    }

    #[test]
    fn test_jitter_offset_centered() {
        for i in 0..16 {
            let j = jitter_offset(i);
            assert!(j[0] >= -0.5 && j[0] <= 0.5, "jitter x out of range: {}", j[0]);
            assert!(j[1] >= -0.5 && j[1] <= 0.5, "jitter y out of range: {}", j[1]);
        }
    }

    #[test]
    fn test_jitter_varies() {
        let j0 = jitter_offset(0);
        let j1 = jitter_offset(1);
        assert!(j0 != j1, "consecutive jitters should differ");
    }

    #[test]
    fn test_halton_zero() {
        assert!((halton(0, 2) - 0.0).abs() < 1e-6);
    }
}
