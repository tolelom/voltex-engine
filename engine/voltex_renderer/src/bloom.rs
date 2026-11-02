#![allow(missing_docs)]

use crate::hdr::HDR_FORMAT;
use bytemuck::{Pod, Zeroable};

/// Number of bloom mip levels (downsample + upsample chain).
pub const BLOOM_MIP_COUNT: usize = 5;

/// Uniform buffer for the bloom pass.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct BloomUniform {
    /// Luminance threshold above which a pixel contributes to bloom.
    pub threshold: f32,
    /// Soft knee for the threshold.
    pub soft_threshold: f32,
    pub _padding: [f32; 2],
}

impl Default for BloomUniform {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            soft_threshold: 0.5,
            _padding: [0.0; 2],
        }
    }
}

/// GPU resources for the bloom pass (mip chain + uniform buffer).
pub struct BloomResources {
    /// One `TextureView` per mip level (5 levels).
    pub mip_views: Vec<wgpu::TextureView>,
    pub uniform_buffer: wgpu::Buffer,
    /// Blend intensity applied during the tonemap pass.
    pub intensity: f32,
}

impl BloomResources {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let mip_views = create_mip_views(device, width, height);

        let _uniform = BloomUniform::default();
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bloom Uniform Buffer"),
            size: std::mem::size_of::<BloomUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Initialize with defaults via a write at construction time would require a queue;
        // callers may write the buffer themselves. The buffer is left zero-initialised here.

        Self {
            mip_views,
            uniform_buffer,
            intensity: 0.5,
        }
    }

    /// Recreate the mip-chain textures when the window is resized.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.mip_views = create_mip_views(device, width, height);
    }
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Return the (width, height) of each bloom mip level.
///
/// Level 0 = width/2 × height/2; each subsequent level halves again.
/// Sizes are clamped to a minimum of 1.
pub fn mip_sizes(width: u32, height: u32) -> Vec<(u32, u32)> {
    let mut sizes = Vec::with_capacity(BLOOM_MIP_COUNT);
    let mut w = (width / 2).max(1);
    let mut h = (height / 2).max(1);
    for _ in 0..BLOOM_MIP_COUNT {
        sizes.push((w, h));
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    sizes
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn create_mip_views(device: &wgpu::Device, width: u32, height: u32) -> Vec<wgpu::TextureView> {
    let sizes = mip_sizes(width, height);
    sizes
        .into_iter()
        .enumerate()
        .map(|(i, (w, h))| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("Bloom Mip {} Texture", i)),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            texture.create_view(&wgpu::TextureViewDescriptor::default())
        })
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_sizes_1920_1080() {
        let sizes = mip_sizes(1920, 1080);
        assert_eq!(sizes.len(), BLOOM_MIP_COUNT);
        assert_eq!(sizes[0], (960, 540));
        assert_eq!(sizes[1], (480, 270));
        assert_eq!(sizes[2], (240, 135));
        assert_eq!(sizes[3], (120, 67));
        assert_eq!(sizes[4], (60, 33));
    }

    #[test]
    fn mip_sizes_64_64() {
        let sizes = mip_sizes(64, 64);
        assert_eq!(sizes.len(), BLOOM_MIP_COUNT);
        assert_eq!(sizes[0], (32, 32));
        assert_eq!(sizes[1], (16, 16));
        assert_eq!(sizes[2], (8, 8));
        assert_eq!(sizes[3], (4, 4));
        assert_eq!(sizes[4], (2, 2));
    }

    #[test]
    fn bloom_uniform_default() {
        let u = BloomUniform::default();
        assert!((u.threshold - 1.0).abs() < f32::EPSILON);
        assert!((u.soft_threshold - 0.5).abs() < f32::EPSILON);
        assert_eq!(u._padding, [0.0f32; 2]);
    }
}
