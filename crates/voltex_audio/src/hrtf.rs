//! Approximate Head-Related Transfer Function (HRTF) — Woodworth ITD model
//! plus a cosine ILD shadow factor. Cheap stand-in for full HRIR convolution.

/// Head radius in meters (average human).
const HEAD_RADIUS: f32 = 0.0875;
/// Speed of sound in m/s.
const SPEED_OF_SOUND: f32 = 343.0;

/// HRTF filter result for a sound at a given azimuth angle.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HrtfResult {
    /// ITD: delay for the left ear in samples.
    pub left_delay_samples: f32,
    /// ITD: delay for the right ear in samples.
    pub right_delay_samples: f32,
    /// ILD: gain for the left ear in `[0.0, 1.0]`.
    pub left_gain: f32,
    /// ILD: gain for the right ear in `[0.0, 1.0]`.
    pub right_gain: f32,
}

/// Calculate HRTF parameters from azimuth angle.
/// azimuth: angle in radians, 0 = front, PI/2 = right, -PI/2 = left, PI = behind.
pub(crate) fn calculate_hrtf(azimuth: f32, sample_rate: u32) -> HrtfResult {
    // ITD: Woodworth formula
    // time_diff = (HEAD_RADIUS / SPEED_OF_SOUND) * (azimuth + sin(azimuth))
    let itd = (HEAD_RADIUS / SPEED_OF_SOUND) * (azimuth.abs() + azimuth.abs().sin());
    let delay_samples = itd * sample_rate as f32;

    // ILD: simplified frequency-independent model
    // Sound is louder on the side facing the source
    let shadow = 0.5 * (1.0 + azimuth.cos()); // 1.0 at front, 0.5 at side, 0.0 at back

    let (left_delay, right_delay, left_gain, right_gain);
    if azimuth >= 0.0 {
        // Sound from right side
        left_delay = delay_samples;
        right_delay = 0.0;
        left_gain = (0.3 + 0.7 * shadow).min(1.0); // shadowed side
        right_gain = 1.0;
    } else {
        // Sound from left side
        left_delay = 0.0;
        right_delay = delay_samples;
        left_gain = 1.0;
        right_gain = (0.3 + 0.7 * shadow).min(1.0);
    }

    HrtfResult {
        left_delay_samples: left_delay,
        right_delay_samples: right_delay,
        left_gain,
        right_gain,
    }
}

/// Calculate azimuth angle from listener position/forward to sound position.
pub(crate) fn azimuth_from_positions(
    listener_pos: [f32; 3],
    listener_forward: [f32; 3],
    listener_right: [f32; 3],
    sound_pos: [f32; 3],
) -> f32 {
    let dx = sound_pos[0] - listener_pos[0];
    let dy = sound_pos[1] - listener_pos[1];
    let dz = sound_pos[2] - listener_pos[2];
    let len = (dx * dx + dy * dy + dz * dz).sqrt();
    if len < 1e-6 {
        return 0.0;
    }
    let dir = [dx / len, dy / len, dz / len];

    // Dot with right vector gives sin(azimuth)
    let right_dot =
        dir[0] * listener_right[0] + dir[1] * listener_right[1] + dir[2] * listener_right[2];
    // Dot with forward vector gives cos(azimuth)
    let fwd_dot = dir[0] * listener_forward[0]
        + dir[1] * listener_forward[1]
        + dir[2] * listener_forward[2];

    right_dot.atan2(fwd_dot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_hrtf_front() {
        let r = calculate_hrtf(0.0, 44100);
        assert!((r.left_delay_samples - 0.0).abs() < 1.0);
        assert!((r.right_delay_samples - 0.0).abs() < 1.0);
        assert!((r.left_gain - r.right_gain).abs() < 0.01); // symmetric
    }

    #[test]
    fn test_hrtf_right() {
        let r = calculate_hrtf(PI / 2.0, 44100);
        assert!(r.left_delay_samples > r.right_delay_samples);
        assert!(r.left_gain < r.right_gain);
    }

    #[test]
    fn test_hrtf_left() {
        let r = calculate_hrtf(-PI / 2.0, 44100);
        assert!(r.right_delay_samples > r.left_delay_samples);
        assert!(r.right_gain < r.left_gain);
    }

    #[test]
    fn test_azimuth_front() {
        let az = azimuth_from_positions(
            [0.0; 3],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, -5.0],
        );
        assert!(az.abs() < 0.1); // roughly front
    }

    #[test]
    fn test_azimuth_right() {
        let az = azimuth_from_positions(
            [0.0; 3],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
        );
        assert!((az - PI / 2.0).abs() < 0.1);
    }
}
