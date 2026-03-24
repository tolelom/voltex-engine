use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub ao: f32,
    pub _padding: f32,
}

impl MaterialUniform {
    pub fn new() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            ao: 1.0,
            _padding: 0.0,
        }
    }

    pub fn with_params(base_color: [f32; 4], metallic: f32, roughness: f32) -> Self {
        Self {
            base_color,
            metallic,
            roughness,
            ao: 1.0,
            _padding: 0.0,
        }
    }

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
