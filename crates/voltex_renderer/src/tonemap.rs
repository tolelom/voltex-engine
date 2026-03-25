use bytemuck::{Pod, Zeroable};

/// Uniform buffer for the tonemap pass.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TonemapUniform {
    /// Bloom contribution weight.
    pub bloom_intensity: f32,
    /// Pre-tonemap exposure multiplier.
    pub exposure: f32,
    pub _padding: [f32; 2],
}

impl Default for TonemapUniform {
    fn default() -> Self {
        Self {
            bloom_intensity: 0.5,
            exposure: 1.0,
            _padding: [0.0; 2],
        }
    }
}

/// CPU implementation of the ACES filmic tonemap curve (for testing / CPU-side work).
///
/// Formula: clamp((x*(2.51*x+0.03))/(x*(2.43*x+0.59)+0.14), 0, 1)
pub fn aces_tonemap(x: f32) -> f32 {
    let num = x * (2.51 * x + 0.03);
    let den = x * (2.43 * x + 0.59) + 0.14;
    (num / den).clamp(0.0, 1.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aces_zero() {
        // aces(0) should be ≈ 0
        assert!(aces_tonemap(0.0).abs() < 1e-5, "aces(0) = {}", aces_tonemap(0.0));
    }

    #[test]
    fn aces_one() {
        // aces(1) ≈ 0.80 with the standard formula
        // clamp((1*(2.51+0.03))/(1*(2.43+0.59)+0.14), 0, 1) = 2.54/3.16 ≈ 0.8038
        let v = aces_tonemap(1.0);
        assert!(
            (v - 0.8038).abs() < 0.001,
            "aces(1) = {}, expected ≈ 0.8038",
            v
        );
    }

    #[test]
    fn aces_large() {
        // aces(10) should be very close to 1.0 (saturated)
        let v = aces_tonemap(10.0);
        assert!(v > 0.999, "aces(10) = {}, expected ≈ 1.0", v);
    }

    #[test]
    fn tonemap_uniform_default() {
        let u = TonemapUniform::default();
        assert!((u.bloom_intensity - 0.5).abs() < f32::EPSILON);
        assert!((u.exposure - 1.0).abs() < f32::EPSILON);
        assert_eq!(u._padding, [0.0f32; 2]);
    }
}
