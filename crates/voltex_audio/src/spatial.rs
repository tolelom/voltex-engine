//! 3D spatialisation primitives: [`Listener`] + [`SpatialParams`] +
//! distance attenuation + equal-power panning.

use voltex_math::Vec3;
use std::f32::consts::PI;

/// Represents the listener in 3D space.
#[derive(Debug, Clone)]
pub struct Listener {
    /// World position of the listener (typically the camera/avatar).
    pub position: Vec3,
    /// Forward facing direction (unit vector).
    pub forward: Vec3,
    /// Right direction (unit vector, perpendicular to `forward`).
    pub right: Vec3,
}

impl Default for Listener {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            forward: Vec3::new(0.0, 0.0, -1.0),
            right: Vec3::X,
        }
    }
}

/// Parameters for a 3D audio emitter.
#[derive(Debug, Clone)]
pub struct SpatialParams {
    /// World position of the emitter.
    pub position: Vec3,
    /// Distance below which attenuation = 1.0 (no falloff).
    pub min_distance: f32,
    /// Distance beyond which attenuation = 0.0 (silent).
    pub max_distance: f32,
}

impl SpatialParams {
    /// Create with explicit parameters.
    pub fn new(position: Vec3, min_distance: f32, max_distance: f32) -> Self {
        Self { position, min_distance, max_distance }
    }

    /// Create at a position with default distances (1.0 min, 100.0 max).
    pub fn at(position: Vec3) -> Self {
        Self { position, min_distance: 1.0, max_distance: 100.0 }
    }
}

/// Inverse-distance attenuation model.
/// Returns 1.0 when distance <= min_dist, 0.0 when distance >= max_dist,
/// otherwise min_dist / distance clamped to [0, 1].
pub fn distance_attenuation(distance: f32, min_dist: f32, max_dist: f32) -> f32 {
    if distance <= min_dist {
        1.0
    } else if distance >= max_dist {
        0.0
    } else {
        (min_dist / distance).clamp(0.0, 1.0)
    }
}

/// Equal-power stereo panning based on listener orientation and emitter position.
/// Returns (left_gain, right_gain).
/// If the emitter is at the same position as the listener, returns (1.0, 1.0).
pub fn stereo_pan(listener: &Listener, emitter_pos: Vec3) -> (f32, f32) {
    let diff = emitter_pos - listener.position;
    let distance = diff.length();
    const EPSILON: f32 = 1e-6;
    if distance < EPSILON {
        return (1.0, 1.0);
    }
    let direction = diff * (1.0 / distance);
    let pan = direction.dot(listener.right).clamp(-1.0, 1.0);
    // Map pan [-1, 1] to angle [0, PI/2] via angle = pan * PI/4 + PI/4
    let angle = pan * (PI / 4.0) + (PI / 4.0);
    let left = angle.cos();
    let right = angle.sin();
    (left, right)
}

/// Convenience function combining distance attenuation and stereo panning.
/// Returns (attenuation, left_gain, right_gain).
pub fn compute_spatial_gains(listener: &Listener, spatial: &SpatialParams) -> (f32, f32, f32) {
    let diff = spatial.position - listener.position;
    let distance = diff.length();
    let attenuation = distance_attenuation(distance, spatial.min_distance, spatial.max_distance);
    let (left, right) = stereo_pan(listener, spatial.position);
    (attenuation, left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_math::Vec3;

    #[test]
    fn attenuation_at_min() {
        // distance <= min_distance should return 1.0
        assert_eq!(distance_attenuation(0.5, 1.0, 10.0), 1.0);
        assert_eq!(distance_attenuation(1.0, 1.0, 10.0), 1.0);
    }

    #[test]
    fn attenuation_at_max() {
        // distance >= max_distance should return 0.0
        assert_eq!(distance_attenuation(10.0, 1.0, 10.0), 0.0);
        assert_eq!(distance_attenuation(50.0, 1.0, 10.0), 0.0);
    }

    #[test]
    fn attenuation_between() {
        // inverse: min_dist / distance
        let result = distance_attenuation(5.0, 1.0, 10.0);
        let expected = 1.0_f32 / 5.0_f32;
        assert!((result - expected).abs() < 1e-6, "expected {expected}, got {result}");
    }

    #[test]
    fn pan_right() {
        // Emitter to the right (+X) should give right > left
        let listener = Listener::default();
        let emitter = Vec3::new(10.0, 0.0, 0.0);
        let (left, right) = stereo_pan(&listener, emitter);
        assert!(right > left, "expected right > left for +X emitter, got left={left}, right={right}");
    }

    #[test]
    fn pan_left() {
        // Emitter to the left (-X) should give left > right
        let listener = Listener::default();
        let emitter = Vec3::new(-10.0, 0.0, 0.0);
        let (left, right) = stereo_pan(&listener, emitter);
        assert!(left > right, "expected left > right for -X emitter, got left={left}, right={right}");
    }

    #[test]
    fn pan_front() {
        // Emitter directly in front (-Z) should give roughly equal gains
        let listener = Listener::default();
        let emitter = Vec3::new(0.0, 0.0, -10.0);
        let (left, right) = stereo_pan(&listener, emitter);
        assert!((left - right).abs() < 0.01, "expected roughly equal for front emitter, got left={left}, right={right}");
    }

    #[test]
    fn pan_same_position() {
        // Same position as listener should return (1.0, 1.0)
        let listener = Listener::default();
        let emitter = Vec3::ZERO;
        let (left, right) = stereo_pan(&listener, emitter);
        assert_eq!((left, right), (1.0, 1.0));
    }

    #[test]
    fn compute_spatial_gains_combines_both() {
        let listener = Listener::default();
        // Emitter at (5, 0, 0) with min=1, max=10
        let spatial = SpatialParams::new(Vec3::new(5.0, 0.0, 0.0), 1.0, 10.0);
        let (attenuation, left, right) = compute_spatial_gains(&listener, &spatial);

        // Check attenuation: distance=5, min=1, max=10 → 1/5 = 0.2
        let expected_atten = 1.0_f32 / 5.0_f32;
        assert!((attenuation - expected_atten).abs() < 1e-6,
            "attenuation mismatch: expected {expected_atten}, got {attenuation}");

        // Emitter is to the right, so right > left
        assert!(right > left, "expected right > left, got left={left}, right={right}");
    }
}
