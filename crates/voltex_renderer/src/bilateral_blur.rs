pub struct BilateralBlur {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl BilateralBlur {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bilateral Blur Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("bilateral_blur.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bilateral Blur BGL"),
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
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Depth,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bilateral Blur PL"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Bilateral Blur Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        BilateralBlur { pipeline, bind_group_layout }
    }

    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bilateral Blur BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(input_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(normal_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(output_view) },
            ],
        });

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Bilateral Blur Pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups((width + 15) / 16, (height + 15) / 16, 1);
    }
}

/// Pure CPU bilateral weight calculation (for testing).
pub fn bilateral_weight(
    spatial_dist: f32,
    depth_diff: f32,
    normal_dot: f32,
    sigma_spatial: f32,
    sigma_depth: f32,
    sigma_normal: f32,
) -> f32 {
    let w_spatial = (-spatial_dist * spatial_dist / (2.0 * sigma_spatial * sigma_spatial)).exp();
    let w_depth = (-depth_diff.abs() / sigma_depth).exp();
    let w_normal = normal_dot.max(0.0).powf(sigma_normal);
    w_spatial * w_depth * w_normal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bilateral_weight_center() {
        // At center: dist=0, depth_diff=0, normal_dot=1
        let w = bilateral_weight(0.0, 0.0, 1.0, 2.0, 0.1, 16.0);
        assert!((w - 1.0).abs() < 0.01); // all factors = 1
    }

    #[test]
    fn test_bilateral_weight_depth_edge() {
        // Large depth difference -> low weight
        let w = bilateral_weight(0.0, 10.0, 1.0, 2.0, 0.1, 16.0);
        assert!(w < 0.01, "large depth diff should give low weight: {}", w);
    }

    #[test]
    fn test_bilateral_weight_normal_edge() {
        // Perpendicular normals -> low weight
        let w = bilateral_weight(0.0, 0.0, 0.0, 2.0, 0.1, 16.0);
        assert!(w < 0.01, "perpendicular normals should give low weight: {}", w);
    }

    #[test]
    fn test_bilateral_weight_distance() {
        // Far spatial distance -> lower weight than near
        let w_near = bilateral_weight(1.0, 0.0, 1.0, 2.0, 0.1, 16.0);
        let w_far = bilateral_weight(4.0, 0.0, 1.0, 2.0, 0.1, 16.0);
        assert!(w_near > w_far);
    }
}
