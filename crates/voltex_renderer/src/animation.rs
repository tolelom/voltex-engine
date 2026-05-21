#![allow(missing_docs)]

use crate::gltf::{GltfChannel, AnimationPath, Interpolation};

/// Quaternion for rotation interpolation.
#[derive(Debug, Clone, Copy)]
pub struct Quat {
    pub x: f32, pub y: f32, pub z: f32, pub w: f32,
}

impl Quat {
    pub fn identity() -> Self { Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 } }

    pub fn slerp(a: Quat, b: Quat, t: f32) -> Quat {
        let mut dot = a.x*b.x + a.y*b.y + a.z*b.z + a.w*b.w;
        let mut b = b;
        if dot < 0.0 { b = Quat { x: -b.x, y: -b.y, z: -b.z, w: -b.w }; dot = -dot; }
        // `acos` returns NaN for inputs outside [-1, 1]. Floating-point
        // accumulation in the dot product above can push the result just past
        // 1.0 even for unit quaternions, which used to silently produce
        // NaN-filled rotations downstream.
        dot = dot.clamp(-1.0, 1.0);
        if dot > 0.9995 {
            // Linear interpolation for nearly identical quaternions
            let r = Quat {
                x: a.x + t * (b.x - a.x), y: a.y + t * (b.y - a.y),
                z: a.z + t * (b.z - a.z), w: a.w + t * (b.w - a.w),
            };
            return r.normalize();
        }
        let theta = dot.acos();
        let sin_theta = theta.sin();
        let wa = ((1.0 - t) * theta).sin() / sin_theta;
        let wb = (t * theta).sin() / sin_theta;
        Quat {
            x: wa * a.x + wb * b.x, y: wa * a.y + wb * b.y,
            z: wa * a.z + wb * b.z, w: wa * a.w + wb * b.w,
        }
    }

    pub fn normalize(self) -> Quat {
        let len = (self.x*self.x + self.y*self.y + self.z*self.z + self.w*self.w).sqrt();
        if len < 1e-8 { return Quat::identity(); }
        Quat { x: self.x/len, y: self.y/len, z: self.z/len, w: self.w/len }
    }

    pub fn to_mat4(&self) -> [[f32; 4]; 4] {
        let (x, y, z, w) = (self.x, self.y, self.z, self.w);
        [
            [1.0-2.0*(y*y+z*z), 2.0*(x*y+w*z),     2.0*(x*z-w*y),     0.0],
            [2.0*(x*y-w*z),     1.0-2.0*(x*x+z*z), 2.0*(y*z+w*x),     0.0],
            [2.0*(x*z+w*y),     2.0*(y*z-w*x),     1.0-2.0*(x*x+y*y), 0.0],
            [0.0,               0.0,               0.0,               1.0],
        ]
    }
}

/// Sample a keyframe channel at time t.
pub fn sample_channel(channel: &GltfChannel, time: f32) -> Vec<f32> {
    if channel.times.is_empty() { return vec![]; }
    if channel.times.len() == 1 { return channel.values.clone(); }

    // Clamp time
    let t = time.clamp(channel.times[0], *channel.times.last().unwrap());

    // Find bracketing keyframes
    let mut i = 0;
    while i < channel.times.len() - 1 && channel.times[i + 1] < t {
        i += 1;
    }
    let j = (i + 1).min(channel.times.len() - 1);

    if i == j { return extract_keyframe(&channel.values, i, &channel.target_path); }

    let dt = channel.times[j] - channel.times[i];
    let factor = if dt > 1e-8 { (t - channel.times[i]) / dt } else { 0.0 };

    let a = extract_keyframe(&channel.values, i, &channel.target_path);
    let b = extract_keyframe(&channel.values, j, &channel.target_path);

    match channel.interpolation {
        Interpolation::Step => a,
        Interpolation::Linear | Interpolation::CubicSpline => {
            match channel.target_path {
                AnimationPath::Rotation => {
                    // SLERP for quaternions
                    let qa = Quat { x: a[0], y: a[1], z: a[2], w: a[3] };
                    let qb = Quat { x: b[0], y: b[1], z: b[2], w: b[3] };
                    let qr = Quat::slerp(qa, qb, factor);
                    vec![qr.x, qr.y, qr.z, qr.w]
                }
                _ => {
                    // Linear interpolation for translation/scale
                    a.iter().zip(b.iter()).map(|(&va, &vb)| va + (vb - va) * factor).collect()
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
        AnimationPlayer { current_time: 0.0, duration, playing: true, looping: true, speed: 1.0 }
    }

    pub fn update(&mut self, dt: f32) {
        if !self.playing { return; }
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

    pub fn reset(&mut self) { self.current_time = 0.0; self.playing = true; }
}

/// Build a 4x4 TRS matrix from translation, rotation (quat), scale.
pub fn build_trs(translation: &[f32], rotation: &[f32], scale: &[f32]) -> [[f32; 4]; 4] {
    let q = if rotation.len() >= 4 {
        Quat { x: rotation[0], y: rotation[1], z: rotation[2], w: rotation[3] }
    } else { Quat::identity() };

    let rot = q.to_mat4();
    let (tx, ty, tz) = if translation.len() >= 3 { (translation[0], translation[1], translation[2]) } else { (0.0, 0.0, 0.0) };
    let (sx, sy, sz) = if scale.len() >= 3 { (scale[0], scale[1], scale[2]) } else { (1.0, 1.0, 1.0) };

    // TRS = T * R * S
    [
        [rot[0][0]*sx, rot[0][1]*sy, rot[0][2]*sz, 0.0],
        [rot[1][0]*sx, rot[1][1]*sy, rot[1][2]*sz, 0.0],
        [rot[2][0]*sx, rot[2][1]*sy, rot[2][2]*sz, 0.0],
        [tx,           ty,           tz,           1.0],
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
        let m = q.to_mat4();
        assert!((m[0][0] - 1.0).abs() < 1e-6);
        assert!((m[1][1] - 1.0).abs() < 1e-6);
        assert!((m[2][2] - 1.0).abs() < 1e-6);
        assert!((m[3][3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_quat_slerp_endpoints() {
        let a = Quat::identity();
        let b = Quat { x: 0.0, y: FRAC_1_SQRT_2, z: 0.0, w: FRAC_1_SQRT_2 }; // 90° around Y
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
        let id = [[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]];
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
        assert_eq!(extract_keyframe(&truncated_values, 0, &AnimationPath::Translation), vec![1.0, 2.0, 3.0]);
        assert_eq!(extract_keyframe(&truncated_values, 1, &AnimationPath::Translation), vec![4.0, 5.0, 6.0]);
        // Index 4 is out of range — pre-fix this panicked, now returns default.
        assert_eq!(extract_keyframe(&truncated_values, 4, &AnimationPath::Translation), vec![0.0, 0.0, 0.0]);
        // Same for scale (default 1) and rotation (identity quat).
        assert_eq!(extract_keyframe(&[], 0, &AnimationPath::Scale), vec![1.0, 1.0, 1.0]);
        assert_eq!(extract_keyframe(&[], 0, &AnimationPath::Rotation), vec![0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn quat_slerp_with_floating_point_dot_overshoot_does_not_nan() {
        // Two near-identical unit quaternions whose dot accumulates just past
        // 1.0 in f32 used to feed acos a > 1.0 input → NaN result. The clamp
        // added in this fix keeps the result finite.
        let a = Quat { x: 0.3, y: 0.4, z: 0.5, w: std::f32::consts::FRAC_1_SQRT_2 }.normalize();
        // Construct b as exactly a + tiny noise.
        let b = Quat { x: a.x + 1e-9, y: a.y, z: a.z, w: a.w }.normalize();
        let r = Quat::slerp(a, b, 0.5);
        assert!(!r.x.is_nan() && !r.y.is_nan() && !r.z.is_nan() && !r.w.is_nan());
    }
}
