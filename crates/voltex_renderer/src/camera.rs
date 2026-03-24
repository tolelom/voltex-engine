use voltex_math::{Vec3, Mat4};

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // radians, Y-axis rotation
    pub pitch: f32,  // radians, X-axis rotation
    pub fov_y: f32,  // radians
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
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

    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn view_matrix(&self) -> Mat4 {
        let target = self.position + self.forward();
        Mat4::look_at(self.position, target, Vec3::Y)
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective(self.fov_y, self.aspect, self.near, self.far)
    }

    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }
}

pub struct FpsController {
    pub speed: f32,
    pub mouse_sensitivity: f32,
}

impl FpsController {
    pub fn new() -> Self {
        Self { speed: 5.0, mouse_sensitivity: 0.003 }
    }

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
