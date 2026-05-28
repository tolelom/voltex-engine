#![allow(missing_docs)]

use crate::gpu::DEPTH_FORMAT;

pub const GBUFFER_POSITION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
pub const GBUFFER_NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const GBUFFER_ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
pub const GBUFFER_MATERIAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn create_rt(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("GBuffer Depth Texture"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

pub struct GBuffer {
    pub position_view: wgpu::TextureView,
    pub normal_view: wgpu::TextureView,
    pub albedo_view: wgpu::TextureView,
    pub material_view: wgpu::TextureView,
    pub depth_view: wgpu::TextureView,
}

impl GBuffer {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let position_view = create_rt(
            device,
            width,
            height,
            GBUFFER_POSITION_FORMAT,
            "GBuffer Position",
        );
        let normal_view = create_rt(
            device,
            width,
            height,
            GBUFFER_NORMAL_FORMAT,
            "GBuffer Normal",
        );
        let albedo_view = create_rt(
            device,
            width,
            height,
            GBUFFER_ALBEDO_FORMAT,
            "GBuffer Albedo",
        );
        let material_view = create_rt(
            device,
            width,
            height,
            GBUFFER_MATERIAL_FORMAT,
            "GBuffer Material",
        );
        let depth_view = create_depth(device, width, height);

        Self {
            position_view,
            normal_view,
            albedo_view,
            material_view,
            depth_view,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = Self::new(device, width, height);
    }
}
