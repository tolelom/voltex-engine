//! Orbit camera for the scene viewport — pitch/yaw + distance, mouse-drag
//! orbit + middle-click pan + scroll zoom.

use voltex_math::{Vec3, Mat4};
use std::f32::consts::PI;

const PITCH_LIMIT: f32 = PI / 2.0 - 0.01;
const MIN_DISTANCE: f32 = 0.5;
const MAX_DISTANCE: f32 = 50.0;
const ORBIT_SENSITIVITY: f32 = 0.005;
const ZOOM_FACTOR: f32 = 0.1;
const PAN_SENSITIVITY: f32 = 0.01;

/// Orbit camera that pivots around a target point.
pub struct OrbitCamera {
    /// World-space point the camera orbits around.
    pub target: Vec3,
    /// Distance from `target` to the eye, in world units.
    pub distance: f32,
    /// Yaw in radians (rotation around the world Y axis).
    pub yaw: f32,
    /// Pitch in radians (clamped near ±π/2 to avoid gimbal flip).
    pub pitch: f32,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Near clipping plane distance.
    pub near: f32,
    /// Far clipping plane distance.
    pub far: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self::new()
    }
}

impl OrbitCamera {
    /// New camera with sensible defaults (5-unit distance, slight downward pitch).
    pub fn new() -> Self {
        OrbitCamera {
            target: Vec3::ZERO,
            distance: 5.0,
            yaw: 0.0,
            pitch: 0.3,
            fov_y: PI / 4.0,
            near: 0.1,
            far: 100.0,
        }
    }

    /// Eye position in world space, derived from yaw/pitch/distance/target.
    pub fn position(&self) -> Vec3 {
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        Vec3::new(
            self.target.x + self.distance * cp * sy,
            self.target.y + self.distance * sp,
            self.target.z + self.distance * cp * cy,
        )
    }

    /// Applies a mouse-drag orbit (`dx`/`dy` in pixels), clamping pitch.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * ORBIT_SENSITIVITY;
        self.pitch += dy * ORBIT_SENSITIVITY;
        self.pitch = self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Scroll-wheel zoom; positive `delta` zooms in, clamped to `[MIN,MAX]`.
    pub fn zoom(&mut self, delta: f32) {
        self.distance *= 1.0 - delta * ZOOM_FACTOR;
        self.distance = self.distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Middle-drag pan — translates the target along the camera's
    /// right/up axes by `(dx, dy)` scaled by distance.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let forward = (self.target - self.position()).normalize();
        let right = forward.cross(Vec3::Y);
        let right = if right.length() < 1e-4 { Vec3::X } else { right.normalize() };
        let up = right.cross(forward).normalize();
        let offset_x = right * (-dx * PAN_SENSITIVITY * self.distance);
        let offset_y = up * (dy * PAN_SENSITIVITY * self.distance);
        self.target = self.target + offset_x + offset_y;
    }

    /// View matrix from the current eye position looking at `target`.
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at(self.position(), self.target, Vec3::Y)
    }

    /// Perspective projection matrix for the given viewport aspect.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective(self.fov_y, aspect, self.near, self.far)
    }

    /// Combined `proj * view`, ready to upload as a camera UBO.
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection_matrix(aspect).mul_mat4(&self.view_matrix())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_position() {
        let cam = OrbitCamera::new();
        let pos = cam.position();
        assert!((pos.x).abs() < 1e-3);
        assert!(pos.y > 0.0);
        assert!(pos.z > 0.0);
    }

    #[test]
    fn test_orbit_changes_yaw_pitch() {
        let mut cam = OrbitCamera::new();
        let old_yaw = cam.yaw;
        let old_pitch = cam.pitch;
        cam.orbit(100.0, 50.0);
        assert!((cam.yaw - old_yaw - 100.0 * ORBIT_SENSITIVITY).abs() < 1e-6);
        assert!((cam.pitch - old_pitch - 50.0 * ORBIT_SENSITIVITY).abs() < 1e-6);
    }

    #[test]
    fn test_pitch_clamped() {
        let mut cam = OrbitCamera::new();
        cam.orbit(0.0, 100000.0);
        assert!(cam.pitch <= PITCH_LIMIT);
        cam.orbit(0.0, -200000.0);
        assert!(cam.pitch >= -PITCH_LIMIT);
    }

    #[test]
    fn test_zoom_changes_distance() {
        let mut cam = OrbitCamera::new();
        let d0 = cam.distance;
        cam.zoom(1.0);
        assert!(cam.distance < d0);
    }

    #[test]
    fn test_zoom_clamped() {
        let mut cam = OrbitCamera::new();
        cam.zoom(1000.0);
        assert!(cam.distance >= MIN_DISTANCE);
        cam.zoom(-10000.0);
        assert!(cam.distance <= MAX_DISTANCE);
    }

    #[test]
    fn test_pan_moves_target() {
        let mut cam = OrbitCamera::new();
        let t0 = cam.target;
        cam.pan(10.0, 0.0);
        assert!((cam.target.x - t0.x).abs() > 1e-4 || (cam.target.z - t0.z).abs() > 1e-4);
    }

    #[test]
    fn test_view_matrix_not_zero() {
        let cam = OrbitCamera::new();
        let v = cam.view_matrix();
        let sum: f32 = v.cols.iter().flat_map(|c| c.iter()).map(|x| x.abs()).sum();
        assert!(sum > 1.0);
    }

    #[test]
    fn test_projection_matrix() {
        let cam = OrbitCamera::new();
        let p = cam.projection_matrix(16.0 / 9.0);
        assert!(p.cols[0][0] > 0.0);
    }

    #[test]
    fn test_view_projection() {
        let cam = OrbitCamera::new();
        let vp = cam.view_projection(1.0);
        let v = cam.view_matrix();
        let p = cam.projection_matrix(1.0);
        let expected = p.mul_mat4(&v);
        for i in 0..4 {
            for j in 0..4 {
                assert!((vp.cols[i][j] - expected.cols[i][j]).abs() < 1e-4,
                    "mismatch at [{i}][{j}]: {} vs {}", vp.cols[i][j], expected.cols[i][j]);
            }
        }
    }
}
