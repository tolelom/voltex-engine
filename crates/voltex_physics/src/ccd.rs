//! Continuous collision detection primitives — currently swept-sphere-vs-AABB.

use voltex_math::{Vec3, AABB, Ray};
use crate::ray::ray_vs_aabb;

/// Swept sphere vs AABB continuous collision detection.
/// Expands the AABB by the sphere radius, then tests a ray from start to end.
/// Returns `t` in `[0, 1]` of first contact, or `None` if no contact.
pub fn swept_sphere_vs_aabb(start: Vec3, end: Vec3, radius: f32, aabb: &AABB) -> Option<f32> {
    // Expand AABB by sphere radius
    let r = Vec3::new(radius, radius, radius);
    let expanded = AABB::new(aabb.min - r, aabb.max + r);

    let direction = end - start;
    let sweep_len = direction.length();

    if sweep_len < 1e-10 {
        // No movement — check if already inside
        if expanded.contains_point(start) {
            return Some(0.0);
        }
        return None;
    }

    let ray = Ray::new(start, direction * (1.0 / sweep_len));

    match ray_vs_aabb(&ray, &expanded) {
        Some(t) => {
            let parametric_t = t / sweep_len;
            if parametric_t <= 1.0 {
                Some(parametric_t)
            } else {
                None
            }
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn test_swept_sphere_hits_aabb() {
        let start = Vec3::new(-10.0, 0.0, 0.0);
        let end = Vec3::new(10.0, 0.0, 0.0);
        let radius = 0.5;
        let aabb = AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0));

        let t = swept_sphere_vs_aabb(start, end, radius, &aabb).unwrap();
        // Expanded AABB min.x = 3.5, start.x = -10, direction = 20
        // t = (3.5 - (-10)) / 20 = 13.5 / 20 = 0.675
        assert!(t > 0.0 && t < 1.0);
        assert!(approx(t, 0.675));
    }

    #[test]
    fn test_swept_sphere_misses_aabb() {
        let start = Vec3::new(-10.0, 10.0, 0.0);
        let end = Vec3::new(10.0, 10.0, 0.0);
        let radius = 0.5;
        let aabb = AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0));

        assert!(swept_sphere_vs_aabb(start, end, radius, &aabb).is_none());
    }

    #[test]
    fn test_swept_sphere_starts_inside() {
        let start = Vec3::new(5.0, 0.0, 0.0);
        let end = Vec3::new(10.0, 0.0, 0.0);
        let radius = 0.5;
        let aabb = AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0));

        let t = swept_sphere_vs_aabb(start, end, radius, &aabb).unwrap();
        assert!(approx(t, 0.0));
    }

    #[test]
    fn test_swept_sphere_tunneling_detection() {
        // Fast sphere that would tunnel through a thin wall
        let start = Vec3::new(-100.0, 0.0, 0.0);
        let end = Vec3::new(100.0, 0.0, 0.0);
        let radius = 0.1;
        // Thin wall at x=0
        let aabb = AABB::new(Vec3::new(-0.05, -10.0, -10.0), Vec3::new(0.05, 10.0, 10.0));

        let t = swept_sphere_vs_aabb(start, end, radius, &aabb);
        assert!(t.is_some(), "should detect tunneling through thin wall");
        let t = t.unwrap();
        assert!(t > 0.0 && t < 1.0);
    }

    #[test]
    fn test_swept_sphere_no_movement() {
        let start = Vec3::new(5.0, 0.0, 0.0);
        let end = Vec3::new(5.0, 0.0, 0.0);
        let radius = 0.5;
        let aabb = AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0));

        // Inside expanded AABB, so should return 0
        let t = swept_sphere_vs_aabb(start, end, radius, &aabb).unwrap();
        assert!(approx(t, 0.0));
    }

    #[test]
    fn test_swept_sphere_beyond_range() {
        let start = Vec3::new(-10.0, 0.0, 0.0);
        let end = Vec3::new(-5.0, 0.0, 0.0);
        let radius = 0.5;
        let aabb = AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0));

        // AABB is at x=4..6, moving from -10 to -5 won't reach it
        assert!(swept_sphere_vs_aabb(start, end, radius, &aabb).is_none());
    }
}
