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
    /// callers can pass an unnormalised vector.
    ///
    /// **Beware**: passing [`Vec3::ZERO`] does *not* panic — it stores
    /// a NaN direction (see [`Vec3::normalize`]). Downstream code that
    /// branches on `direction` (raycasts, picking) will then behave
    /// unpredictably. Validate or use [`Ray::try_new`] when the
    /// direction is computed from external data.
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Construct a ray, returning `None` if `direction` is the zero
    /// vector or has subnormal length. Prefer this over [`Ray::new`]
    /// when the direction comes from untrusted or computed data.
    pub fn try_new(origin: Vec3, direction: Vec3) -> Option<Self> {
        if direction.length_squared() < f32::MIN_POSITIVE {
            None
        } else {
            Some(Self {
                origin,
                direction: direction.normalize(),
            })
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

    #[test]
    fn try_new_rejects_zero_direction() {
        assert!(Ray::try_new(Vec3::ZERO, Vec3::ZERO).is_none());
    }

    #[test]
    fn try_new_accepts_nonzero_direction() {
        let r = Ray::try_new(Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)).expect("should succeed");
        assert!(approx_vec(r.direction, Vec3::X));
    }
}
