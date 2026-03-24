use voltex_math::{Vec3, Mat4};

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Vec3, // euler angles (radians): pitch(x), yaw(y), roll(z)
    pub scale: Vec3,
}

impl Transform {
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
        }
    }

    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
        }
    }

    pub fn from_position_scale(position: Vec3, scale: Vec3) -> Self {
        Self {
            position,
            rotation: Vec3::ZERO,
            scale,
        }
    }

    /// Builds the model matrix: Translation * RotY * RotX * RotZ * Scale
    pub fn matrix(&self) -> Mat4 {
        let t = Mat4::translation(self.position.x, self.position.y, self.position.z);
        let ry = Mat4::rotation_y(self.rotation.y);
        let rx = Mat4::rotation_x(self.rotation.x);
        let rz = Mat4::rotation_z(self.rotation.z);
        let s = Mat4::scale(self.scale.x, self.scale.y, self.scale.z);
        t * ry * rx * rz * s
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
    use voltex_math::Vec4;
    use std::f32::consts::FRAC_PI_2;

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
        t.rotation.y = FRAC_PI_2;
        let m = t.matrix();
        let p = Vec4::new(1.0, 0.0, 0.0, 1.0);
        let result = m * p;
        assert!(approx_eq(result.x, 0.0), "x: {}", result.x);
        assert!(approx_eq(result.y, 0.0), "y: {}", result.y);
        assert!(approx_eq(result.z, -1.0), "z: {}", result.z);
    }
}
