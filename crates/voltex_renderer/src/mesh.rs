use crate::vertex::MeshVertex;
use wgpu::util::DeviceExt;

pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl Mesh {
    pub fn new(device: &wgpu::Device, vertices: &[MeshVertex], indices: &[u32]) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, num_indices: indices.len() as u32 }
    }

    /// Create a mesh with additional buffer usage flags (e.g. `BLAS_INPUT` for ray tracing).
    pub fn new_with_usage(
        device: &wgpu::Device,
        vertices: &[MeshVertex],
        indices: &[u32],
        extra_usage: wgpu::BufferUsages,
    ) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX | extra_usage,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX | extra_usage,
        });
        Self { vertex_buffer, index_buffer, num_indices: indices.len() as u32 }
    }
}
