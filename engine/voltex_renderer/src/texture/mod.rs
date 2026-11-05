//! GPU texture wrapper + bind-group-layout helpers for the PBR material slots
//! (albedo / normal / ORM / emissive).
#![allow(missing_docs)]

// BMP 디코더는 GPU 무관이라 voltex_image 로 옮겼다. 기존 경로
// `voltex_renderer::texture::{parse_bmp, BmpImage, BMP_MAX_DIMENSION}` 는 유지.
pub use voltex_image::bmp::{BmpImage, BMP_MAX_DIMENSION};

/// Decode an uncompressed 24/32-bit BMP. Thin wrapper over
/// [`voltex_image::parse_bmp`] that keeps the renderer's error type.
pub fn parse_bmp(data: &[u8]) -> crate::Result<BmpImage> {
    voltex_image::parse_bmp(data).map_err(|e| crate::RendererError::Generic(e.to_string()))
}

/// RGBA8 GPU texture + its view, sampler and a group bound with
/// [`GpuTexture::bind_group_layout`] (binding 0 = view, 1 = sampler).
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
}

impl GpuTexture {
    pub fn from_rgba(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pixels: &[u8],
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        // Color/albedo data is sRGB-encoded.
        Self::from_rgba_with_format(
            device,
            queue,
            width,
            height,
            pixels,
            layout,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )
    }

    /// Like [`Self::from_rgba`] but uploads as **linear** `Rgba8Unorm` — use for
    /// non-color data (normal maps, ORM/metallic-roughness, masks) so the GPU
    /// does not apply an sRGB→linear decode to it.
    pub fn from_rgba_linear(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pixels: &[u8],
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self::from_rgba_with_format(
            device,
            queue,
            width,
            height,
            pixels,
            layout,
            wgpu::TextureFormat::Rgba8Unorm,
        )
    }

    fn from_rgba_with_format(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pixels: &[u8],
        layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GpuTexture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("GpuTexture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GpuTexture Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            texture,
            view,
            sampler,
            bind_group,
        }
    }

    /// Decode a PNG byte buffer and upload it as a GPU texture. Convenience
    /// over [`decode_png`](crate::decode_png) + [`Self::from_rgba`]. Returns the
    /// decoder error message on failure.
    pub fn from_png(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        png_bytes: &[u8],
        layout: &wgpu::BindGroupLayout,
    ) -> Result<Self, String> {
        let img = crate::decode_png(png_bytes).map_err(|e| e.to_string())?;
        Ok(Self::from_rgba(device, queue, img.width, img.height, &img.rgba, layout))
    }

    /// Create a 1x1 white texture (sRGB 255,255,255,255). Used as the default
    /// albedo and ORM map (no tint, full occlusion/roughness/metallic factors).
    pub fn white_1x1(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self::from_rgba(device, queue, 1, 1, &[255, 255, 255, 255], layout)
    }

    /// Create a 1x1 black texture (RGBA 0,0,0,255). Used as default emissive (no emission).
    pub fn black_1x1(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self::from_rgba(device, queue, 1, 1, &[0, 0, 0, 255], layout)
    }

    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TextureBindGroupLayout"),
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

    /// Create a 1x1 flat normal map (tangent-space up: 0,0,1) — the default
    /// for PBR materials without a normal map. Linear `Rgba8Unorm`, since
    /// normal data is not sRGB.
    pub fn flat_normal_map_1x1(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self::from_rgba_linear(device, queue, 1, 1, &FLAT_NORMAL_RGBA, layout)
    }

    /// Old form of [`Self::flat_normal_map_1x1`] that returns an unnamed
    /// `(texture, view, sampler)` tuple.
    #[deprecated(note = "use `GpuTexture::flat_normal_map_1x1`, which returns a `GpuTexture`")]
    pub fn flat_normal_1x1(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
        let layout = Self::bind_group_layout(device);
        let t = Self::flat_normal_map_1x1(device, queue, &layout);
        (t.texture, t.view, t.sampler)
    }
}

/// Tangent-space `(0, 0, 1)` encoded as `n * 0.5 + 0.5` → `[128, 128, 255]`.
const FLAT_NORMAL_RGBA: [u8; 4] = [128, 128, 255, 255];

/// Bind group layout for PBR textures: albedo (binding 0-1) + normal map (binding 2-3).
pub fn pbr_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("PBR Texture Bind Group Layout"),
        entries: &[
            // binding 0: albedo texture
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
            // binding 1: albedo sampler
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // binding 2: normal map texture
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            // binding 3: normal map sampler
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Create a bind group for PBR textures (albedo + normal map).
pub fn create_pbr_texture_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    albedo_view: &wgpu::TextureView,
    albedo_sampler: &wgpu::Sampler,
    normal_view: &wgpu::TextureView,
    normal_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("PBR Texture Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(albedo_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(albedo_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(normal_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(normal_sampler),
            },
        ],
    })
}

/// Bind group layout for full PBR textures: albedo (0-1) + normal (2-3) + ORM (4-5) + emissive (6-7).
pub fn pbr_full_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("PBR Full Texture Bind Group Layout"),
        entries: &[
            // binding 0: albedo texture
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
            // binding 1: albedo sampler
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // binding 2: normal map texture
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            // binding 3: normal map sampler
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // binding 4: ORM texture (AO/Roughness/Metallic)
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            // binding 5: ORM sampler
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // binding 6: emissive texture
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            // binding 7: emissive sampler
            wgpu::BindGroupLayoutEntry {
                binding: 7,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Create a bind group for full PBR textures (albedo + normal + ORM + emissive).
#[allow(clippy::too_many_arguments)]
pub fn create_pbr_full_texture_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    albedo_view: &wgpu::TextureView,
    albedo_sampler: &wgpu::Sampler,
    normal_view: &wgpu::TextureView,
    normal_sampler: &wgpu::Sampler,
    orm_view: &wgpu::TextureView,
    orm_sampler: &wgpu::Sampler,
    emissive_view: &wgpu::TextureView,
    emissive_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("PBR Full Texture Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(albedo_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(albedo_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(normal_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(normal_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(orm_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(orm_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(emissive_view),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(emissive_sampler),
            },
        ],
    })
}

