//! Skinned mesh data: `SkinnedMeshVertex` (vertex + joints/weights), building it from
//! glTF, the GPU `SkinnedMesh`, `SkinnedCameraUniform`, and the `BonePalette` joint-matrix UBO.
#![allow(missing_docs)]

use bytemuck::{Pod, Zeroable};

/// Vertex for skinned meshes: standard vertex + joint indices + weights.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SkinnedMeshVertex {
    pub position: [f32; 3], // 12
    pub normal: [f32; 3],   // 12
    pub uv: [f32; 2],       // 8
    pub tangent: [f32; 4],  // 16
    pub joints: [u32; 4],   // 16 (u32 for shader compatibility, actual values are u16 range)
    pub weights: [f32; 4],  // 16
}
// Total: 80 bytes

impl SkinnedMeshVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<SkinnedMeshVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }, // position
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            }, // normal
            wgpu::VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            }, // uv
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            }, // tangent
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 4,
                format: wgpu::VertexFormat::Uint32x4,
            }, // joints
            wgpu::VertexAttribute {
                offset: 64,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32x4,
            }, // weights
        ],
    };
}

/// Assemble [`SkinnedMeshVertex`]es from a parsed glTF mesh primitive: combine
/// its base vertices with the `JOINTS_0` / `WEIGHTS_0` attributes. Vertices
/// without joint data default to "fully bound to joint 0" (weight 1), so a
/// non-skinned mesh renders undeformed through the skinned pipeline.
pub fn skinned_vertices_from_gltf(mesh: &crate::gltf::GltfMesh) -> Vec<SkinnedMeshVertex> {
    mesh.vertices
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let j = mesh
                .joints
                .as_ref()
                .and_then(|js| js.get(i))
                .copied()
                .unwrap_or([0, 0, 0, 0]);
            let w = mesh
                .weights
                .as_ref()
                .and_then(|ws| ws.get(i))
                .copied()
                .unwrap_or([1.0, 0.0, 0.0, 0.0]);
            SkinnedMeshVertex {
                position: v.position,
                normal: v.normal,
                uv: v.uv,
                tangent: v.tangent,
                joints: [j[0] as u32, j[1] as u32, j[2] as u32, j[3] as u32],
                weights: w,
            }
        })
        .collect()
}

/// GPU-resident skinned mesh — interleaved [`SkinnedMeshVertex`] buffer + 32-bit
/// index buffer. The skinned analogue of [`crate::Mesh`].
pub struct SkinnedMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl SkinnedMesh {
    /// Upload `vertices` (with joint indices + weights) and `indices` to GPU.
    pub fn new(device: &wgpu::Device, vertices: &[SkinnedMeshVertex], indices: &[u32]) -> Self {
        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SkinnedMesh Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SkinnedMesh Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }
}

/// Per-entity camera uniform for the skinned pipeline (`mesh_skinned.wgsl`
/// group 0): view-projection + model. 128 bytes (two mat4), 256-align safe.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SkinnedCameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
}

/// Maximum number of bones per skeleton.
pub const MAX_BONES: usize = 128;

/// Bone matrix palette: array of mat4 for GPU upload.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct BonePalette {
    pub matrices: [[[f32; 4]; 4]; MAX_BONES],
}

impl Default for BonePalette {
    fn default() -> Self {
        Self::new()
    }
}

impl BonePalette {
    pub fn new() -> Self {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        BonePalette {
            matrices: [identity; MAX_BONES],
        }
    }

    pub fn set_bone(&mut self, index: usize, matrix: [[f32; 4]; 4]) {
        if index < MAX_BONES {
            self.matrices[index] = matrix;
        }
    }
}

// Copy needs to be derived manually or not at all since it's large
// BonePalette is 128 * 64 = 8192 bytes — too large for Copy

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skinned_vertex_size() {
        assert_eq!(std::mem::size_of::<SkinnedMeshVertex>(), 80);
    }

    #[test]
    fn skinned_vertices_from_gltf_combines_joints_and_weights() {
        use crate::gltf::GltfMesh;
        use crate::MeshVertex;
        let mesh = GltfMesh {
            vertices: vec![MeshVertex {
                position: [1.0, 2.0, 3.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.25, 0.75],
                tangent: [1.0, 0.0, 0.0, 1.0],
            }],
            indices: vec![0],
            name: None,
            material: None,
            joints: Some(vec![[0u16, 1, 2, 3]]),
            weights: Some(vec![[0.5f32, 0.3, 0.2, 0.0]]),
        };
        let sv = skinned_vertices_from_gltf(&mesh);
        assert_eq!(sv.len(), 1);
        assert_eq!(sv[0].joints, [0, 1, 2, 3]);
        assert_eq!(sv[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(sv[0].uv, [0.25, 0.75]);
        assert!((sv[0].weights[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn skinned_vertices_default_when_no_joint_data() {
        use crate::gltf::GltfMesh;
        use crate::MeshVertex;
        let mesh = GltfMesh {
            vertices: vec![MeshVertex {
                position: [0.0; 3],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0; 2],
                tangent: [1.0, 0.0, 0.0, 1.0],
            }],
            indices: vec![0],
            name: None,
            material: None,
            joints: None,
            weights: None,
        };
        let sv = skinned_vertices_from_gltf(&mesh);
        assert_eq!(sv[0].joints, [0, 0, 0, 0]);
        assert_eq!(sv[0].weights, [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_skinned_vertex_layout() {
        assert_eq!(SkinnedMeshVertex::LAYOUT.attributes.len(), 6);
        assert_eq!(SkinnedMeshVertex::LAYOUT.array_stride, 80);
    }

    #[test]
    fn test_bone_palette_identity() {
        let palette = BonePalette::new();
        // First bone should be identity
        assert!((palette.matrices[0][0][0] - 1.0).abs() < 1e-6);
        assert!((palette.matrices[0][1][1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_bone_palette_set() {
        let mut palette = BonePalette::new();
        let custom = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 2.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        palette.set_bone(5, custom);
        assert!((palette.matrices[5][0][0] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_bone_palette_set_out_of_bounds_is_ignored() {
        // index == MAX_BONES is out of range; set_bone silently drops it
        // (no panic, no mutation). The last in-range slot is MAX_BONES - 1.
        let mut palette = BonePalette::new();
        let custom = [
            [7.0, 0.0, 0.0, 0.0],
            [0.0, 7.0, 0.0, 0.0],
            [0.0, 0.0, 7.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];

        // Out-of-bounds write is a no-op: last valid slot stays identity.
        palette.set_bone(MAX_BONES, custom);
        assert!((palette.matrices[MAX_BONES - 1][0][0] - 1.0).abs() < 1e-6);

        // In-range write at the boundary index does take effect.
        palette.set_bone(MAX_BONES - 1, custom);
        assert!((palette.matrices[MAX_BONES - 1][0][0] - 7.0).abs() < 1e-6);
        assert!((palette.matrices[MAX_BONES - 1][1][1] - 7.0).abs() < 1e-6);
    }

    #[test]
    fn test_bone_palette_size() {
        let size = std::mem::size_of::<BonePalette>();
        assert_eq!(size, MAX_BONES * 64); // 128 * 64 = 8192
    }

    #[test]
    fn test_skinned_vertex_pod() {
        let v = SkinnedMeshVertex {
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0; 2],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joints: [0, 1, 0, 0],
            weights: [0.5, 0.5, 0.0, 0.0],
        };
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        assert_eq!(bytes.len(), 80);
    }
}
