//! Off-screen render target used by the editor's 3D scene viewport — color +
//! depth attachments, sized to match the viewport panel.
#![allow(missing_docs)]

/// Color attachment format used by the viewport's offscreen target.
pub const VIEWPORT_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Depth attachment format used by the viewport's offscreen target.
pub const VIEWPORT_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Pair of color + depth offscreen textures plus a sampled view for compositing.
pub struct ViewportTexture {
    /// Color attachment (Rgba8Unorm).
    pub color_texture: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl ViewportTexture {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let w = width.max(1);
        let h = height.max(1);

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Color"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: VIEWPORT_COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Depth"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: VIEWPORT_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        ViewportTexture {
            color_texture,
            color_view,
            depth_texture,
            depth_view,
            width: w,
            height: h,
        }
    }

    pub fn ensure_size(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        let w = width.max(1);
        let h = height.max(1);
        if w == self.width && h == self.height {
            return false;
        }
        *self = Self::new(device, w, h);
        true
    }
}
