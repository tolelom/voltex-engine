//! Offscreen color + depth render target with CPU readback.

/// PNG-friendly color format. sRGB so the bytes we read back are already
/// gamma-encoded and look correct when written straight into a PNG.
pub const CAPTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Depth format matching [`voltex_renderer::DEPTH_FORMAT`].
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn align_up(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

/// An offscreen render target: a color texture (RGBA8 sRGB, COPY_SRC-capable)
/// plus a matching depth attachment.
pub struct OffscreenTarget {
    /// Color attachment texture.
    pub color: wgpu::Texture,
    /// Color attachment view (pass this as `target_view`).
    pub color_view: wgpu::TextureView,
    /// Depth attachment view (pass this as `depth_view`).
    pub depth_view: wgpu::TextureView,
    /// Target width in pixels.
    pub width: u32,
    /// Target height in pixels.
    pub height: u32,
}

impl OffscreenTarget {
    /// Allocate a fresh offscreen target of the given size.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Capture Color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CAPTURE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());

        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Capture Depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            color,
            color_view,
            depth_view,
            width,
            height,
        }
    }

    /// Copy the color texture back to CPU memory as tightly-packed RGBA8
    /// (`width * height * 4` bytes, top row first). Blocks until the GPU
    /// finishes and the buffer maps.
    pub fn read_rgba8(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
        let unpadded_bytes_per_row = self.width * 4;
        let padded_bytes_per_row =
            align_up(unpadded_bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer_size = (padded_bytes_per_row * self.height) as u64;

        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Capture Readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Capture Copy Encoder"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll");

        let mapped = slice.get_mapped_range().expect("map readback buffer");
        let mut out = Vec::with_capacity((unpadded_bytes_per_row * self.height) as usize);
        for row in 0..self.height {
            let start = (row * padded_bytes_per_row) as usize;
            out.extend_from_slice(&mapped[start..start + unpadded_bytes_per_row as usize]);
        }
        drop(mapped);
        readback.unmap();
        out
    }
}
