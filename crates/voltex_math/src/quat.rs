//! Unit-quaternion rotation primitive.
//!
//! Quaternions are the engine's representation of orientation for
//! interpolation-friendly storage (no gimbal lock, well-defined slerp,
//! shorter compose paths than 3×3 matrices). [`Transform`](crate::Vec3)
//! historically stored Euler angles; the migration to `Quat` is staged in
//! the audit backlog as E3.
//!
//! **Convention**: Hamilton product, right-handed, `+Y` up. Euler
//! conversions use the **YXZ** Tait-Bryan order
//! (`R = RotY(y) * RotX(x) * RotZ(z)`) — chosen to match
//! `Transform::matrix()`'s pre-existing rotation order so a future
//! `rotation: Vec3` → `rotation: Quat` swap yields the same world matrix.

use crate::mat4::Mat4;
use crate::vec3::Vec3;

/// Unit quaternion `xi + yj + zk + w` representing a rotation.
///
/// All public methods assume — and most return — unit quaternions. Mixing
/// in non-unit values silently produces a non-rotation transform; call
/// [`Quat::normalize`] after any composition that may drift.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    /// Vector part `x`.
    pub x: f32,
    /// Vector part `y`.
    pub y: f32,
    /// Vector part `z`.
    pub z: f32,
    /// Scalar part `w`.
    pub w: f32,
}

impl Quat {
    /// Identity rotation `(0, 0, 0, 1)`.
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// Construct from components without normalizing — caller's
    /// responsibility to ensure unit length when treating as a rotation.
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Identity rotation. Alias for [`Quat::IDENTITY`].
    pub fn identity() -> Self {
        Self::IDENTITY
    }

    /// Rotation of `angle` radians around a unit `axis`.
    /// The axis is normalized internally; a zero-length axis returns identity.
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self {
        let len = (axis.x * axis.x + axis.y * axis.y + axis.z * axis.z).sqrt();
        if len < 1e-8 {
            return Self::IDENTITY;
        }
        let inv = 1.0 / len;
        let (s, c) = (angle * 0.5).sin_cos();
        Self {
            x: axis.x * inv * s,
            y: axis.y * inv * s,
            z: axis.z * inv * s,
            w: c,
        }
    }

    /// Convert YXZ Tait-Bryan Euler angles (radians) to a quaternion.
    ///
    /// Matches `Transform::matrix()`'s `RotY(e.y) * RotX(e.x) * RotZ(e.z)`
    /// order so `Quat::from_euler_yxz(t.rotation).to_mat4()` equals
    /// `Mat4::rotation_y(t.rotation.y) * Mat4::rotation_x(t.rotation.x) *
    /// Mat4::rotation_z(t.rotation.z)` modulo floating-point error.
    pub fn from_euler_yxz(euler: Vec3) -> Self {
        let qy = Self::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), euler.y);
        let qx = Self::from_axis_angle(Vec3::new(1.0, 0.0, 0.0), euler.x);
        let qz = Self::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), euler.z);
        // Compose Y * X * Z (read left-to-right: apply Z then X then Y).
        qy * qx * qz
    }

    /// Hamilton product `self * rhs` (apply `rhs` then `self` to a vector).
    pub fn mul_quat(self, rhs: Self) -> Self {
        let (ax, ay, az, aw) = (self.x, self.y, self.z, self.w);
        let (bx, by, bz, bw) = (rhs.x, rhs.y, rhs.z, rhs.w);
        Self {
            x: aw * bx + ax * bw + ay * bz - az * by,
            y: aw * by - ax * bz + ay * bw + az * bx,
            z: aw * bz + ax * by - ay * bx + az * bw,
            w: aw * bw - ax * bx - ay * by - az * bz,
        }
    }

    /// Squared length — useful when comparing magnitudes without `sqrt`.
    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    /// Euclidean length.
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Dot product of two quaternions (4-D dot).
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z + self.w * rhs.w
    }

    /// Conjugate: negate the vector part. Equals the inverse for unit
    /// quaternions and is cheaper.
    pub fn conjugate(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }

    /// Inverse `q⁻¹ = q* / |q|²`. Returns identity if length is too small.
    pub fn inverse(self) -> Self {
        let len2 = self.length_squared();
        if len2 < 1e-16 {
            return Self::IDENTITY;
        }
        let inv = 1.0 / len2;
        let c = self.conjugate();
        Self {
            x: c.x * inv,
            y: c.y * inv,
            z: c.z * inv,
            w: c.w * inv,
        }
    }

    /// Normalize to unit length. Returns identity for zero-length input.
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len < 1e-8 {
            return Self::IDENTITY;
        }
        let inv = 1.0 / len;
        Self {
            x: self.x * inv,
            y: self.y * inv,
            z: self.z * inv,
            w: self.w * inv,
        }
    }

    /// Convert to a 4×4 rotation matrix (last row/column = identity).
    /// The input is assumed unit-length; non-unit inputs scale the output.
    pub fn to_mat4(self) -> Mat4 {
        let (x, y, z, w) = (self.x, self.y, self.z, self.w);
        // Column-major: each [..] is a column. Matches Mat4::rotation_y etc.
        Mat4 {
            cols: [
                [
                    1.0 - 2.0 * (y * y + z * z),
                    2.0 * (x * y + w * z),
                    2.0 * (x * z - w * y),
                    0.0,
                ],
                [
                    2.0 * (x * y - w * z),
                    1.0 - 2.0 * (x * x + z * z),
                    2.0 * (y * z + w * x),
                    0.0,
                ],
                [
                    2.0 * (x * z + w * y),
                    2.0 * (y * z - w * x),
                    1.0 - 2.0 * (x * x + y * y),
                    0.0,
                ],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Spherical linear interpolation between two unit quaternions.
    /// `t` is clamped to `[0, 1]`. Picks the shorter arc by flipping `b`
    /// when `dot(a, b) < 0`. Falls back to normalized linear interpolation
    /// for nearly-parallel inputs where `acos` would lose precision.
    ///
    /// NaN-safe: clamps the dot product to `[-1, 1]` before `acos` since
    /// floating-point drift can push unit-quaternion dot products just
    /// past 1.0, which previously produced NaN-filled rotations.
    pub fn slerp(a: Self, b: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mut dot = a.dot(b);
        let mut b = b;
        if dot < 0.0 {
            b = Self {
                x: -b.x,
                y: -b.y,
                z: -b.z,
                w: -b.w,
            };
            dot = -dot;
        }
        dot = dot.clamp(-1.0, 1.0);

        if dot > 0.9995 {
            // Nearly identical — linear interpolation + renormalize.
            let r = Self {
                x: a.x + t * (b.x - a.x),
                y: a.y + t * (b.y - a.y),
                z: a.z + t * (b.z - a.z),
                w: a.w + t * (b.w - a.w),
            };
            return r.normalize();
        }
        let theta = dot.acos();
        let sin_theta = theta.sin();
        let wa = ((1.0 - t) * theta).sin() / sin_theta;
        let wb = (t * theta).sin() / sin_theta;
        Self {
            x: wa * a.x + wb * b.x,
            y: wa * a.y + wb * b.y,
            z: wa * a.z + wb * b.z,
            w: wa * a.w + wb * b.w,
        }
    }
}

impl Default for Quat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl std::ops::Mul for Quat {
    type Output = Quat;
    fn mul(self, rhs: Quat) -> Quat {
        self.mul_quat(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    fn assert_quat_eq(a: Quat, b: Quat, eps: f32, label: &str) {
        assert!(
            (a.x - b.x).abs() < eps
                && (a.y - b.y).abs() < eps
                && (a.z - b.z).abs() < eps
                && (a.w - b.w).abs() < eps,
            "{label}: {a:?} vs {b:?}"
        );
    }

    fn assert_mat4_close(a: &Mat4, b: &Mat4, eps: f32, label: &str) {
        for c in 0..4 {
            for r in 0..4 {
                assert!(
                    (a.cols[c][r] - b.cols[c][r]).abs() < eps,
                    "{label}: col {c} row {r}: {} vs {}",
                    a.cols[c][r],
                    b.cols[c][r]
                );
            }
        }
    }

    #[test]
    fn identity_is_zero_xyz_one_w() {
        let q = Quat::identity();
        assert_eq!(q, Quat::new(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn identity_to_mat4_is_identity() {
        let m = Quat::identity().to_mat4();
        assert_mat4_close(&m, &Mat4::IDENTITY, 1e-6, "identity quat → mat");
    }

    #[test]
    fn from_axis_angle_zero_axis_returns_identity() {
        let q = Quat::from_axis_angle(Vec3::ZERO, 1.5);
        assert_eq!(q, Quat::identity());
    }

    #[test]
    fn from_axis_angle_y_90_matches_mat4_rotation_y() {
        let q = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), FRAC_PI_2);
        assert_mat4_close(&q.to_mat4(), &Mat4::rotation_y(FRAC_PI_2), 1e-6, "rotY 90°");
    }

    #[test]
    fn from_axis_angle_x_90_matches_mat4_rotation_x() {
        let q = Quat::from_axis_angle(Vec3::new(1.0, 0.0, 0.0), FRAC_PI_2);
        assert_mat4_close(&q.to_mat4(), &Mat4::rotation_x(FRAC_PI_2), 1e-6, "rotX 90°");
    }

    #[test]
    fn from_axis_angle_z_90_matches_mat4_rotation_z() {
        let q = Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), FRAC_PI_2);
        assert_mat4_close(&q.to_mat4(), &Mat4::rotation_z(FRAC_PI_2), 1e-6, "rotZ 90°");
    }

    #[test]
    fn from_euler_yxz_matches_transform_rotation_order() {
        // Pick non-trivial angles that exercise all three components, then
        // verify Quat::from_euler_yxz(e).to_mat4() equals the matrix that
        // Transform::matrix() builds (RotY * RotX * RotZ).
        let e = Vec3::new(0.3, 0.7, -0.4);
        let q_mat = Quat::from_euler_yxz(e).to_mat4();
        let mat_chain = Mat4::rotation_y(e.y) * Mat4::rotation_x(e.x) * Mat4::rotation_z(e.z);
        assert_mat4_close(&q_mat, &mat_chain, 1e-5, "YXZ euler equivalence");
    }

    #[test]
    fn multiplication_with_identity_is_noop() {
        let q = Quat::from_axis_angle(Vec3::new(1.0, 2.0, 3.0).normalize(), 1.2);
        assert_quat_eq(q * Quat::identity(), q, 1e-6, "q * I");
        assert_quat_eq(Quat::identity() * q, q, 1e-6, "I * q");
    }

    #[test]
    fn conjugate_inverse_unit_quat_are_equal() {
        let q = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), 0.7);
        // Unit quaternion: conjugate equals inverse.
        assert_quat_eq(q.conjugate(), q.inverse(), 1e-6, "unit conj = inv");
        // q * q⁻¹ = identity.
        assert_quat_eq(q * q.inverse(), Quat::identity(), 1e-6, "q · q⁻¹");
    }

    #[test]
    fn inverse_handles_zero_quat() {
        let z = Quat::new(0.0, 0.0, 0.0, 0.0);
        assert_eq!(z.inverse(), Quat::identity());
    }

    #[test]
    fn normalize_handles_zero_quat() {
        let z = Quat::new(0.0, 0.0, 0.0, 0.0);
        assert_eq!(z.normalize(), Quat::identity());
    }

    #[test]
    fn slerp_at_zero_returns_a_and_at_one_returns_b() {
        let a = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), 0.0);
        let b = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), FRAC_PI_2);
        assert_quat_eq(Quat::slerp(a, b, 0.0), a, 1e-6, "slerp t=0");
        assert_quat_eq(Quat::slerp(a, b, 1.0), b, 1e-6, "slerp t=1");
    }

    #[test]
    fn slerp_midpoint_is_45_degree_rotation() {
        let a = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), 0.0);
        let b = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), FRAC_PI_2);
        let mid = Quat::slerp(a, b, 0.5);
        let expected = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), FRAC_PI_2 * 0.5);
        assert_quat_eq(mid, expected, 1e-6, "slerp midpoint");
    }

    #[test]
    fn slerp_takes_shorter_arc_when_dot_negative() {
        // A and -A are the same rotation. Slerping toward a "negated" b
        // (180° flipped sign) should still produce a 90° rotation when
        // b is the 90° quaternion negated.
        let a = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), 0.0);
        let b = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), FRAC_PI_2);
        let neg_b = Quat::new(-b.x, -b.y, -b.z, -b.w);
        let mid_normal = Quat::slerp(a, b, 0.5);
        let mid_negated = Quat::slerp(a, neg_b, 0.5);
        // Same shorter arc, so same rotation (possibly different sign).
        // Compare by the resulting matrices instead of raw components.
        assert_mat4_close(
            &mid_normal.to_mat4(),
            &mid_negated.to_mat4(),
            1e-5,
            "shorter-arc invariance",
        );
    }

    #[test]
    fn slerp_clamps_t_to_zero_one() {
        let a = Quat::identity();
        let b = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), FRAC_PI_2);
        assert_quat_eq(Quat::slerp(a, b, -1.0), a, 1e-6, "t<0 clamps to a");
        assert_quat_eq(Quat::slerp(a, b, 2.0), b, 1e-6, "t>1 clamps to b");
    }

    #[test]
    fn slerp_is_nan_safe_on_drifted_unit_quats() {
        // Float drift can push dot(a, b) to ~1.0000001 even when a == b is
        // unit. The clamp inside slerp keeps acos defined.
        let a = Quat::new(0.0, 0.0, 0.0, 1.0000001); // slightly past unit
        let b = Quat::new(0.0, 0.0, 0.0, 1.0);
        let r = Quat::slerp(a, b, 0.5);
        assert!(
            r.x.is_finite() && r.y.is_finite() && r.z.is_finite() && r.w.is_finite(),
            "slerp must not produce NaN on drifted unit inputs: got {r:?}"
        );
    }

    #[test]
    fn length_of_unit_quat_is_one() {
        let q = Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), 0.5);
        assert!((q.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn from_euler_yxz_zero_is_identity() {
        let q = Quat::from_euler_yxz(Vec3::ZERO);
        assert_quat_eq(q, Quat::identity(), 1e-6, "0 euler = identity");
    }

    #[test]
    fn from_euler_yxz_pi_y_matches_rotation_y_pi() {
        let q = Quat::from_euler_yxz(Vec3::new(0.0, PI, 0.0));
        assert_mat4_close(&q.to_mat4(), &Mat4::rotation_y(PI), 1e-5, "euler Y=π");
    }
}
