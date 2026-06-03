//! Free-flying first-person camera + WASD/mouse controller.

use voltex_math::{Mat4, Vec3};

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
    /// When `Some(h)`, use an orthographic projection with view height `h`
    /// (width `h * aspect`) instead of perspective — for 2D / top-down games.
    /// `None` (default) = perspective from `fov_y`.
    pub ortho_height: Option<f32>,
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
            ortho_height: None,
        }
    }

    /// New orthographic camera at `position` framing a vertical span of
    /// `height` world units (width = `height * aspect`). For 2D / top-down
    /// games. Near/far default to a generous `-100..100` style range so
    /// objects on either side of the eye plane stay visible.
    pub fn orthographic(position: Vec3, aspect: f32, height: f32) -> Self {
        let mut c = Self::new(position, aspect);
        c.ortho_height = Some(height);
        c.near = -100.0;
        c.far = 100.0;
        c
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
    ///
    /// At pitch ±90° the forward vector is parallel to `Vec3::Y`, so the
    /// cross product is zero and `.normalize()` would produce NaN.
    /// `FpsController::process_mouse` clamps pitch to ±89° to avoid this,
    /// but `pitch` is a public field — user code that sets it directly
    /// would otherwise leak NaN into every transform built from `right()`.
    /// Falls back to `Vec3::X` for the degenerate case.
    pub fn right(&self) -> Vec3 {
        let r = self.forward().cross(Vec3::Y);
        let n = r.normalize_or_zero();
        if n == Vec3::ZERO {
            Vec3::X
        } else {
            n
        }
    }

    /// View matrix from eye looking along [`forward`](Self::forward).
    pub fn view_matrix(&self) -> Mat4 {
        let target = self.position + self.forward();
        Mat4::look_at(self.position, target, Vec3::Y)
    }

    /// Projection matrix: orthographic when [`ortho_height`](Self::ortho_height)
    /// is set, otherwise perspective from `fov_y`/aspect/near/far.
    pub fn projection_matrix(&self) -> Mat4 {
        match self.ortho_height {
            Some(h) => {
                let half_h = h * 0.5;
                let half_w = half_h * self.aspect;
                Mat4::orthographic(-half_w, half_w, -half_h, half_h, self.near, self.far)
            }
            None => Mat4::perspective(self.fov_y, self.aspect, self.near, self.far),
        }
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
        Self {
            speed: 5.0,
            mouse_sensitivity: 0.003,
        }
    }

    /// Translates the camera by the per-frame `(forward, right, up)` input
    /// (in the camera's local frame), scaled by speed and `dt`.
    pub fn process_movement(
        &self,
        camera: &mut Camera,
        forward: f32,
        right: f32,
        up: f32,
        dt: f32,
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
            fwd.x,
            fwd.y,
            fwd.z
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
            fwd.x,
            fwd.y,
            fwd.z
        );
    }

    #[test]
    fn camera_right_at_straight_up_does_not_nan() {
        // Pre-fix: pitch = +90° made forward ≈ (0,1,0); the cross product
        // with Vec3::Y collapsed to a sub-normal vector whose normalize()
        // produced NaN. In f32, cos(π/2) doesn't return exactly 0, so the
        // cross product is a tiny vector along ±X — small enough to push
        // through `normalize_or_zero` and into NaN before the fallback was
        // added.
        let mut cam = Camera::new(Vec3::ZERO, 1.0);
        cam.pitch = std::f32::consts::FRAC_PI_2; // exactly +90°
        let r = cam.right();
        // No NaN anywhere.
        assert!(!r.x.is_nan() && !r.y.is_nan() && !r.z.is_nan());
        // Result is unit-length along the X axis (sign depends on f32 noise).
        assert!(
            (r.length() - 1.0).abs() < 1e-4,
            "right() must be unit length, got {:?}",
            r
        );
        assert!(r.y.abs() < 1e-4);
        assert!(r.z.abs() < 1e-4);
        assert!(r.x.abs() > 0.99);
    }

    #[test]
    fn camera_right_at_straight_down_does_not_nan() {
        let mut cam = Camera::new(Vec3::ZERO, 1.0);
        cam.pitch = -std::f32::consts::FRAC_PI_2;
        let r = cam.right();
        assert!(!r.x.is_nan() && !r.y.is_nan() && !r.z.is_nan());
        assert!((r.length() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn orthographic_has_no_perspective_divide() {
        // Camera at (0,0,5) looking -Z, ortho height 10, aspect 1.
        let cam = Camera::orthographic(Vec3::new(0.0, 0.0, 5.0), 1.0, 10.0);
        assert_eq!(cam.ortho_height, Some(10.0));
        let vp = cam.view_projection();
        // Two points at x=2, same Y, but different depths. Under orthographic
        // projection their clip x (after w-divide) must be identical — no
        // perspective foreshortening.
        let near = vp.mul_vec4(voltex_math::Vec4::new(2.0, 0.0, 0.0, 1.0));
        let far = vp.mul_vec4(voltex_math::Vec4::new(2.0, 0.0, -4.0, 1.0));
        let nx = near.x / near.w;
        let fx = far.x / far.w;
        assert!(
            (nx - fx).abs() < 1e-5,
            "ortho: clip x should not depend on depth, got {nx} vs {fx}"
        );
        // x=2 with half-width 5 → NDC x = 0.4.
        assert!((nx - 0.4).abs() < 1e-4, "expected NDC x≈0.4, got {nx}");
    }

    #[test]
    fn perspective_does_foreshorten() {
        let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), 1.0); // perspective
        let vp = cam.view_projection();
        let near = vp.mul_vec4(voltex_math::Vec4::new(2.0, 0.0, 0.0, 1.0));
        let far = vp.mul_vec4(voltex_math::Vec4::new(2.0, 0.0, -4.0, 1.0));
        let nx = near.x / near.w;
        let fx = far.x / far.w;
        // Farther point projects closer to center under perspective.
        assert!(
            fx < nx,
            "perspective: farther point should foreshorten toward center, got near={nx} far={fx}"
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
