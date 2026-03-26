/// Bilateral bloom weight: attenuates blur across brightness edges.
pub fn bilateral_bloom_weight(
    center_luminance: f32,
    sample_luminance: f32,
    spatial_weight: f32,
    sigma_luminance: f32,
) -> f32 {
    let lum_diff = (center_luminance - sample_luminance).abs();
    let lum_weight = (-lum_diff * lum_diff / (2.0 * sigma_luminance * sigma_luminance)).exp();
    spatial_weight * lum_weight
}

/// Calculate luminance from RGB.
pub fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// 5-tap Gaussian weights for 1D bloom blur.
pub fn gaussian_5tap() -> [f32; 5] {
    // Sigma ≈ 1.4
    [0.0625, 0.25, 0.375, 0.25, 0.0625]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bilateral_bloom_same_luminance() {
        let w = bilateral_bloom_weight(0.5, 0.5, 1.0, 0.1);
        assert!((w - 1.0).abs() < 0.01); // same lum → full weight
    }

    #[test]
    fn test_bilateral_bloom_edge() {
        let w = bilateral_bloom_weight(0.1, 0.9, 1.0, 0.1);
        assert!(w < 0.01); // large lum diff → near zero
    }

    #[test]
    fn test_luminance_white() {
        let l = luminance(1.0, 1.0, 1.0);
        assert!((l - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_gaussian_5tap_sum() {
        let g = gaussian_5tap();
        let sum: f32 = g.iter().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }
}
