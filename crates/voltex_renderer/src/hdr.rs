#![allow(missing_docs)]

/// Texture format used for HDR render targets.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// An HDR render target (Rgba16Float) used as the output of the lighting pass.
pub struct HdrTarget {
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl HdrTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let view = create_hdr_view(device, width, height);
        Self { view, width, height }
    }

    /// Recreate the HDR texture when the window is resized.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.view = create_hdr_view(device, width, height);
        self.width = width;
        self.height = height;
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn create_hdr_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("HDR Target Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
