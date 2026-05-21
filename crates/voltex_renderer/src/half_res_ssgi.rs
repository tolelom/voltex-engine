//! Half-resolution SSGI — downsample, GI pass, bilateral upsample.
#![allow(missing_docs)]

/// Calculate half-resolution dimensions (rounded up).
pub fn half_resolution(width: u32, height: u32) -> (u32, u32) {
    (width.div_ceil(2), height.div_ceil(2))
}

/// Bilinear upscale weight calculation for a 2x2 tap pattern.
pub fn bilinear_weights(frac_x: f32, frac_y: f32) -> [f32; 4] {
    let w00 = (1.0 - frac_x) * (1.0 - frac_y);
    let w10 = frac_x * (1.0 - frac_y);
    let w01 = (1.0 - frac_x) * frac_y;
    let w11 = frac_x * frac_y;
    [w00, w10, w01, w11]
}

/// Depth-aware upscale: reject samples with large depth discontinuity.
pub fn depth_aware_weight(center_depth: f32, sample_depth: f32, threshold: f32) -> f32 {
    if (center_depth - sample_depth).abs() > threshold { 0.0 } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_half_resolution() {
        assert_eq!(half_resolution(1920, 1080), (960, 540));
        assert_eq!(half_resolution(1921, 1081), (961, 541));
        assert_eq!(half_resolution(1, 1), (1, 1));
    }

    #[test]
    fn test_bilinear_weights_center() {
        let w = bilinear_weights(0.5, 0.5);
        for &wi in &w { assert!((wi - 0.25).abs() < 1e-6); }
    }

    #[test]
    fn test_bilinear_weights_corner() {
        let w = bilinear_weights(0.0, 0.0);
        assert!((w[0] - 1.0).abs() < 1e-6); // top-left = full weight
        assert!((w[1] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_bilinear_weights_sum_to_one() {
        let w = bilinear_weights(0.3, 0.7);
        let sum: f32 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_depth_aware_accept() {
        assert!((depth_aware_weight(0.5, 0.51, 0.1) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_depth_aware_reject() {
        assert!((depth_aware_weight(0.5, 0.9, 0.1) - 0.0).abs() < 1e-6);
    }
}
