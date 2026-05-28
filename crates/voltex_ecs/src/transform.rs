//! Engine built-in `Transform` component — position + quaternion rotation + scale.
//!
//! The rotation is stored as a unit [`Quat`] (4 floats, YXZ Tait-Bryan
//! convention when converted to/from Euler). This removes Euler gimbal lock
//! from accumulated rotations and lets the physics integrator use the
//! quaternion-correct `dq/dt = ½ ω q` step instead of linear Vec3 add.
//!
//! **Migration helpers**: `Transform::from_euler_yxz` constructs from the
//! historical Euler triple, and the serializers in `component_registry` /
//! `scene` / `binary_scene` accept both the legacy 3-float Euler payload
//! and the current 4-float quaternion payload so old `.vscn` files keep
//! loading.

use voltex_math::{Mat4, Quat, Vec3};

/// 3D affine transform stored as position + quaternion rotation + scale.
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    /// World position.
    pub position: Vec3,
    /// Orientation as a unit quaternion (Hamilton, right-handed).
    pub rotation: Quat,
    /// Non-uniform scale.
    pub scale: Vec3,
}

impl Transform {
    /// Identity transform (zero position, identity rotation, unit scale).
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    /// Convenience: transform at `position` with identity rotation, unit scale.
    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    /// Convenience: transform at `position` with identity rotation and given scale.
    pub fn from_position_scale(position: Vec3, scale: Vec3) -> Self {
        Self {
            position,
            rotation: Quat::IDENTITY,
            scale,
        }
    }

    /// Construct from position + YXZ Euler rotation (pitch x, yaw y, roll z radians) + scale.
    /// Convenience for call sites still authoring orientations as Euler triples.
    pub fn from_euler_yxz(position: Vec3, euler: Vec3, scale: Vec3) -> Self {
        Self {
            position,
            rotation: Quat::from_euler_yxz(euler),
            scale,
        }
    }

    /// Builds the model matrix: Translation * Rotation * Scale.
    pub fn matrix(&self) -> Mat4 {
        let t = Mat4::translation(self.position.x, self.position.y, self.position.z);
        let r = self.rotation.to_mat4();
        let s = Mat4::scale(self.scale.x, self.scale.y, self.scale.z);
        t * r * s
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;
    use voltex_math::Vec4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn test_identity_transform() {
        let t = Transform::new();
        let m = t.matrix();
        // Transform point (1, 2, 3) — should be unchanged
        let p = Vec4::new(1.0, 2.0, 3.0, 1.0);
        let result = m * p;
        assert!(approx_eq(result.x, 1.0), "x: {}", result.x);
        assert!(approx_eq(result.y, 2.0), "y: {}", result.y);
        assert!(approx_eq(result.z, 3.0), "z: {}", result.z);
        assert!(approx_eq(result.w, 1.0), "w: {}", result.w);
    }

    #[test]
    fn test_translation() {
        let t = Transform::from_position(Vec3::new(10.0, 20.0, 30.0));
        let m = t.matrix();
        // Transform origin — should move to (10,20,30)
        let p = Vec4::new(0.0, 0.0, 0.0, 1.0);
        let result = m * p;
        assert!(approx_eq(result.x, 10.0), "x: {}", result.x);
        assert!(approx_eq(result.y, 20.0), "y: {}", result.y);
        assert!(approx_eq(result.z, 30.0), "z: {}", result.z);
    }

    #[test]
    fn test_scale() {
        let t = Transform::from_position_scale(Vec3::ZERO, Vec3::new(2.0, 3.0, 4.0));
        let m = t.matrix();
        // Scale (1,1,1) to (2,3,4)
        let p = Vec4::new(1.0, 1.0, 1.0, 1.0);
        let result = m * p;
        assert!(approx_eq(result.x, 2.0), "x: {}", result.x);
        assert!(approx_eq(result.y, 3.0), "y: {}", result.y);
        assert!(approx_eq(result.z, 4.0), "z: {}", result.z);
    }

    #[test]
    fn test_rotation_y() {
        let mut t = Transform::new();
        // 90° Y rotation on (1,0,0) -> approx (0,0,-1)
        t.rotation = voltex_math::Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), FRAC_PI_2);
        let m = t.matrix();
        let p = Vec4::new(1.0, 0.0, 0.0, 1.0);
        let result = m * p;
        assert!(approx_eq(result.x, 0.0), "x: {}", result.x);
        assert!(approx_eq(result.y, 0.0), "y: {}", result.y);
        assert!(approx_eq(result.z, -1.0), "z: {}", result.z);
    }

    #[test]
    fn from_euler_yxz_matches_matrix_chain() {
        let euler = Vec3::new(0.3, 0.7, -0.4);
        let t = Transform::from_euler_yxz(Vec3::ZERO, euler, Vec3::ONE);
        let m = t.matrix();
        let m_chain = voltex_math::Mat4::rotation_y(euler.y)
            * voltex_math::Mat4::rotation_x(euler.x)
            * voltex_math::Mat4::rotation_z(euler.z);
        for c in 0..4 {
            for r in 0..4 {
                assert!(
                    (m.cols[c][r] - m_chain.cols[c][r]).abs() < 1e-5,
                    "col {} row {}: {} vs {}",
                    c,
                    r,
                    m.cols[c][r],
                    m_chain.cols[c][r]
                );
            }
        }
    }
}
