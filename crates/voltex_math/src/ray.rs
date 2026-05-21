//! Infinite ray with unit-length direction — picking / raycasting primitive.

use crate::Vec3;

/// A ray starting at `origin` and travelling in unit-length `direction`.
/// Construction always normalises the direction.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    /// Origin point.
    pub origin: Vec3,
    /// Unit-length direction.
    pub direction: Vec3,
}

impl Ray {
    /// Construct a ray. The direction is normalised before storage —
    /// callers can pass an unnormalised vector. Panics if direction is
    /// the zero vector.
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Point along the ray at parameter `t`: `origin + direction * t`.
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn approx_vec(a: Vec3, b: Vec3) -> bool {
        approx(a.x, b.x) && approx(a.y, b.y) && approx(a.z, b.z)
    }

    #[test]
    fn test_new_normalizes_direction() {
        let r = Ray::new(Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0));
        assert!(approx_vec(r.direction, Vec3::X));
    }

    #[test]
    fn test_at() {
        let r = Ray::new(Vec3::new(1.0, 2.0, 3.0), Vec3::X);
        let p = r.at(5.0);
        assert!(approx_vec(p, Vec3::new(6.0, 2.0, 3.0)));
    }
}
