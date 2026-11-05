//! Reusable fade curve shapes. Apply via [`apply_fade`].

/// Fade curve types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FadeCurve {
    /// `f(t) = t` — uniform rate.
    Linear,
    /// `f(t) = t²` — slow start, fast finish.
    Exponential,
    /// `f(t) = √t` — fast start, slow finish.
    Logarithmic,
    /// Smoothstep `3t² − 2t³` — smooth at both ends.
    SCurve,
}

/// Apply fade curve to a normalized parameter t (0.0 to 1.0).
pub fn apply_fade(t: f32, curve: FadeCurve) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match curve {
        FadeCurve::Linear => t,
        FadeCurve::Exponential => t * t,
        FadeCurve::Logarithmic => t.sqrt(),
        FadeCurve::SCurve => {
            // Smoothstep: 3t^2 - 2t^3
            t * t * (3.0 - 2.0 * t)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear() {
        assert!((apply_fade(0.5, FadeCurve::Linear) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_exponential() {
        assert!((apply_fade(0.5, FadeCurve::Exponential) - 0.25).abs() < 1e-6);
        assert!(apply_fade(0.5, FadeCurve::Exponential) < 0.5); // slower start
    }

    #[test]
    fn test_logarithmic() {
        assert!(apply_fade(0.5, FadeCurve::Logarithmic) > 0.5); // faster start
    }

    #[test]
    fn test_scurve() {
        assert!((apply_fade(0.0, FadeCurve::SCurve) - 0.0).abs() < 1e-6);
        assert!((apply_fade(1.0, FadeCurve::SCurve) - 1.0).abs() < 1e-6);
        assert!((apply_fade(0.5, FadeCurve::SCurve) - 0.5).abs() < 1e-6); // midpoint same
    }

    #[test]
    fn test_endpoints() {
        for curve in [FadeCurve::Linear, FadeCurve::Exponential, FadeCurve::Logarithmic, FadeCurve::SCurve] {
            assert!((apply_fade(0.0, curve) - 0.0).abs() < 1e-6);
            assert!((apply_fade(1.0, curve) - 1.0).abs() < 1e-6);
        }
    }
}
