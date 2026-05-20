use crate::json_parser::JsonValue;
use super::types::GltfMaterial;

pub(super) fn extract_material(mat: &JsonValue) -> Option<GltfMaterial> {
    let pbr = mat.get("pbrMetallicRoughness")?;
    let base_color = if let Some(arr) = pbr.get("baseColorFactor").and_then(|v| v.as_array()) {
        [
            arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        ]
    } else {
        [1.0, 1.0, 1.0, 1.0]
    };
    let metallic = pbr.get("metallicFactor").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let roughness = pbr.get("roughnessFactor").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    Some(GltfMaterial { base_color, metallic, roughness })
}
