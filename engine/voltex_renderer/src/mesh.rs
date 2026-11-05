//! GPU-resident triangle mesh — interleaved [`MeshVertex`] buffer + 32-bit index buffer.

use crate::vertex::MeshVertex;
use wgpu::util::DeviceExt;

/// One GPU-resident mesh — vertex buffer + index buffer + index count.
pub struct Mesh {
    /// Vertex buffer containing interleaved [`MeshVertex`] data.
    pub vertex_buffer: wgpu::Buffer,
    /// 32-bit index buffer.
    pub index_buffer: wgpu::Buffer,
    /// Number of indices to draw (`vkCmdDrawIndexed.indexCount`).
    pub num_indices: u32,
}

impl Mesh {
    /// Uploads `vertices` + `indices` to GPU buffers tagged for typical mesh use.
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
        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }

    /// Parse a Wavefront OBJ source string and upload it as a GPU mesh.
    /// Convenience over `parse_obj` + [`Mesh::new`]. The caller reads the file
    /// (from disk / VFS) and passes its text.
    pub fn from_obj(device: &wgpu::Device, source: &str) -> Self {
        let data = crate::obj::parse_obj(source);
        Mesh::new(device, &data.vertices, &data.indices)
    }

    /// Parse a glTF/GLB byte buffer and upload its **first** mesh primitive as
    /// a GPU mesh. Convenience over [`parse_gltf`](crate::parse_gltf) +
    /// [`Mesh::new`]. Returns the decoder error / "no meshes" on failure.
    /// (Materials, skins, animations, and additional primitives are ignored
    /// here — use `parse_gltf` directly for those.)
    pub fn from_gltf(device: &wgpu::Device, bytes: &[u8]) -> Result<Self, String> {
        let data = crate::parse_gltf(bytes).map_err(|e| e.to_string())?;
        let prim = data.meshes.first().ok_or("glTF has no meshes")?;
        Ok(Mesh::new(device, &prim.vertices, &prim.indices))
    }

    /// An axis-aligned cube of edge length `size`, centered at the origin.
    /// One-call convenience over [`generate_cube_mesh`](crate::generate_cube_mesh).
    pub fn cube(device: &wgpu::Device, size: f32) -> Self {
        crate::generate_cube_mesh(size).upload(device)
    }

    /// A flat `size`×`size` ground quad on the XZ plane (`+Y` normal). One-call
    /// convenience over [`generate_plane_mesh`](crate::generate_plane_mesh).
    pub fn plane(device: &wgpu::Device, size: f32) -> Self {
        crate::generate_plane_mesh(size).upload(device)
    }

    /// A `width`×`height` quad on the XY plane facing `+Z`. One-call convenience
    /// over [`generate_quad_mesh`](crate::generate_quad_mesh).
    pub fn quad(device: &wgpu::Device, width: f32, height: f32) -> Self {
        crate::generate_quad_mesh(width, height).upload(device)
    }

    /// A UV sphere of `radius` with `sectors`×`stacks` tessellation. One-call
    /// convenience over [`generate_sphere_mesh`](crate::generate_sphere_mesh).
    pub fn sphere(device: &wgpu::Device, radius: f32, sectors: u32, stacks: u32) -> Self {
        crate::generate_sphere_mesh(radius, sectors, stacks).upload(device)
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
        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }
}
