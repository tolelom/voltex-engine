use crate::vertex::MeshVertex;

pub struct GltfData {
    pub meshes: Vec<GltfMesh>,
    pub nodes: Vec<GltfNode>,
    pub skins: Vec<GltfSkin>,
    pub animations: Vec<GltfAnimation>,
}

pub struct GltfMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub name: Option<String>,
    pub material: Option<GltfMaterial>,
    pub joints: Option<Vec<[u16; 4]>>,
    pub weights: Option<Vec<[f32; 4]>>,
}

pub struct GltfMaterial {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
}

#[derive(Debug, Clone)]
pub struct GltfNode {
    pub name: Option<String>,
    pub children: Vec<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],   // quaternion [x,y,z,w]
    pub scale: [f32; 3],
    pub mesh: Option<usize>,
    pub skin: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct GltfSkin {
    pub name: Option<String>,
    pub joints: Vec<usize>,
    pub inverse_bind_matrices: Vec<[[f32; 4]; 4]>,
    pub skeleton: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct GltfAnimation {
    pub name: Option<String>,
    pub channels: Vec<GltfChannel>,
}

#[derive(Debug, Clone)]
pub struct GltfChannel {
    pub target_node: usize,
    pub target_path: AnimationPath,
    pub interpolation: Interpolation,
    pub times: Vec<f32>,
    pub values: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationPath {
    Translation,
    Rotation,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interpolation {
    Linear,
    Step,
    CubicSpline,
}

pub fn parse_animation_path(s: &str) -> AnimationPath {
    match s {
        "translation" => AnimationPath::Translation,
        "rotation" => AnimationPath::Rotation,
        "scale" => AnimationPath::Scale,
        _ => AnimationPath::Translation,
    }
}

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
        assert_eq!(parse_animation_path("translation"), AnimationPath::Translation);
        assert_eq!(parse_animation_path("rotation"), AnimationPath::Rotation);
        assert_eq!(parse_animation_path("scale"), AnimationPath::Scale);
    }

    #[test]
    fn test_parse_animation_path_unknown_defaults_to_translation() {
        // Documented silent-coercion: unknown/empty/mis-cased strings fall back
        // to Translation. Pinned so any future change is intentional.
        assert_eq!(parse_animation_path(""), AnimationPath::Translation);
        assert_eq!(parse_animation_path("weights"), AnimationPath::Translation);
        assert_eq!(parse_animation_path("Translation"), AnimationPath::Translation); // case-sensitive
        assert_eq!(parse_animation_path("ROTATION"), AnimationPath::Translation);
    }

    #[test]
    fn test_parse_interpolation_known() {
        assert_eq!(parse_interpolation("LINEAR"), Interpolation::Linear);
        assert_eq!(parse_interpolation("STEP"), Interpolation::Step);
        assert_eq!(parse_interpolation("CUBICSPLINE"), Interpolation::CubicSpline);
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
