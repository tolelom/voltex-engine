//! GPU-uploaded PBR material parameters.

use bytemuck::{Pod, Zeroable};

/// PBR material uniform — base color + metallic/roughness/AO factors.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MaterialUniform {
    /// Albedo / base color in linear space `[r, g, b, a]`.
    pub base_color: [f32; 4],
    /// Metallic factor in `[0.0, 1.0]`.
    pub metallic: f32,
    /// Roughness factor in `[0.0, 1.0]`.
    pub roughness: f32,
    /// Ambient-occlusion factor in `[0.0, 1.0]`.
    pub ao: f32,
    /// Padding to satisfy `std140` 16-byte alignment.
    pub _padding: f32,
}

impl Default for MaterialUniform {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialUniform {
    /// White, fully-rough, fully-occluded default (the "unlit" placeholder).
    pub fn new() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            ao: 1.0,
            _padding: 0.0,
        }
    }

    /// Helper to build a material with explicit base color + metallic + roughness.
    pub fn with_params(base_color: [f32; 4], metallic: f32, roughness: f32) -> Self {
        Self {
            base_color,
            metallic,
            roughness,
            ao: 1.0,
            _padding: 0.0,
        }
    }

    /// Bind group layout (group 1, binding 0) for the material UBO with a
    /// dynamic offset — supports drawing N entities from one shared buffer.
    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<MaterialUniform>() as u64,
                    ),
                },
                count: None,
            }],
        })
    }
}
