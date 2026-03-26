use voltex_math::{Vec3, AABB};

/// A convex hull collider defined by a set of vertices.
#[derive(Debug, Clone)]
pub struct ConvexHull {
    pub vertices: Vec<Vec3>,
}

impl ConvexHull {
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
}

#[derive(Debug, Clone)]
pub enum Collider {
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
    Capsule { radius: f32, half_height: f32 },
    ConvexHull(ConvexHull),
}

impl Collider {
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
                let mut min = position + hull.vertices[0];
                let mut max = min;
                for &v in &hull.vertices[1..] {
                    let p = position + v;
                    min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
                    max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
                }
                AABB::new(min, max)
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
