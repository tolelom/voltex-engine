/// Pure CPU exposure calculation logic (testable).
pub fn calculate_target_exposure(
    avg_log_luminance: f32,
    key_value: f32,
    min_exp: f32,
    max_exp: f32,
) -> f32 {
    let avg_lum = avg_log_luminance.exp();
    let target = key_value / avg_lum.max(0.0001);
    target.clamp(min_exp, max_exp)
}

/// Smooth adaptation over time.
pub fn adapt_exposure(current: f32, target: f32, dt: f32, speed: f32) -> f32 {
    current + (target - current) * (1.0 - (-dt * speed).exp())
}

/// GPU-side auto exposure compute + readback.
pub struct AutoExposure {
    compute_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    result_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    pub exposure: f32,
    pub min_exposure: f32,
    pub max_exposure: f32,
    pub adaptation_speed: f32,
    pub key_value: f32,
    pending_read: bool,
}

impl AutoExposure {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Auto Exposure Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("auto_exposure.wgsl").into()),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Auto Exposure BGL"),
                entries: &[
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Auto Exposure PL"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let compute_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Auto Exposure Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

        let result_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Auto Exposure Result"),
            size: 8,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Auto Exposure Staging"),
            size: 8,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        AutoExposure {
            compute_pipeline,
            bind_group_layout,
            result_buffer,
            staging_buffer,
            exposure: 1.0,
            min_exposure: 0.1,
            max_exposure: 10.0,
            adaptation_speed: 2.0,
            key_value: 0.18,
            pending_read: false,
        }
    }

    /// Dispatch compute shader to calculate luminance. Call once per frame.
    pub fn dispatch(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        hdr_view: &wgpu::TextureView,
        hdr_width: u32,
        hdr_height: u32,
    ) {
        // Clear result buffer
        queue.write_buffer(&self.result_buffer, 0, &[0u8; 8]);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Auto Exposure BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.result_buffer.as_entire_binding(),
                },
            ],
        });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Auto Exposure Pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.compute_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            let wg_x = (hdr_width + 15) / 16;
            let wg_y = (hdr_height + 15) / 16;
            cpass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        // Copy result to staging for CPU readback
        encoder.copy_buffer_to_buffer(&self.result_buffer, 0, &self.staging_buffer, 0, 8);
        self.pending_read = true;
    }

    /// Read back luminance result and update exposure. Call after queue.submit().
    /// Returns true if exposure was updated.
    pub fn update_exposure(&mut self, _dt: f32) -> bool {
        if !self.pending_read {
            return false;
        }
        self.pending_read = false;

        let slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // The caller must poll the device for the map to complete.
        // For a full integration, use async or poll in the render loop.
        // For now, return false — use set_average_luminance() for CPU-side updates.
        let _ = rx;
        false
    }

    /// Simple CPU-only exposure update without GPU readback.
    /// Use when you have a luminance estimate from other means.
    pub fn set_average_luminance(&mut self, avg_log_lum: f32, dt: f32) {
        let target = calculate_target_exposure(
            avg_log_lum,
            self.key_value,
            self.min_exposure,
            self.max_exposure,
        );
        self.exposure = adapt_exposure(self.exposure, target, dt, self.adaptation_speed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_target_exposure() {
        // avg_log_lum = ln(0.18) -> avg_lum = 0.18 -> target = 0.18/0.18 = 1.0
        let avg_log_lum = 0.18_f32.ln();
        let target = calculate_target_exposure(avg_log_lum, 0.18, 0.1, 10.0);
        assert!((target - 1.0).abs() < 0.01, "target={}", target);
    }

    #[test]
    fn test_target_exposure_bright_scene() {
        // Bright scene: avg_lum = 2.0 -> target = 0.18/2.0 = 0.09 -> clamped to min 0.1
        let avg_log_lum = 2.0_f32.ln();
        let target = calculate_target_exposure(avg_log_lum, 0.18, 0.1, 10.0);
        assert!((target - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_target_exposure_dark_scene() {
        // Dark scene: avg_lum = 0.001 -> target = 0.18/0.001 = 180 -> clamped to max 10.0
        let avg_log_lum = 0.001_f32.ln();
        let target = calculate_target_exposure(avg_log_lum, 0.18, 0.1, 10.0);
        assert!((target - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_adapt_exposure_no_time() {
        let result = adapt_exposure(1.0, 5.0, 0.0, 2.0);
        assert!((result - 1.0).abs() < 0.01); // dt=0 -> no change
    }

    #[test]
    fn test_adapt_exposure_converges() {
        let mut exp = 1.0;
        for _ in 0..100 {
            exp = adapt_exposure(exp, 5.0, 0.016, 2.0); // 60fps
        }
        assert!(
            (exp - 5.0).abs() < 0.2,
            "should converge to 5.0, got {}",
            exp
        );
    }

    #[test]
    fn test_adapt_exposure_large_dt() {
        let result = adapt_exposure(1.0, 5.0, 100.0, 2.0);
        assert!((result - 5.0).abs() < 0.01); // large dt -> near target
    }
}
