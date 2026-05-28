//! PBR mesh vertex — the geometry layout produced by the OBJ/glTF decoders.
//!
//! This is a plain POD struct with no GPU coupling. The wgpu vertex buffer
//! layout for it lives in `voltex_renderer` (`MESH_VERTEX_LAYOUT`).

use bytemuck::{Pod, Zeroable};

/// PBR-ready mesh vertex used by all the engine's mesh pipelines.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MeshVertex {
    /// Local-space position.
    pub position: [f32; 3],
    /// Surface normal (unit vector).
    pub normal: [f32; 3],
    /// UV coordinates `[u, v]`.
    pub uv: [f32; 2],
    /// Tangent vector + handedness `[tx, ty, tz, w]` for normal mapping.
    pub tangent: [f32; 4],
}
