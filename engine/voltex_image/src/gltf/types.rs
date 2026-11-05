//! Public glTF output types: `GltfData` and the mesh, material, node, skin and
//! animation records it holds, plus string parsers for animation path / interpolation.

use crate::vertex::MeshVertex;

/// All geometry, nodes, skins, and animations extracted from a glTF/GLB file.
pub struct GltfData {
    /// Decoded mesh primitives.
    pub meshes: Vec<GltfMesh>,
    /// Scene nodes (transform hierarchy).
    pub nodes: Vec<GltfNode>,
    /// Skeletal skins with inverse-bind matrices.
    pub skins: Vec<GltfSkin>,
    /// Keyframe animation tracks.
    pub animations: Vec<GltfAnimation>,
}

/// A single mesh primitive with geometry and an optional PBR material.
pub struct GltfMesh {
    /// Deduplicated vertex buffer.
    pub vertices: Vec<MeshVertex>,
    /// Triangle list indices into `vertices`.
    pub indices: Vec<u32>,
    /// Optional mesh name from the glTF asset.
    pub name: Option<String>,
    /// Optional PBR material parameters.
    pub material: Option<GltfMaterial>,
    /// Joint indices for skeletal animation (4 per vertex), if present.
    pub joints: Option<Vec<[u16; 4]>>,
    /// Joint weights for skeletal animation (4 per vertex), if present.
    pub weights: Option<Vec<[f32; 4]>>,
}

/// PBR metallic-roughness material parameters.
pub struct GltfMaterial {
    /// Base color factor `[r, g, b, a]`.
    pub base_color: [f32; 4],
    /// Metallic factor in `[0, 1]`.
    pub metallic: f32,
    /// Roughness factor in `[0, 1]`.
    pub roughness: f32,
}

/// A scene node carrying a local transform and optional mesh/skin references.
#[derive(Debug, Clone)]
pub struct GltfNode {
    /// Optional node name from the glTF asset.
    pub name: Option<String>,
    /// Indices of child nodes in the scene graph.
    pub children: Vec<usize>,
    /// Local translation `[x, y, z]`.
    pub translation: [f32; 3],
    /// Local rotation as a quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// Local scale `[x, y, z]`.
    pub scale: [f32; 3],
    /// Index of the mesh attached to this node, if any.
    pub mesh: Option<usize>,
    /// Index of the skin attached to this node, if any.
    pub skin: Option<usize>,
}

/// A skeletal skin with joint indices and inverse-bind matrices.
#[derive(Debug, Clone)]
pub struct GltfSkin {
    /// Optional skin name from the glTF asset.
    pub name: Option<String>,
    /// Ordered list of node indices that act as joints.
    pub joints: Vec<usize>,
    /// One 4×4 inverse-bind matrix per joint.
    pub inverse_bind_matrices: Vec<[[f32; 4]; 4]>,
    /// Root joint node index, if specified.
    pub skeleton: Option<usize>,
}

/// A named animation containing one or more channels.
#[derive(Debug, Clone)]
pub struct GltfAnimation {
    /// Optional animation name from the glTF asset.
    pub name: Option<String>,
    /// Keyframe channels that make up the animation.
    pub channels: Vec<GltfChannel>,
}

/// A single keyframe channel targeting one property of one node.
#[derive(Debug, Clone)]
pub struct GltfChannel {
    /// Index of the target node in [`GltfData::nodes`].
    pub target_node: usize,
    /// Which transform property this channel animates.
    pub target_path: AnimationPath,
    /// Interpolation method between keyframes.
    pub interpolation: Interpolation,
    /// Keyframe timestamps in seconds.
    pub times: Vec<f32>,
    /// Flattened keyframe values (stride depends on `target_path`).
    pub values: Vec<f32>,
}

/// Which node transform property an animation channel targets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationPath {
    /// Animates the node's local translation `[x, y, z]`.
    Translation,
    /// Animates the node's local rotation quaternion `[x, y, z, w]`.
    Rotation,
    /// Animates the node's local scale `[x, y, z]`.
    Scale,
}

/// Interpolation method between animation keyframes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interpolation {
    /// Linear interpolation between adjacent keyframes.
    Linear,
    /// Step (constant) interpolation — holds the previous keyframe value.
    Step,
    /// Cubic spline interpolation using in- and out-tangents.
    CubicSpline,
}

/// Parse an animation path string from glTF JSON into an [`AnimationPath`].
///
/// Unknown or mis-cased strings fall back silently to [`AnimationPath::Translation`].
pub fn parse_animation_path(s: &str) -> AnimationPath {
    match s {
        "translation" => AnimationPath::Translation,
        "rotation" => AnimationPath::Rotation,
        "scale" => AnimationPath::Scale,
        _ => AnimationPath::Translation,
    }
}

/// Parse an interpolation string from glTF JSON into an [`Interpolation`].
///
/// Unknown or mis-cased strings fall back silently to [`Interpolation::Linear`].
pub fn parse_interpolation(s: &str) -> Interpolation {
    match s {
        "LINEAR" => Interpolation::Linear,
        "STEP" => Interpolation::Step,
        "CUBICSPLINE" => Interpolation::CubicSpline,
        _ => Interpolation::Linear,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_animation_path_known() {
        assert_eq!(
            parse_animation_path("translation"),
            AnimationPath::Translation
        );
        assert_eq!(parse_animation_path("rotation"), AnimationPath::Rotation);
        assert_eq!(parse_animation_path("scale"), AnimationPath::Scale);
    }

    #[test]
    fn test_parse_animation_path_unknown_defaults_to_translation() {
        // Documented silent-coercion: unknown/empty/mis-cased strings fall back
        // to Translation. Pinned so any future change is intentional.
        assert_eq!(parse_animation_path(""), AnimationPath::Translation);
        assert_eq!(parse_animation_path("weights"), AnimationPath::Translation);
        assert_eq!(
            parse_animation_path("Translation"),
            AnimationPath::Translation
        ); // case-sensitive
        assert_eq!(parse_animation_path("ROTATION"), AnimationPath::Translation);
    }

    #[test]
    fn test_parse_interpolation_known() {
        assert_eq!(parse_interpolation("LINEAR"), Interpolation::Linear);
        assert_eq!(parse_interpolation("STEP"), Interpolation::Step);
        assert_eq!(
            parse_interpolation("CUBICSPLINE"),
            Interpolation::CubicSpline
        );
    }

    #[test]
    fn test_parse_interpolation_unknown_defaults_to_linear() {
        // Documented silent-coercion: unknown/empty/mis-cased strings fall back
        // to Linear. Pinned so any future change is intentional.
        assert_eq!(parse_interpolation(""), Interpolation::Linear);
        assert_eq!(parse_interpolation("linear"), Interpolation::Linear); // case-sensitive
        assert_eq!(parse_interpolation("CATMULLROM"), Interpolation::Linear);
        assert_eq!(parse_interpolation("Step"), Interpolation::Linear);
    }
}
