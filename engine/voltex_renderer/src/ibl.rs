#![allow(missing_docs)]

use crate::brdf_lut::generate_brdf_lut;

pub(crate) const BRDF_LUT_SIZE: u32 = 256;

pub struct IblResources {
    pub brdf_lut_texture: wgpu::Texture,
    pub brdf_lut_view: wgpu::TextureView,
    pub brdf_lut_sampler: wgpu::Sampler,
}

impl IblResources {
    /// CPU fallback: generates the BRDF LUT on the CPU and uploads as Rgba8Unorm.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let size = BRDF_LUT_SIZE;

        // Generate CPU-side LUT data.
        let lut_data = generate_brdf_lut(size);

        // Convert [f32; 2] → RGBA8 pixels (R=scale*255, G=bias*255, B=0, A=255).
        let mut pixels: Vec<u8> = Vec::with_capacity((size * size * 4) as usize);
        for [scale, bias] in &lut_data {
            pixels.push((scale.clamp(0.0, 1.0) * 255.0).round() as u8);
            pixels.push((bias.clamp(0.0, 1.0) * 255.0).round() as u8);
            pixels.push(0u8);
            pixels.push(255u8);
        }

        let extent = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };

        // Create the texture (linear, NOT sRGB).
        let brdf_lut_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BrdfLutTexture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &brdf_lut_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            extent,
        );

        let brdf_lut_view = brdf_lut_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let brdf_lut_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("BrdfLutSampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            brdf_lut_texture,
            brdf_lut_view,
            brdf_lut_sampler,
        }
    }

    /// GPU compute path: generates the BRDF LUT via a compute shader in Rg16Float format.
    /// Higher precision than the CPU Rgba8Unorm path.
    pub fn new_gpu(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let size = BRDF_LUT_SIZE;

        let extent = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };

        // Create Rg16Float storage texture
        let brdf_lut_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BrdfLutTexture_GPU"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let storage_view = brdf_lut_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create compute pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BRDF LUT Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("brdf_lut_compute.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BRDF LUT Compute BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rg16Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BRDF LUT Compute Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("BRDF LUT Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BRDF LUT Compute Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&storage_view),
            }],
        });

        // Dispatch compute shader
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("BRDF LUT Compute Encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("BRDF LUT Compute Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            // Dispatch enough workgroups to cover size x size texels (16x16 per workgroup)
            let wg_x = size.div_ceil(16);
            let wg_y = size.div_ceil(16);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        queue.submit(std::iter::once(encoder.finish()));

        let brdf_lut_view = brdf_lut_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let brdf_lut_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("BrdfLutSampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            brdf_lut_texture,
            brdf_lut_view,
            brdf_lut_sampler,
        }
    }
}
