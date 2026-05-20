/// Van der Corput sequence via bit-reversal.
pub fn radical_inverse_vdc(mut bits: u32) -> f32 {
    bits = bits.rotate_right(16);
    bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1);
    bits = ((bits & 0x33333333) << 2) | ((bits & 0xCCCCCCCC) >> 2);
    bits = ((bits & 0x0F0F0F0F) << 4) | ((bits & 0xF0F0F0F0) >> 4);
    bits = ((bits & 0x00FF00FF) << 8) | ((bits & 0xFF00FF00) >> 8);
    bits as f32 * 2.328_306_4e-10 // / 0x100000000
}

/// Hammersley low-discrepancy 2D sample.
pub fn hammersley(i: u32, n: u32) -> [f32; 2] {
    [i as f32 / n as f32, radical_inverse_vdc(i)]
}

/// GGX importance-sampled half vector in tangent space (N = (0,0,1)).
pub fn importance_sample_ggx(xi: [f32; 2], roughness: f32) -> [f32; 3] {
    let a = roughness * roughness;
    let phi = 2.0 * std::f32::consts::PI * xi[0];
    let cos_theta = ((1.0 - xi[1]) / (1.0 + (a * a - 1.0) * xi[1])).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    [phi.cos() * sin_theta, phi.sin() * sin_theta, cos_theta]
}

/// Smith geometry function for IBL: k = a²/2.
pub fn geometry_smith_ibl(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let k = a / 2.0;
    let ggx_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let ggx_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
    ggx_v * ggx_l
}

/// Monte Carlo integration of the split-sum BRDF for a given NdotV and roughness.
/// Returns (scale, bias) such that F_env ≈ F0 * scale + bias.
pub fn integrate_brdf(n_dot_v: f32, roughness: f32) -> (f32, f32) {
    const NUM_SAMPLES: u32 = 1024;

    // View vector in tangent space where N = (0,0,1).
    let v = [
        (1.0 - n_dot_v * n_dot_v).max(0.0).sqrt(),
        0.0_f32,
        n_dot_v,
    ];

    let mut scale = 0.0_f32;
    let mut bias = 0.0_f32;

    for i in 0..NUM_SAMPLES {
        let xi = hammersley(i, NUM_SAMPLES);
        let h = importance_sample_ggx(xi, roughness);

        // dot(V, H)
        let v_dot_h = (v[0] * h[0] + v[1] * h[1] + v[2] * h[2]).max(0.0);

        // Reflect V around H to get L.
        let l = [
            2.0 * v_dot_h * h[0] - v[0],
            2.0 * v_dot_h * h[1] - v[1],
            2.0 * v_dot_h * h[2] - v[2],
        ];

        let n_dot_l = l[2].max(0.0); // L.z in tangent space
        let n_dot_h = h[2].max(0.0);

        if n_dot_l > 0.0 {
            let g = geometry_smith_ibl(n_dot_v, n_dot_l, roughness);
            let g_vis = g * v_dot_h / (n_dot_h * n_dot_v).max(0.001);
            let fc = (1.0 - v_dot_h).powi(5);
            scale += g_vis * (1.0 - fc);
            bias += g_vis * fc;
        }
    }

    (scale / NUM_SAMPLES as f32, bias / NUM_SAMPLES as f32)
}

/// Generate the BRDF LUT for the split-sum IBL approximation.
///
/// Returns `size * size` elements. Each element is `[scale, bias]` where
/// the x-axis (u) maps NdotV in [0, 1] and the y-axis (v) maps roughness in [0, 1].
pub fn generate_brdf_lut(size: u32) -> Vec<[f32; 2]> {
    let mut lut = Vec::with_capacity((size * size) as usize);
    for row in 0..size {
        // v maps to roughness (row 0 → roughness near 0, last row → 1).
        let roughness = ((row as f32 + 0.5) / size as f32).clamp(0.0, 1.0);
        for col in 0..size {
            // u maps to NdotV.
            let n_dot_v = ((col as f32 + 0.5) / size as f32).clamp(0.0, 1.0);
            let (scale, bias) = integrate_brdf(n_dot_v, roughness);
            lut.push([scale, bias]);
        }
    }
    lut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brdf_lut_dimensions() {
        let size = 64u32;
        let lut = generate_brdf_lut(size);
        assert_eq!(lut.len(), (size * size) as usize);
    }

    #[test]
    fn test_brdf_lut_values_in_range() {
        let lut = generate_brdf_lut(64);
        for pixel in &lut {
            assert!(
                pixel[0] >= 0.0 && pixel[0] <= 1.5,
                "scale {} out of range",
                pixel[0]
            );
            assert!(
                pixel[1] >= 0.0 && pixel[1] <= 1.5,
                "bias {} out of range",
                pixel[1]
            );
        }
    }

    #[test]
    fn test_hammersley() {
        let n = 1024u32;
        let sample = hammersley(0, n);
        assert_eq!(sample[0], 0.0, "hammersley(0, N).x should be 0");
    }
}
