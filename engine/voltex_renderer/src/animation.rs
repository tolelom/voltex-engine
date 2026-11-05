//! CPU keyframe animation: `sample_channel` interpolates a glTF animation channel at a
//! time, `AnimationPlayer` tracks playback time, and `build_trs` / `mat4_mul` build joint matrices.
#![allow(missing_docs)]

use crate::gltf::{AnimationPath, GltfChannel, Interpolation};
use voltex_math::Quat;

/// Sample a keyframe channel at time t.
pub fn sample_channel(channel: &GltfChannel, time: f32) -> Vec<f32> {
    if channel.times.is_empty() {
        return vec![];
    }
    if channel.times.len() == 1 {
        return channel.values.clone();
    }

    // Clamp time
    let t = time.clamp(channel.times[0], *channel.times.last().unwrap());

    // Find bracketing keyframes
    let mut i = 0;
    while i < channel.times.len() - 1 && channel.times[i + 1] < t {
        i += 1;
    }
    let j = (i + 1).min(channel.times.len() - 1);

    if i == j {
        return extract_keyframe(&channel.values, i, &channel.target_path);
    }

    let dt = channel.times[j] - channel.times[i];
    let factor = if dt > 1e-8 {
        (t - channel.times[i]) / dt
    } else {
        0.0
    };

    let a = extract_keyframe(&channel.values, i, &channel.target_path);
    let b = extract_keyframe(&channel.values, j, &channel.target_path);

    match channel.interpolation {
        Interpolation::Step => a,
        Interpolation::Linear | Interpolation::CubicSpline => {
            match channel.target_path {
                AnimationPath::Rotation => {
                    // SLERP for quaternions
                    let qa = Quat {
                        x: a[0],
                        y: a[1],
                        z: a[2],
                        w: a[3],
                    };
                    let qb = Quat {
                        x: b[0],
                        y: b[1],
                        z: b[2],
                        w: b[3],
                    };
                    let qr = Quat::slerp(qa, qb, factor);
                    vec![qr.x, qr.y, qr.z, qr.w]
                }
                _ => {
                    // Linear interpolation for translation/scale
                    a.iter()
                        .zip(b.iter())
                        .map(|(&va, &vb)| va + (vb - va) * factor)
                        .collect()
                }
            }
        }
    }
}

fn extract_keyframe(values: &[f32], index: usize, path: &AnimationPath) -> Vec<f32> {
    let stride = match path {
        AnimationPath::Translation | AnimationPath::Scale => 3,
        AnimationPath::Rotation => 4,
    };
    let start = index * stride;
    // Clamp both ends — a malformed glTF where values.len() < times.len() *
    // stride used to panic on `values[start..end]` when `start > values.len()`.
    if start >= values.len() {
        // Return a sensible default per path so the caller can still
        // interpolate (translation/scale = 0, rotation = identity).
        return match path {
            AnimationPath::Translation => vec![0.0, 0.0, 0.0],
            AnimationPath::Scale => vec![1.0, 1.0, 1.0],
            AnimationPath::Rotation => vec![0.0, 0.0, 0.0, 1.0],
        };
    }
    let end = (start + stride).min(values.len());
    values[start..end].to_vec()
}

/// Animation playback state.
pub struct AnimationPlayer {
    pub current_time: f32,
    pub duration: f32,
    pub playing: bool,
    pub looping: bool,
    pub speed: f32,
}

impl AnimationPlayer {
    pub fn new(duration: f32) -> Self {
        AnimationPlayer {
            current_time: 0.0,
            duration,
            playing: true,
            looping: true,
            speed: 1.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if !self.playing {
            return;
        }
        self.current_time += dt * self.speed;
        if self.current_time > self.duration {
            // A zero-duration animation has no meaningful state — clamp to
            // 0 and stop. Pre-fix, `current_time %= 0.0` was integer-modulo's
            // f32 cousin and produced NaN (not a panic on f32, but every
            // downstream `sample_channel` call then took the
            // `times.is_empty()` shortcut on NaN-compared times — silent
            // bad).
            if self.duration <= 0.0 {
                self.current_time = 0.0;
                self.playing = false;
            } else if self.looping {
                self.current_time %= self.duration;
            } else {
                self.current_time = self.duration;
                self.playing = false;
            }
        }
    }

    pub fn reset(&mut self) {
        self.current_time = 0.0;
        self.playing = true;
    }
}

/// Build a 4x4 TRS matrix from translation, rotation (quat), scale.
pub fn build_trs(translation: &[f32], rotation: &[f32], scale: &[f32]) -> [[f32; 4]; 4] {
    let q = if rotation.len() >= 4 {
        Quat {
            x: rotation[0],
            y: rotation[1],
            z: rotation[2],
            w: rotation[3],
        }
    } else {
        Quat::identity()
    };

    let rot = q.to_mat4().cols;
    let (tx, ty, tz) = if translation.len() >= 3 {
        (translation[0], translation[1], translation[2])
    } else {
        (0.0, 0.0, 0.0)
    };
    let (sx, sy, sz) = if scale.len() >= 3 {
        (scale[0], scale[1], scale[2])
    } else {
        (1.0, 1.0, 1.0)
    };

    // TRS = T * R * S. Column-major (`m[column][row]`): R·S scales each
    // column of R by that axis' scale factor.
    [
        [rot[0][0] * sx, rot[0][1] * sx, rot[0][2] * sx, 0.0],
        [rot[1][0] * sy, rot[1][1] * sy, rot[1][2] * sy, 0.0],
        [rot[2][0] * sz, rot[2][1] * sz, rot[2][2] * sz, 0.0],
        [tx, ty, tz, 1.0],
    ]
}

/// Multiply two 4x4 matrices (column-major).
pub fn mat4_mul(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut r = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                r[i][j] += a[k][j] * b[i][k];
            }
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_1_SQRT_2;

    #[test]
    fn test_quat_identity_to_mat4() {
        let q = Quat::identity();
        let m = q.to_mat4().cols;
        assert!((m[0][0] - 1.0).abs() < 1e-6);
        assert!((m[1][1] - 1.0).abs() < 1e-6);
        assert!((m[2][2] - 1.0).abs() < 1e-6);
        assert!((m[3][3] - 1.0).abs() < 1e-6);
    }

    /// Transform a vec3 (w=1 point) by a column-major `[[f32;4];4]` matrix.
    /// Outer index = column (matches build_trs putting translation in m[3]).
    fn transform_point(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
        let mut out = [0.0f32; 3];
        for (row, slot) in out.iter_mut().enumerate() {
            // M·v = v.x*col0 + v.y*col1 + v.z*col2 + 1*col3, per-row component.
            *slot = p[0] * m[0][row] + p[1] * m[1][row] + p[2] * m[2][row] + m[3][row];
        }
        out
    }

    #[test]
    fn test_quat_to_mat4_90deg_y_maps_pos_z_to_pos_x() {
        // 90° rotation about +Y: q = (0, sin45, 0, cos45).
        let s = FRAC_1_SQRT_2;
        let q = Quat {
            x: 0.0,
            y: s,
            z: 0.0,
            w: s,
        };
        let m = q.to_mat4().cols;
        // Column-major (outer = column): rotating +Z must yield +X.
        // Derived by hand from the matrix entries: column2 = m[2] = [1,0,0].
        let rotated_z = transform_point(&m, [0.0, 0.0, 1.0]);
        assert!(
            (rotated_z[0] - 1.0).abs() < 1e-5,
            "+Z.x -> {}",
            rotated_z[0]
        );
        assert!(rotated_z[1].abs() < 1e-5, "+Z.y -> {}", rotated_z[1]);
        assert!(rotated_z[2].abs() < 1e-5, "+Z.z -> {}", rotated_z[2]);
        // And +X must map to -Z (so the rotation is a proper, non-transposed one).
        let rotated_x = transform_point(&m, [1.0, 0.0, 0.0]);
        assert!(rotated_x[0].abs() < 1e-5, "+X.x -> {}", rotated_x[0]);
        assert!(
            (rotated_x[2] + 1.0).abs() < 1e-5,
            "+X.z -> {}",
            rotated_x[2]
        );
        // +Y is the rotation axis: unchanged.
        let rotated_y = transform_point(&m, [0.0, 1.0, 0.0]);
        assert!(
            (rotated_y[1] - 1.0).abs() < 1e-5,
            "+Y.y -> {}",
            rotated_y[1]
        );
    }

    #[test]
    fn test_build_trs_translation_and_scale_on_point() {
        // translation (1,2,3), identity rotation, scale (2,2,2).
        let m = build_trs(&[1.0, 2.0, 3.0], &[0.0, 0.0, 0.0, 1.0], &[2.0, 2.0, 2.0]);
        // Point (5,5,5): scaled to (10,10,10) then translated -> (11,12,13).
        let p = transform_point(&m, [5.0, 5.0, 5.0]);
        assert!((p[0] - 11.0).abs() < 1e-5, "x={}", p[0]);
        assert!((p[1] - 12.0).abs() < 1e-5, "y={}", p[1]);
        assert!((p[2] - 13.0).abs() < 1e-5, "z={}", p[2]);
        // Origin maps to the pure translation.
        let o = transform_point(&m, [0.0, 0.0, 0.0]);
        assert!(
            (o[0] - 1.0).abs() < 1e-5 && (o[1] - 2.0).abs() < 1e-5 && (o[2] - 3.0).abs() < 1e-5
        );
    }

    #[test]
    fn test_build_trs_scales_before_rotating() {
        // 90° about +Z (x→y), non-uniform scale (2, 3, 1).
        let half = std::f32::consts::FRAC_PI_4;
        let m = build_trs(&[0.0, 0.0, 0.0], &[0.0, 0.0, half.sin(), half.cos()], &[2.0, 3.0, 1.0]);
        // T·R·S: (1,0,0) is scaled to (2,0,0) first, then rotated to (0,2,0).
        let p = transform_point(&m, [1.0, 0.0, 0.0]);
        assert!(p[0].abs() < 1e-5 && (p[1] - 2.0).abs() < 1e-5, "got {p:?}");
        // (0,1,0) → (0,3,0) → (-3,0,0).
        let q = transform_point(&m, [0.0, 1.0, 0.0]);
        assert!((q[0] + 3.0).abs() < 1e-5 && q[1].abs() < 1e-5, "got {q:?}");
    }

    #[test]
    fn test_mat4_mul_translate_times_scale() {
        // A = translate(1,2,3), B = scale(2). mat4_mul(&A,&B) = A·B = scale-then-translate.
        let a = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [1.0, 2.0, 3.0, 1.0],
        ];
        let b = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 2.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let r = mat4_mul(&a, &b);
        // Hand-computed expected (column-major): diag 2, translation column (1,2,3).
        let expected = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 2.0, 0.0],
            [1.0, 2.0, 3.0, 1.0],
        ];
        for c in 0..4 {
            for row in 0..4 {
                assert!(
                    (r[c][row] - expected[c][row]).abs() < 1e-6,
                    "r[{}][{}] = {} expected {}",
                    c,
                    row,
                    r[c][row],
                    expected[c][row]
                );
            }
        }
        // Sanity: transforming (5,5,5) gives scale*5 + translate = (11,12,13).
        let p = transform_point(&r, [5.0, 5.0, 5.0]);
        assert!(
            (p[0] - 11.0).abs() < 1e-5 && (p[1] - 12.0).abs() < 1e-5 && (p[2] - 13.0).abs() < 1e-5
        );
    }

    #[test]
    fn test_quat_slerp_endpoints() {
        let a = Quat::identity();
        let b = Quat {
            x: 0.0,
            y: FRAC_1_SQRT_2,
            z: 0.0,
            w: FRAC_1_SQRT_2,
        }; // 90° around Y
        let r0 = Quat::slerp(a, b, 0.0);
        assert!((r0.w - 1.0).abs() < 0.01);
        let r1 = Quat::slerp(a, b, 1.0);
        assert!((r1.y - FRAC_1_SQRT_2).abs() < 0.01);
    }

    #[test]
    fn test_sample_channel_linear() {
        let channel = GltfChannel {
            target_node: 0,
            target_path: AnimationPath::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: vec![0.0, 0.0, 0.0, 10.0, 0.0, 0.0],
        };
        let val = sample_channel(&channel, 0.5);
        assert!((val[0] - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_sample_channel_step() {
        let channel = GltfChannel {
            target_node: 0,
            target_path: AnimationPath::Translation,
            interpolation: Interpolation::Step,
            times: vec![0.0, 1.0],
            values: vec![0.0, 0.0, 0.0, 10.0, 0.0, 0.0],
        };
        let val = sample_channel(&channel, 0.5);
        assert!((val[0] - 0.0).abs() < 0.01); // step: stays at first keyframe
    }

    #[test]
    fn test_animation_player_loop() {
        let mut player = AnimationPlayer::new(2.0);
        player.update(3.0);
        assert!(player.current_time < 2.0); // looped
        assert!(player.playing);
    }

    #[test]
    fn test_animation_player_no_loop() {
        let mut player = AnimationPlayer::new(1.0);
        player.looping = false;
        player.update(2.0);
        assert!((player.current_time - 1.0).abs() < 1e-6);
        assert!(!player.playing);
    }

    #[test]
    fn test_build_trs_identity() {
        let m = build_trs(&[0.0, 0.0, 0.0], &[0.0, 0.0, 0.0, 1.0], &[1.0, 1.0, 1.0]);
        assert!((m[0][0] - 1.0).abs() < 1e-6);
        assert!((m[3][0] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_mat4_mul_identity() {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let r = mat4_mul(&id, &id);
        assert!((r[0][0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn animation_player_zero_duration_does_not_div_by_zero() {
        // Pre-fix `current_time %= 0.0` produced NaN for f32 (not a panic
        // but every downstream channel sample then misbehaved). The player
        // now clamps to 0 and stops.
        let mut player = AnimationPlayer::new(0.0);
        player.update(1.0);
        assert_eq!(player.current_time, 0.0);
        assert!(!player.playing);
        assert!(!player.current_time.is_nan());
    }

    #[test]
    fn extract_keyframe_with_truncated_values_returns_default() {
        // Malformed glTF: times has 5 entries but values has only 6 floats
        // (room for 2 translations of stride=3). Pre-fix, indexing
        // values[12..15] for keyframe 4 panicked with slice OOB.
        let truncated_values: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        // Index 0 and 1 exist.
        assert_eq!(
            extract_keyframe(&truncated_values, 0, &AnimationPath::Translation),
            vec![1.0, 2.0, 3.0]
        );
        assert_eq!(
            extract_keyframe(&truncated_values, 1, &AnimationPath::Translation),
            vec![4.0, 5.0, 6.0]
        );
        // Index 4 is out of range — pre-fix this panicked, now returns default.
        assert_eq!(
            extract_keyframe(&truncated_values, 4, &AnimationPath::Translation),
            vec![0.0, 0.0, 0.0]
        );
        // Same for scale (default 1) and rotation (identity quat).
        assert_eq!(
            extract_keyframe(&[], 0, &AnimationPath::Scale),
            vec![1.0, 1.0, 1.0]
        );
        assert_eq!(
            extract_keyframe(&[], 0, &AnimationPath::Rotation),
            vec![0.0, 0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn quat_slerp_with_floating_point_dot_overshoot_does_not_nan() {
        // Two near-identical unit quaternions whose dot accumulates just past
        // 1.0 in f32 used to feed acos a > 1.0 input → NaN result. The clamp
        // added in this fix keeps the result finite.
        let a = Quat {
            x: 0.3,
            y: 0.4,
            z: 0.5,
            w: std::f32::consts::FRAC_1_SQRT_2,
        }
        .normalize();
        // Construct b as exactly a + tiny noise.
        let b = Quat {
            x: a.x + 1e-9,
            y: a.y,
            z: a.z,
            w: a.w,
        }
        .normalize();
        let r = Quat::slerp(a, b, 0.5);
        assert!(!r.x.is_nan() && !r.y.is_nan() && !r.z.is_nan() && !r.w.is_nan());
    }
}
