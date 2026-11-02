#![allow(missing_docs)]

use bytemuck::{Pod, Zeroable};

/// Texture format used for the RT shadow output (single-channel float).
pub const RT_SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

/// Uniform buffer for the RT shadow compute pass.
///
/// Layout (32 bytes, 16-byte aligned):
///   light_direction [f32; 3] + _pad0: f32   → 16 bytes
///   width: u32, height: u32, _pad1: [u32; 2] → 16 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct RtShadowUniform {
    pub light_direction: [f32; 3],
    pub _pad0: f32,
    pub width: u32,
    pub height: u32,
    pub _pad1: [u32; 2],
}

/// GPU resources for the RT shadow compute pass.
pub struct RtShadowResources {
    pub shadow_texture: wgpu::Texture,
    pub shadow_view: wgpu::TextureView,
    pub uniform_buffer: wgpu::Buffer,
    pub width: u32,
    pub height: u32,
}

impl RtShadowResources {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let (shadow_texture, shadow_view) = create_shadow_texture(device, width, height);

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RT Shadow Uniform Buffer"),
            size: std::mem::size_of::<RtShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            shadow_texture,
            shadow_view,
            uniform_buffer,
            width,
            height,
        }
    }

    /// Recreate the shadow texture when the window is resized.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let (shadow_texture, shadow_view) = create_shadow_texture(device, width, height);
        self.shadow_texture = shadow_texture;
        self.shadow_view = shadow_view;
        self.width = width;
        self.height = height;
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn create_shadow_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("RT Shadow Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: RT_SHADOW_FORMAT,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rt_shadow_uniform_size() {
        assert_eq!(std::mem::size_of::<RtShadowUniform>(), 32);
    }
}
