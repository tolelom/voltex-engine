#![allow(missing_docs)]

/// Octahedral normal encoding: vec3 normal → vec2 (compact).
pub fn encode_octahedral(n: [f32; 3]) -> [f32; 2] {
    let sum = n[0].abs() + n[1].abs() + n[2].abs();
    let mut oct = [n[0] / sum, n[1] / sum];
    if n[2] < 0.0 {
        let ox = oct[0];
        let oy = oct[1];
        oct[0] = (1.0 - oy.abs()) * if ox >= 0.0 { 1.0 } else { -1.0 };
        oct[1] = (1.0 - ox.abs()) * if oy >= 0.0 { 1.0 } else { -1.0 };
    }
    oct
}

/// Decode octahedral back to normal vec3.
pub fn decode_octahedral(oct: [f32; 2]) -> [f32; 3] {
    let mut n = [oct[0], oct[1], 1.0 - oct[0].abs() - oct[1].abs()];
    if n[2] < 0.0 {
        let ox = n[0];
        let oy = n[1];
        n[0] = (1.0 - oy.abs()) * if ox >= 0.0 { 1.0 } else { -1.0 };
        n[1] = (1.0 - ox.abs()) * if oy >= 0.0 { 1.0 } else { -1.0 };
    }
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    [n[0] / len, n[1] / len, n[2] / len]
}

/// Reconstruct world position from depth + UV + inverse view-projection matrix.
pub fn reconstruct_position(uv: [f32; 2], depth: f32, inv_view_proj: &[[f32; 4]; 4]) -> [f32; 3] {
    let ndc_x = uv[0] * 2.0 - 1.0;
    let ndc_y = 1.0 - uv[1] * 2.0;
    let clip = [ndc_x, ndc_y, depth, 1.0];

    // Matrix multiply: inv_view_proj * clip (column-major)
    let mut world = [0.0f32; 4];
    for i in 0..4 {
        for j in 0..4 {
            world[i] += inv_view_proj[j][i] * clip[j];
        }
    }

    if world[3].abs() > 1e-8 {
        [
            world[0] / world[3],
            world[1] / world[3],
            world[2] / world[3],
        ]
    } else {
        [0.0; 3]
    }
}

/// Compressed G-Buffer format recommendations.
pub const COMPRESSED_NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;
pub const COMPRESSED_ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
pub const COMPRESSED_MATERIAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
// Position: reconstructed from depth, no texture needed

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_octahedral_roundtrip_positive_z() {
        let n = [0.0, 0.0, 1.0];
        let enc = encode_octahedral(n);
        let dec = decode_octahedral(enc);
        for i in 0..3 {
            assert!(
                (n[i] - dec[i]).abs() < 0.01,
                "axis {}: {} vs {}",
                i,
                n[i],
                dec[i]
            );
        }
    }

    #[test]
    fn test_octahedral_roundtrip_negative_z() {
        let n = [0.0, 0.0, -1.0];
        let enc = encode_octahedral(n);
        let dec = decode_octahedral(enc);
        for i in 0..3 {
            assert!((n[i] - dec[i]).abs() < 0.01);
        }
    }

    #[test]
    fn test_octahedral_roundtrip_diagonal() {
        let s = 1.0 / 3.0_f32.sqrt();
        let n = [s, s, s];
        let enc = encode_octahedral(n);
        let dec = decode_octahedral(enc);
        for i in 0..3 {
            assert!((n[i] - dec[i]).abs() < 0.01);
        }
    }

    #[test]
    fn test_octahedral_roundtrip_axes() {
        for n in [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
        ] {
            let dec = decode_octahedral(encode_octahedral(n));
            for i in 0..3 {
                assert!((n[i] - dec[i]).abs() < 0.02, "{:?} → {:?}", n, dec);
            }
        }
    }

    #[test]
    fn test_reconstruct_position_identity() {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let pos = reconstruct_position([0.5, 0.5], 0.5, &identity);
        // UV(0.5,0.5) → NDC(0,0), depth=0.5 → (0, 0, 0.5) in clip space
        assert!((pos[0] - 0.0).abs() < 0.01);
        assert!((pos[1] - 0.0).abs() < 0.01);
        assert!((pos[2] - 0.5).abs() < 0.01);
    }
}
