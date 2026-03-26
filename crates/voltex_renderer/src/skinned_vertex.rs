use bytemuck::{Pod, Zeroable};

/// Vertex for skinned meshes: standard vertex + joint indices + weights.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SkinnedMeshVertex {
    pub position: [f32; 3],    // 12
    pub normal: [f32; 3],      // 12
    pub uv: [f32; 2],          // 8
    pub tangent: [f32; 4],     // 16
    pub joints: [u32; 4],      // 16 (u32 for shader compatibility, actual values are u16 range)
    pub weights: [f32; 4],     // 16
}
// Total: 80 bytes

impl SkinnedMeshVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<SkinnedMeshVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32x3 },  // position
            wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 },  // normal
            wgpu::VertexAttribute { offset: 24, shader_location: 2, format: wgpu::VertexFormat::Float32x2 },  // uv
            wgpu::VertexAttribute { offset: 32, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },  // tangent
            wgpu::VertexAttribute { offset: 48, shader_location: 4, format: wgpu::VertexFormat::Uint32x4 },   // joints
            wgpu::VertexAttribute { offset: 64, shader_location: 5, format: wgpu::VertexFormat::Float32x4 },  // weights
        ],
    };
}

/// Maximum number of bones per skeleton.
pub const MAX_BONES: usize = 128;

/// Bone matrix palette: array of mat4 for GPU upload.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct BonePalette {
    pub matrices: [[[f32; 4]; 4]; MAX_BONES],
}

impl BonePalette {
    pub fn new() -> Self {
        let identity = [[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]];
        BonePalette { matrices: [identity; MAX_BONES] }
    }

    pub fn set_bone(&mut self, index: usize, matrix: [[f32; 4]; 4]) {
        if index < MAX_BONES { self.matrices[index] = matrix; }
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
        let custom = [[2.0,0.0,0.0,0.0],[0.0,2.0,0.0,0.0],[0.0,0.0,2.0,0.0],[0.0,0.0,0.0,1.0]];
        palette.set_bone(5, custom);
        assert!((palette.matrices[5][0][0] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_bone_palette_size() {
        let size = std::mem::size_of::<BonePalette>();
        assert_eq!(size, MAX_BONES * 64); // 128 * 64 = 8192
    }

    #[test]
    fn test_skinned_vertex_pod() {
        let v = SkinnedMeshVertex {
            position: [0.0; 3], normal: [0.0, 1.0, 0.0], uv: [0.0; 2],
            tangent: [1.0, 0.0, 0.0, 1.0], joints: [0, 1, 0, 0], weights: [0.5, 0.5, 0.0, 0.0],
        };
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        assert_eq!(bytes.len(), 80);
    }
}
