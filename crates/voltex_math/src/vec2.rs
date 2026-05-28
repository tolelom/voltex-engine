//! 2D `f32` vector + basic operator overloads.

use std::ops::{Add, Mul, Neg, Sub};

/// Two-component `f32` vector. Used for 2D positions, UVs, and
/// screen-space coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
}

impl Vec2 {
    /// `(0, 0)`.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    /// `(1, 1)`.
    pub const ONE: Self = Self { x: 1.0, y: 1.0 };

    /// Construct a new vector.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Dot product `self · rhs`.
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y
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
    /// [`Vec2::ZERO`] therefore returns a NaN-filled vector which then
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
        }
    }

    /// NaN-safe normalisation. Returns [`Vec2::ZERO`] if the input is
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
            }
        }
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Neg for Vec2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = Vec2::new(1.0, 2.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
    }

    #[test]
    fn test_add() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        let c = a + b;
        assert_eq!(c, Vec2::new(4.0, 6.0));
    }

    #[test]
    fn test_dot() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        assert_eq!(a.dot(b), 11.0);
    }

    #[test]
    fn test_length() {
        let v = Vec2::new(3.0, 4.0);
        assert!((v.length() - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize() {
        let v = Vec2::new(4.0, 0.0);
        let n = v.normalize();
        assert!((n.length() - 1.0).abs() < 1e-6);
        assert_eq!(n, Vec2::new(1.0, 0.0));
    }

    #[test]
    fn normalize_zero_produces_nan() {
        // Documents the known-bad behaviour explicitly.
        let n = Vec2::ZERO.normalize();
        assert!(n.x.is_nan() && n.y.is_nan());
    }

    #[test]
    fn normalize_or_zero_returns_zero_for_zero_input() {
        assert_eq!(Vec2::ZERO.normalize_or_zero(), Vec2::ZERO);
    }

    #[test]
    fn normalize_or_zero_matches_normalize_for_nonzero_input() {
        let v = Vec2::new(3.0, 4.0);
        let a = v.normalize();
        let b = v.normalize_or_zero();
        assert!((a.x - b.x).abs() < 1e-6);
        assert!((a.y - b.y).abs() < 1e-6);
        assert!((b.length() - 1.0).abs() < 1e-6);
    }
}
