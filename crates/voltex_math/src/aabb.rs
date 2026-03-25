// crates/voltex_math/src/aabb.rs
use crate::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_center_half_extents(center: Vec3, half: Vec3) -> Self {
        Self {
            min: center - half,
            max: center + half,
        }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn merged(&self, other: &AABB) -> AABB {
        AABB {
            min: Vec3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Vec3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    pub fn surface_area(&self) -> f32 {
        let d = self.max - self.min;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_accessors() {
        let a = AABB::new(Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0));
        let c = a.center();
        assert!((c.x).abs() < 1e-6);
        assert!((c.y).abs() < 1e-6);
        assert!((c.z).abs() < 1e-6);
        let h = a.half_extents();
        assert!((h.x - 1.0).abs() < 1e-6);
        assert!((h.y - 2.0).abs() < 1e-6);
        assert!((h.z - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_from_center_half_extents() {
        let a = AABB::from_center_half_extents(Vec3::new(5.0, 5.0, 5.0), Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(a.min, Vec3::new(4.0, 4.0, 4.0));
        assert_eq!(a.max, Vec3::new(6.0, 6.0, 6.0));
    }

    #[test]
    fn test_contains_point() {
        let a = AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        assert!(a.contains_point(Vec3::new(1.0, 1.0, 1.0)));
        assert!(a.contains_point(Vec3::ZERO));
        assert!(!a.contains_point(Vec3::new(3.0, 1.0, 1.0)));
    }

    #[test]
    fn test_intersects() {
        let a = AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let b = AABB::new(Vec3::new(1.0, 1.0, 1.0), Vec3::new(3.0, 3.0, 3.0));
        assert!(a.intersects(&b));

        let c = AABB::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0));
        assert!(!a.intersects(&c));

        let d = AABB::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 2.0, 2.0));
        assert!(a.intersects(&d));
    }

    #[test]
    fn test_merged() {
        let a = AABB::new(Vec3::ZERO, Vec3::ONE);
        let b = AABB::new(Vec3::new(2.0, 2.0, 2.0), Vec3::new(3.0, 3.0, 3.0));
        let m = a.merged(&b);
        assert_eq!(m.min, Vec3::ZERO);
        assert_eq!(m.max, Vec3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn test_surface_area() {
        let a = AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        assert!((a.surface_area() - 24.0).abs() < 1e-6);
    }
}
