use crate::brdf_lut::generate_brdf_lut;

pub const BRDF_LUT_SIZE: u32 = 256;

pub struct IblResources {
    pub brdf_lut_texture: wgpu::Texture,
    pub brdf_lut_view: wgpu::TextureView,
    pub brdf_lut_sampler: wgpu::Sampler,
}

impl IblResources {
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

        let brdf_lut_view =
            brdf_lut_texture.create_view(&wgpu::TextureViewDescriptor::default());

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

    /// Bind group layout for group(4):
    ///   binding 0 — texture_2d<f32> (filterable)
    ///   binding 1 — sampler (filtering)
    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("IblBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("IblBindGroup"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.brdf_lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.brdf_lut_sampler),
                },
            ],
        })
    }
}
