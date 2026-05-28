//! 3D `f32` vector — positions, directions, normals.

use std::ops::{Add, Mul, Neg, Sub};

/// Three-component `f32` vector. The fundamental position / direction
/// type in voltex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
}

impl Vec3 {
    /// `(0, 0, 0)`.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    /// `(1, 1, 1)`.
    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
    /// `(1, 0, 0)` — right.
    pub const X: Self = Self {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    /// `(0, 1, 0)` — up.
    pub const Y: Self = Self {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    /// `(0, 0, 1)` — forward (right-handed: out of the screen).
    pub const Z: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    /// Construct a new vector.
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Dot product `self · rhs`.
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    /// Cross product `self × rhs`. Right-handed.
    pub fn cross(self, rhs: Self) -> Self {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }

    /// Squared length `|self|²`. Cheaper than [`length`](Self::length)
    /// when only relative magnitudes are needed.
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    /// Euclidean length `|self|`.
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Returns a unit-length vector pointing in the same direction.
    ///
    /// **Beware**: dividing an `f32` by 0 does not panic — it produces
    /// `NaN` (when the numerator is also 0) or `±inf`. Passing
    /// [`Vec3::ZERO`] therefore returns a NaN-filled vector which then
    /// silently corrupts downstream computations.
    ///
    /// Use [`normalize_or_zero`](Self::normalize_or_zero) for the
    /// NaN-safe version. (Earlier docstrings claimed this method
    /// panicked on zero input; it does not.)
    pub fn normalize(self) -> Self {
        let len = self.length();
        Self {
            x: self.x / len,
            y: self.y / len,
            z: self.z / len,
        }
    }

    /// NaN-safe normalisation. Returns [`Vec3::ZERO`] if the input is
    /// the zero vector (or any vector with length below
    /// [`f32::MIN_POSITIVE`]). Otherwise behaves like
    /// [`normalize`](Self::normalize).
    pub fn normalize_or_zero(self) -> Self {
        let len_sq = self.length_squared();
        if len_sq < f32::MIN_POSITIVE {
            Self::ZERO
        } else {
            let len = len_sq.sqrt();
            Self {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
            }
        }
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.z, 3.0);
    }

    #[test]
    fn test_zero() {
        let v = Vec3::ZERO;
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.z, 0.0);
    }

    #[test]
    fn test_add() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        let c = a + b;
        assert_eq!(c, Vec3::new(5.0, 7.0, 9.0));
    }

    #[test]
    fn test_sub() {
        let a = Vec3::new(4.0, 5.0, 6.0);
        let b = Vec3::new(1.0, 2.0, 3.0);
        let c = a - b;
        assert_eq!(c, Vec3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn test_scalar_mul() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let r = v * 2.0;
        assert_eq!(r, Vec3::new(2.0, 4.0, 6.0));
    }

    #[test]
    fn test_dot() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a.dot(b), 32.0);
    }

    #[test]
    fn test_cross() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        let c = a.cross(b);
        assert_eq!(c, Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_length() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        assert!((v.length() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_normalize() {
        let v = Vec3::new(3.0, 0.0, 0.0);
        let n = v.normalize();
        assert!((n.length() - 1.0).abs() < 1e-6);
        assert_eq!(n, Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn normalize_zero_produces_nan() {
        // Documenting the known-bad behaviour explicitly so a future change
        // doesn't silently flip semantics. f32::NAN != f32::NAN, hence is_nan().
        let n = Vec3::ZERO.normalize();
        assert!(n.x.is_nan() && n.y.is_nan() && n.z.is_nan());
    }

    #[test]
    fn normalize_or_zero_returns_zero_for_zero_input() {
        assert_eq!(Vec3::ZERO.normalize_or_zero(), Vec3::ZERO);
    }

    #[test]
    fn normalize_or_zero_matches_normalize_for_nonzero_input() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        let a = v.normalize();
        let b = v.normalize_or_zero();
        assert!((a.x - b.x).abs() < 1e-6);
        assert!((a.y - b.y).abs() < 1e-6);
        assert!((a.z - b.z).abs() < 1e-6);
        assert!((b.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_or_zero_handles_subnormal_length() {
        // A vector whose squared length underflows to 0 still triggers the
        // safe branch.
        let v = Vec3::new(1e-30, 0.0, 0.0);
        let n = v.normalize_or_zero();
        // length² < f32::MIN_POSITIVE in this case, so we return ZERO.
        assert_eq!(n, Vec3::ZERO);
    }

    #[test]
    fn test_neg() {
        let v = Vec3::new(1.0, -2.0, 3.0);
        let n = -v;
        assert_eq!(n, Vec3::new(-1.0, 2.0, -3.0));
    }
}
