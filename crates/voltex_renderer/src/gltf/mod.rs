//! glTF 2.0 / GLB parser — vertex/material/animation extraction.
#![allow(missing_docs)]

mod types;
mod parser;
mod accessor;
mod node;
mod material;

pub use types::{
    GltfData, GltfMesh, GltfMaterial, GltfNode, GltfSkin, GltfAnimation,
    GltfChannel, AnimationPath, Interpolation, parse_animation_path, parse_interpolation,
};
pub use parser::parse_gltf;

#[cfg(test)]
mod tests {
    use super::*;
    use super::parser::decode_base64;
    use super::accessor::{read_floats, read_indices_u16};

    #[test]
    fn test_glb_header_magic() {
        // Invalid magic
        let data = [0u8; 12];
        assert!(parse_gltf(&data).is_err());
    }

    #[test]
    fn test_glb_header_version() {
        // Valid magic but wrong version
        let mut data = Vec::new();
        data.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic "glTF"
        data.extend_from_slice(&1u32.to_le_bytes()); // version 1 (we need 2)
        data.extend_from_slice(&12u32.to_le_bytes()); // length
        assert!(parse_gltf(&data).is_err());
    }

    #[test]
    fn test_base64_decode() {
        let encoded = "SGVsbG8="; // "Hello"
        let decoded = decode_base64(encoded).unwrap();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn test_base64_decode_no_padding() {
        let encoded = "SGVsbG8"; // "Hello" without padding
        let decoded = decode_base64(encoded).unwrap();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn test_read_f32_accessor() {
        // Simulate a buffer with 3 float32 values
        let buffer: Vec<u8> = [1.0f32, 2.0, 3.0].iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let data = read_floats(&buffer, 0, 3);
        assert_eq!(data, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_read_u16_indices() {
        let buffer: Vec<u8> = [0u16, 1, 2].iter()
            .flat_map(|i| i.to_le_bytes())
            .collect();
        let indices = read_indices_u16(&buffer, 0, 3);
        assert_eq!(indices, vec![0u32, 1, 2]);
    }

    #[test]
    fn test_parse_minimal_glb() {
        let glb = build_minimal_glb_triangle();
        let data = parse_gltf(&glb).unwrap();
        assert_eq!(data.meshes.len(), 1);
        let mesh = &data.meshes[0];
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.indices.len(), 3);
        // Verify positions
        assert_eq!(mesh.vertices[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(mesh.vertices[1].position, [1.0, 0.0, 0.0]);
        assert_eq!(mesh.vertices[2].position, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn test_parse_glb_with_material() {
        let glb = build_glb_with_material();
        let data = parse_gltf(&glb).unwrap();
        let mesh = &data.meshes[0];
        let mat = mesh.material.as_ref().unwrap();
        assert!((mat.base_color[0] - 1.0).abs() < 0.01);
        assert!((mat.metallic - 0.5).abs() < 0.01);
        assert!((mat.roughness - 0.8).abs() < 0.01);
    }

    /// Build a minimal GLB with one triangle.
    fn build_minimal_glb_triangle() -> Vec<u8> {
        // Binary buffer: 3 positions (vec3) + 3 indices (u16)
        let mut bin = Vec::new();
        // Positions: 3 * vec3 = 36 bytes
        for &v in &[0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        // Indices: 3 * u16 = 6 bytes + 2 padding = 8 bytes
        for &i in &[0u16, 1, 2] {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        bin.extend_from_slice(&[0, 0]); // padding to 4-byte alignment

        let json_str = format!(r#"{{
            "asset": {{"version": "2.0"}},
            "buffers": [{{"byteLength": {}}}],
            "bufferViews": [
                {{"buffer": 0, "byteOffset": 0, "byteLength": 36}},
                {{"buffer": 0, "byteOffset": 36, "byteLength": 6}}
            ],
            "accessors": [
                {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                  "max": [1.0, 1.0, 0.0], "min": [0.0, 0.0, 0.0]}},
                {{"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"}}
            ],
            "meshes": [{{
                "name": "Triangle",
                "primitives": [{{
                    "attributes": {{"POSITION": 0}},
                    "indices": 1
                }}]
            }}]
        }}"#, bin.len());

        let json_bytes = json_str.as_bytes();
        // Pad JSON to 4-byte alignment
        let json_padded_len = (json_bytes.len() + 3) & !3;
        let mut json_padded = json_bytes.to_vec();
        while json_padded.len() < json_padded_len {
            json_padded.push(b' ');
        }

        let total_len = 12 + 8 + json_padded.len() + 8 + bin.len();
        let mut glb = Vec::with_capacity(total_len);

        // Header
        glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic
        glb.extend_from_slice(&2u32.to_le_bytes()); // version
        glb.extend_from_slice(&(total_len as u32).to_le_bytes());

        // JSON chunk
        glb.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
        glb.extend_from_slice(&json_padded);

        // BIN chunk
        glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
        glb.extend_from_slice(&bin);

        glb
    }

    #[test]
    fn test_animation_path_parsing() {
        assert_eq!(parse_animation_path("translation"), AnimationPath::Translation);
        assert_eq!(parse_animation_path("rotation"), AnimationPath::Rotation);
        assert_eq!(parse_animation_path("scale"), AnimationPath::Scale);
    }

    #[test]
    fn test_interpolation_parsing() {
        assert_eq!(parse_interpolation("LINEAR"), Interpolation::Linear);
        assert_eq!(parse_interpolation("STEP"), Interpolation::Step);
        assert_eq!(parse_interpolation("CUBICSPLINE"), Interpolation::CubicSpline);
    }

    #[test]
    fn test_gltf_data_has_new_fields() {
        let glb = build_minimal_glb_triangle();
        let data = parse_gltf(&glb).unwrap();
        assert_eq!(data.meshes.len(), 1);
        // No nodes/skins/animations in minimal GLB — should be empty, not crash
        assert!(data.nodes.is_empty());
        assert!(data.skins.is_empty());
        assert!(data.animations.is_empty());
        // Joints/weights should be None
        assert!(data.meshes[0].joints.is_none());
        assert!(data.meshes[0].weights.is_none());
    }

    #[test]
    fn test_parse_glb_with_node() {
        let glb = build_glb_with_node();
        let data = parse_gltf(&glb).unwrap();
        assert_eq!(data.meshes.len(), 1);
        assert_eq!(data.nodes.len(), 1);
        let node = &data.nodes[0];
        assert_eq!(node.name.as_deref(), Some("RootNode"));
        assert_eq!(node.mesh, Some(0));
        assert!((node.translation[0] - 1.0).abs() < 0.001);
        assert!((node.translation[1] - 2.0).abs() < 0.001);
        assert!((node.translation[2] - 3.0).abs() < 0.001);
        assert_eq!(node.scale, [1.0, 1.0, 1.0]);
    }

    fn build_glb_with_node() -> Vec<u8> {
        let mut bin = Vec::new();
        for &v in &[0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        for &i in &[0u16, 1, 2] {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        bin.extend_from_slice(&[0, 0]);

        let json_str = format!(r#"{{
            "asset": {{"version": "2.0"}},
            "buffers": [{{"byteLength": {}}}],
            "bufferViews": [
                {{"buffer": 0, "byteOffset": 0, "byteLength": 36}},
                {{"buffer": 0, "byteOffset": 36, "byteLength": 6}}
            ],
            "accessors": [
                {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                  "max": [1.0, 1.0, 0.0], "min": [0.0, 0.0, 0.0]}},
                {{"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"}}
            ],
            "nodes": [{{
                "name": "RootNode",
                "mesh": 0,
                "translation": [1.0, 2.0, 3.0]
            }}],
            "meshes": [{{
                "name": "Triangle",
                "primitives": [{{
                    "attributes": {{"POSITION": 0}},
                    "indices": 1
                }}]
            }}]
        }}"#, bin.len());

        let json_bytes = json_str.as_bytes();
        let json_padded_len = (json_bytes.len() + 3) & !3;
        let mut json_padded = json_bytes.to_vec();
        while json_padded.len() < json_padded_len {
            json_padded.push(b' ');
        }

        let total_len = 12 + 8 + json_padded.len() + 8 + bin.len();
        let mut glb = Vec::with_capacity(total_len);
        glb.extend_from_slice(&0x46546C67u32.to_le_bytes());
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&(total_len as u32).to_le_bytes());
        glb.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
        glb.extend_from_slice(&json_padded);
        glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004E4942u32.to_le_bytes());
        glb.extend_from_slice(&bin);
        glb
    }

    /// Build a GLB with one triangle and a material.
    fn build_glb_with_material() -> Vec<u8> {
        let mut bin = Vec::new();
        for &v in &[0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        for &i in &[0u16, 1, 2] {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        bin.extend_from_slice(&[0, 0]); // padding

        let json_str = format!(r#"{{
            "asset": {{"version": "2.0"}},
            "buffers": [{{"byteLength": {}}}],
            "bufferViews": [
                {{"buffer": 0, "byteOffset": 0, "byteLength": 36}},
                {{"buffer": 0, "byteOffset": 36, "byteLength": 6}}
            ],
            "accessors": [
                {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                  "max": [1.0, 1.0, 0.0], "min": [0.0, 0.0, 0.0]}},
                {{"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"}}
            ],
            "materials": [{{
                "pbrMetallicRoughness": {{
                    "baseColorFactor": [1.0, 0.0, 0.0, 1.0],
                    "metallicFactor": 0.5,
                    "roughnessFactor": 0.8
                }}
            }}],
            "meshes": [{{
                "name": "Triangle",
                "primitives": [{{
                    "attributes": {{"POSITION": 0}},
                    "indices": 1,
                    "material": 0
                }}]
            }}]
        }}"#, bin.len());

        let json_bytes = json_str.as_bytes();
        let json_padded_len = (json_bytes.len() + 3) & !3;
        let mut json_padded = json_bytes.to_vec();
        while json_padded.len() < json_padded_len {
            json_padded.push(b' ');
        }

        let total_len = 12 + 8 + json_padded.len() + 8 + bin.len();
        let mut glb = Vec::with_capacity(total_len);

        glb.extend_from_slice(&0x46546C67u32.to_le_bytes());
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&(total_len as u32).to_le_bytes());

        glb.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
        glb.extend_from_slice(&json_padded);

        glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004E4942u32.to_le_bytes());
        glb.extend_from_slice(&bin);

        glb
    }

    // ── Panic-resistance fuzz tests ────────────────────────────────
    //
    // parse_gltf must return Result on any input — never panic on
    // truncated GLB headers, corrupted chunk lengths, or JSON garbage.

    fn fuzz_bytes(seed: u32, len: usize) -> Vec<u8> {
        let mut s: u32 = if seed == 0 { 0xDEAD_BEEF } else { seed };
        (0..len)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s & 0xFF) as u8
            })
            .collect()
    }

    #[test]
    fn fuzz_parse_gltf_no_panic_on_empty() {
        let _ = parse_gltf(&[]);
    }

    #[test]
    fn fuzz_parse_gltf_no_panic_on_truncated_header() {
        let header_full: [u8; 12] = [
            0x67, 0x6C, 0x54, 0x46, // magic "glTF"
            0x02, 0x00, 0x00, 0x00, // version 2
            0x0C, 0x00, 0x00, 0x00, // length 12
        ];
        for n in 0..=header_full.len() {
            let _ = parse_gltf(&header_full[..n]);
        }
    }

    #[test]
    fn fuzz_parse_gltf_no_panic_on_random_bytes() {
        for seed in 1..=128u32 {
            for &len in &[0usize, 1, 12, 32, 128, 512, 2048] {
                let _ = parse_gltf(&fuzz_bytes(seed, len));
            }
        }
    }

    #[test]
    fn fuzz_parse_gltf_no_panic_on_valid_magic_random_tail() {
        let magic: [u8; 4] = [0x67, 0x6C, 0x54, 0x46];
        for seed in 1..=64u32 {
            for &tail_len in &[0usize, 4, 16, 64, 256, 1024] {
                let mut data = magic.to_vec();
                data.extend_from_slice(&fuzz_bytes(seed, tail_len));
                let _ = parse_gltf(&data);
            }
        }
    }

    #[test]
    fn fuzz_parse_gltf_no_panic_on_truncated_valid_glb() {
        // Take a valid GLB and feed every prefix of it.
        let valid = build_minimal_glb_triangle();
        for n in 0..=valid.len() {
            let _ = parse_gltf(&valid[..n]);
        }
    }

    #[test]
    fn parse_gltf_rejects_huge_accessor_count() {
        // Build a GLB whose POSITION accessor claims count = u32::MAX. Pre-fix,
        // `Vec::with_capacity(u32::MAX)` for vec3 = ~48 GB attempted allocation.
        let mut bin = vec![0u8; 36];
        bin.push(0); bin.push(0); // padding
        let json_str = format!(r#"{{
            "asset": {{"version": "2.0"}},
            "buffers": [{{"byteLength": {}}}],
            "bufferViews": [
                {{"buffer": 0, "byteOffset": 0, "byteLength": 36}}
            ],
            "accessors": [
                {{"bufferView": 0, "componentType": 5126, "count": 4294967295, "type": "VEC3"}}
            ],
            "meshes": [{{
                "primitives": [{{"attributes": {{"POSITION": 0}}}}]
            }}]
        }}"#, bin.len());
        let glb = build_glb(&json_str, &bin);
        let err = parse_gltf(&glb).err().expect("expected error").to_string();
        assert!(err.contains("exceeds limit"), "unexpected error: {err}");
    }

    #[test]
    fn parse_gltf_rejects_attribute_length_mismatch() {
        // Build a GLB where POSITION has 3 elements but NORMAL has 5.
        // Pre-fix, the vertex assembly loop indexed normals[3]/normals[4]
        // out of bounds.
        let mut bin = Vec::new();
        // 3 positions (36 bytes)
        for &v in &[0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        // 5 normals (60 bytes)
        for _ in 0..5 {
            for &v in &[0.0f32, 1.0, 0.0] {
                bin.extend_from_slice(&v.to_le_bytes());
            }
        }
        let json_str = format!(r#"{{
            "asset": {{"version": "2.0"}},
            "buffers": [{{"byteLength": {}}}],
            "bufferViews": [
                {{"buffer": 0, "byteOffset": 0, "byteLength": 36}},
                {{"buffer": 0, "byteOffset": 36, "byteLength": 60}}
            ],
            "accessors": [
                {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}},
                {{"bufferView": 1, "componentType": 5126, "count": 5, "type": "VEC3"}}
            ],
            "meshes": [{{
                "primitives": [{{
                    "attributes": {{"POSITION": 0, "NORMAL": 1}}
                }}]
            }}]
        }}"#, bin.len());
        let glb = build_glb(&json_str, &bin);
        let err = parse_gltf(&glb).err().expect("expected error").to_string();
        assert!(err.contains("attribute length mismatch"), "unexpected error: {err}");
    }

    /// Helper to wrap arbitrary JSON + BIN payloads into a GLB container.
    fn build_glb(json_str: &str, bin: &[u8]) -> Vec<u8> {
        let json_bytes = json_str.as_bytes();
        let json_padded_len = (json_bytes.len() + 3) & !3;
        let mut json_padded = json_bytes.to_vec();
        while json_padded.len() < json_padded_len {
            json_padded.push(b' ');
        }
        // BIN chunk must be 4-byte aligned.
        let bin_padded_len = (bin.len() + 3) & !3;
        let mut bin_padded = bin.to_vec();
        while bin_padded.len() < bin_padded_len {
            bin_padded.push(0);
        }
        let total_len = 12 + 8 + json_padded.len() + 8 + bin_padded.len();
        let mut glb = Vec::with_capacity(total_len);
        glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic
        glb.extend_from_slice(&2u32.to_le_bytes()); // version
        glb.extend_from_slice(&(total_len as u32).to_le_bytes());
        glb.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // JSON
        glb.extend_from_slice(&json_padded);
        glb.extend_from_slice(&(bin_padded.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // BIN
        glb.extend_from_slice(&bin_padded);
        glb
    }
}
