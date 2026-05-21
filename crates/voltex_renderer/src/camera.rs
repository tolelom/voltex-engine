//! Free-flying first-person camera + WASD/mouse controller.

use voltex_math::{Vec3, Mat4};

/// First-person camera — position + yaw + pitch + projection params.
pub struct Camera {
    /// World position of the eye.
    pub position: Vec3,
    /// Yaw in radians (rotation around the Y axis).
    pub yaw: f32,
    /// Pitch in radians (rotation around the X axis).
    pub pitch: f32,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Aspect ratio (width / height).
    pub aspect: f32,
    /// Near clipping plane distance.
    pub near: f32,
    /// Far clipping plane distance.
    pub far: f32,
}

impl Camera {
    /// New camera at `position` with sensible defaults (45° fov, near 0.1, far 100).
    pub fn new(position: Vec3, aspect: f32) -> Self {
        Self {
            position,
            yaw: 0.0,
            pitch: 0.0,
            fov_y: std::f32::consts::FRAC_PI_4, // 45 degrees
            aspect,
            near: 0.1,
            far: 100.0,
        }
    }

    /// Unit-length forward direction in world space.
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }

    /// Unit-length right direction (forward × Y axis).
    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    /// View matrix from eye looking along [`forward`](Self::forward).
    pub fn view_matrix(&self) -> Mat4 {
        let target = self.position + self.forward();
        Mat4::look_at(self.position, target, Vec3::Y)
    }

    /// Perspective projection matrix from the camera's `fov_y`/aspect/near/far.
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective(self.fov_y, self.aspect, self.near, self.far)
    }

    /// Combined `proj * view` ready to upload as a camera UBO.
    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }
}

/// FPS-style WASD/mouse controller for [`Camera`].
pub struct FpsController {
    /// Movement speed in world units per second.
    pub speed: f32,
    /// Mouse delta multiplier for yaw/pitch.
    pub mouse_sensitivity: f32,
}

impl Default for FpsController {
    fn default() -> Self {
        Self::new()
    }
}

impl FpsController {
    /// New controller with sensible defaults (5 units/s, 0.003 mouse).
    pub fn new() -> Self {
        Self { speed: 5.0, mouse_sensitivity: 0.003 }
    }

    /// Translates the camera by the per-frame `(forward, right, up)` input
    /// (in the camera's local frame), scaled by speed and `dt`.
    pub fn process_movement(
        &self, camera: &mut Camera,
        forward: f32, right: f32, up: f32, dt: f32,
    ) {
        let cam_forward = camera.forward();
        let cam_right = camera.right();
        let velocity = self.speed * dt;
        camera.position = camera.position
            + cam_forward * (forward * velocity)
            + cam_right * (right * velocity)
            + Vec3::Y * (up * velocity);
    }

    /// Applies a mouse-delta (`dx`, `dy` in screen pixels) to the camera's
    /// yaw and pitch, clamping pitch to ±89° to avoid gimbal flip.
    pub fn process_mouse(&self, camera: &mut Camera, dx: f64, dy: f64) {
        camera.yaw += dx as f32 * self.mouse_sensitivity;
        camera.pitch -= dy as f32 * self.mouse_sensitivity;
        let limit = 89.0_f32.to_radians();
        camera.pitch = camera.pitch.clamp(-limit, limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn vec3_approx_eq(a: Vec3, b: Vec3) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y) && approx_eq(a.z, b.z)
    }

    #[test]
    fn test_camera_default_forward() {
        let cam = Camera::new(Vec3::new(0.0, 0.0, 0.0), 1.0);
        // yaw=0, pitch=0 → forward ≈ (0, 0, -1)
        let fwd = cam.forward();
        assert!(
            vec3_approx_eq(fwd, Vec3::new(0.0, 0.0, -1.0)),
            "Expected (0, 0, -1), got ({}, {}, {})",
            fwd.x, fwd.y, fwd.z
        );
    }

    #[test]
    fn test_camera_yaw_90() {
        let mut cam = Camera::new(Vec3::new(0.0, 0.0, 0.0), 1.0);
        cam.yaw = PI / 2.0;
        // yaw=PI/2 → forward ≈ (1, 0, 0)
        let fwd = cam.forward();
        assert!(
            vec3_approx_eq(fwd, Vec3::new(1.0, 0.0, 0.0)),
            "Expected (1, 0, 0), got ({}, {}, {})",
            fwd.x, fwd.y, fwd.z
        );
    }

    #[test]
    fn test_fps_pitch_clamp() {
        let controller = FpsController::new();
        let mut cam = Camera::new(Vec3::new(0.0, 0.0, 0.0), 1.0);
        let limit = 89.0_f32.to_radians();

        // Extreme upward mouse movement
        controller.process_mouse(&mut cam, 0.0, -1_000_000.0);
        assert!(
            cam.pitch <= limit + 1e-5,
            "Pitch should be clamped to +89°, got {}",
            cam.pitch.to_degrees()
        );

        // Extreme downward mouse movement
        controller.process_mouse(&mut cam, 0.0, 1_000_000.0);
        assert!(
            cam.pitch >= -limit - 1e-5,
            "Pitch should be clamped to -89°, got {}",
            cam.pitch.to_degrees()
        );
    }
}
