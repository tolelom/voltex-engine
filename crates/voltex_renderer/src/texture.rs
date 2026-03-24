pub struct BmpImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA
}

pub fn parse_bmp(data: &[u8]) -> Result<BmpImage, String> {
    if data.len() < 54 {
        return Err(format!("BMP too small: {} bytes", data.len()));
    }
    if data[0] != b'B' || data[1] != b'M' {
        return Err("Not a BMP file: missing 'BM' signature".to_string());
    }

    let pixel_offset = u32::from_le_bytes(data[10..14].try_into().unwrap()) as usize;
    let width_raw = i32::from_le_bytes(data[18..22].try_into().unwrap());
    let height_raw = i32::from_le_bytes(data[22..26].try_into().unwrap());
    let bpp = u16::from_le_bytes(data[28..30].try_into().unwrap());
    let compression = u32::from_le_bytes(data[30..34].try_into().unwrap());

    if bpp != 24 && bpp != 32 {
        return Err(format!("Unsupported BMP bpp: {} (only 24 and 32 supported)", bpp));
    }
    if compression != 0 {
        return Err(format!("Unsupported BMP compression: {} (only 0/uncompressed supported)", compression));
    }

    let width = width_raw.unsigned_abs();
    let height = height_raw.unsigned_abs();
    let bottom_up = height_raw > 0;

    let bytes_per_pixel = (bpp as usize) / 8;
    let row_size = ((bpp as usize * width as usize + 31) / 32) * 4;
    let required_data_size = pixel_offset + row_size * height as usize;

    if data.len() < required_data_size {
        return Err(format!(
            "BMP data too small: need {} bytes, got {}",
            required_data_size,
            data.len()
        ));
    }

    let mut pixels = vec![0u8; (width * height * 4) as usize];

    for row in 0..height as usize {
        let src_row = if bottom_up { height as usize - 1 - row } else { row };
        let row_start = pixel_offset + src_row * row_size;

        for col in 0..width as usize {
            let src_offset = row_start + col * bytes_per_pixel;
            let dst_offset = (row * width as usize + col) * 4;

            // BMP stores BGR(A), convert to RGBA
            let b = data[src_offset];
            let g = data[src_offset + 1];
            let r = data[src_offset + 2];
            let a = if bytes_per_pixel == 4 { data[src_offset + 3] } else { 255 };

            pixels[dst_offset] = r;
            pixels[dst_offset + 1] = g;
            pixels[dst_offset + 2] = b;
            pixels[dst_offset + 3] = a;
        }
    }

    Ok(BmpImage { width, height, pixels })
}

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
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BmpTexture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
            label: Some("BmpSampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BmpBindGroup"),
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

        Self { texture, view, sampler, bind_group }
    }

    pub fn white_1x1(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self::from_rgba(device, queue, 1, 1, &[255, 255, 255, 255], layout)
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

    /// Create a 1x1 flat normal map texture (tangent-space up: 0,0,1).
    /// Uses Rgba8Unorm (linear) since normal data is not sRGB.
    pub fn flat_normal_1x1(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };

        // [128, 128, 255, 255] maps to (0, 0, 1) after * 2 - 1
        let pixels: [u8; 4] = [128, 128, 255, 255];

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FlatNormalTexture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
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
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("FlatNormalSampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        (texture, view, sampler)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bmp_24bit(width: u32, height: u32, pixel_bgr: [u8; 3]) -> Vec<u8> {
        let row_size = ((24 * width + 31) / 32 * 4) as usize;
        let pixel_data_size = row_size * height as usize;
        let file_size = 54 + pixel_data_size;
        let mut data = vec![0u8; file_size];
        data[0] = b'B';
        data[1] = b'M';
        data[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        data[10..14].copy_from_slice(&54u32.to_le_bytes());
        data[14..18].copy_from_slice(&40u32.to_le_bytes());
        data[18..22].copy_from_slice(&(width as i32).to_le_bytes());
        data[22..26].copy_from_slice(&(height as i32).to_le_bytes());
        data[26..28].copy_from_slice(&1u16.to_le_bytes());
        data[28..30].copy_from_slice(&24u16.to_le_bytes());
        for row in 0..height {
            for col in 0..width {
                let offset = 54 + (row as usize) * row_size + (col as usize) * 3;
                data[offset] = pixel_bgr[0];
                data[offset + 1] = pixel_bgr[1];
                data[offset + 2] = pixel_bgr[2];
            }
        }
        data
    }

    #[test]
    fn test_parse_bmp_24bit() {
        let bmp = make_bmp_24bit(2, 2, [255, 0, 0]); // BGR blue
        let img = parse_bmp(&bmp).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.pixels[0], 0);   // R
        assert_eq!(img.pixels[1], 0);   // G
        assert_eq!(img.pixels[2], 255); // B
        assert_eq!(img.pixels[3], 255); // A
    }

    #[test]
    fn test_parse_bmp_not_bmp() {
        let data = vec![0u8; 100];
        assert!(parse_bmp(&data).is_err());
    }

    #[test]
    fn test_parse_bmp_too_small() {
        let data = vec![0u8; 10];
        assert!(parse_bmp(&data).is_err());
    }
}
