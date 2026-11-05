//! Collision shape primitives — Sphere / Box / Capsule / ConvexHull —
//! plus their world-space AABB derivation.

use voltex_math::{Vec3, AABB};

/// A convex hull collider defined by a set of vertices.
#[derive(Debug, Clone)]
pub struct ConvexHull {
    /// Local-space vertex positions defining the hull.
    pub vertices: Vec<Vec3>,
}

impl ConvexHull {
    /// Wraps a vertex list into a `ConvexHull`. The caller is responsible for
    /// the points actually defining a convex set.
    pub fn new(vertices: Vec<Vec3>) -> Self {
        ConvexHull { vertices }
    }

    /// Support function for GJK: returns the vertex farthest in the given direction.
    pub fn support(&self, direction: Vec3) -> Vec3 {
        let mut best = self.vertices[0];
        let mut best_dot = best.dot(direction);
        for &v in &self.vertices[1..] {
            let d = v.dot(direction);
            if d > best_dot {
                best_dot = d;
                best = v;
            }
        }
        best
    }

    /// Local-space AABB of the hull's vertices (panics on an empty hull,
    /// like [`ConvexHull::support`]).
    pub fn local_aabb(&self) -> AABB {
        let first = self.vertices[0];
        let (min, max) = self.vertices[1..]
            .iter()
            .fold((first, first), |(min, max), &v| (min.min(v), max.max(v)));
        AABB::new(min, max)
    }
}

/// Supported collision shapes.
#[derive(Debug, Clone)]
pub enum Collider {
    /// Sphere centred at the entity transform.
    Sphere {
        /// Sphere radius.
        radius: f32,
    },
    /// Axis-aligned box (in the entity local frame).
    Box {
        /// Half-extents along each axis.
        half_extents: Vec3,
    },
    /// Y-axis capsule (cylinder + two hemispherical caps).
    Capsule {
        /// Capsule cap radius.
        radius: f32,
        /// Distance from the centre to either cap centre.
        half_height: f32,
    },
    /// Arbitrary convex polytope.
    ConvexHull(ConvexHull),
}

impl Collider {
    /// World-space AABB enclosing this collider when placed at `position`.
    pub fn aabb(&self, position: Vec3) -> AABB {
        match self {
            Collider::Sphere { radius } => {
                let r = Vec3::new(*radius, *radius, *radius);
                AABB::new(position - r, position + r)
            }
            Collider::Box { half_extents } => {
                AABB::new(position - *half_extents, position + *half_extents)
            }
            Collider::Capsule { radius, half_height } => {
                let r = Vec3::new(*radius, *half_height + *radius, *radius);
                AABB::new(position - r, position + r)
            }
            Collider::ConvexHull(hull) => {
                let local = hull.local_aabb();
                AABB::new(position + local.min, position + local.max)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_aabb() {
        let c = Collider::Sphere { radius: 2.0 };
        let aabb = c.aabb(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(aabb.min, Vec3::new(-1.0, -2.0, -2.0));
        assert_eq!(aabb.max, Vec3::new(3.0, 2.0, 2.0));
    }

    #[test]
    fn test_box_aabb() {
        let c = Collider::Box { half_extents: Vec3::new(1.0, 2.0, 3.0) };
        let aabb = c.aabb(Vec3::ZERO);
        assert_eq!(aabb.min, Vec3::new(-1.0, -2.0, -3.0));
        assert_eq!(aabb.max, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_capsule_aabb() {
        let c = Collider::Capsule { radius: 0.5, half_height: 1.0 };
        let aabb = c.aabb(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(aabb.min, Vec3::new(0.5, 0.5, 2.5));
        assert_eq!(aabb.max, Vec3::new(1.5, 3.5, 3.5));
    }

    #[test]
    fn test_convex_hull_support() {
        let hull = ConvexHull::new(vec![
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, -1.0, 1.0),
        ]);
        // Support in +Y direction should return the top vertex
        let s = hull.support(Vec3::new(0.0, 1.0, 0.0));
        assert!((s.y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_convex_hull_support_negative() {
        let hull = ConvexHull::new(vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ]);
        let s = hull.support(Vec3::new(-1.0, 0.0, 0.0));
        assert!((s.x - 0.0).abs() < 1e-6); // origin is farthest in -X
    }

    #[test]
    fn convex_hull_aabb_is_local_aabb_moved_to_position() {
        let hull = ConvexHull::new(vec![
            Vec3::new(-1.0, 0.0, 2.0),
            Vec3::new(3.0, -2.0, 0.0),
            Vec3::new(0.0, 1.0, -1.0),
        ]);
        let local = hull.local_aabb();
        assert_eq!(local.min, Vec3::new(-1.0, -2.0, -1.0));
        assert_eq!(local.max, Vec3::new(3.0, 1.0, 2.0));
        let world = Collider::ConvexHull(hull).aabb(Vec3::new(10.0, 0.0, 0.0));
        assert_eq!(world.min, Vec3::new(9.0, -2.0, -1.0));
        assert_eq!(world.max, Vec3::new(13.0, 1.0, 2.0));
    }

    #[test]
    fn test_convex_hull_in_collider() {
        // Test that ConvexHull variant works with existing collision system
        let hull = ConvexHull::new(vec![
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ]);
        // Just verify construction works
        assert_eq!(hull.vertices.len(), 8);
    }
}
