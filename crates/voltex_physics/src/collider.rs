use voltex_math::{Vec3, AABB};

#[derive(Debug, Clone, Copy)]
pub enum Collider {
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
    Capsule { radius: f32, half_height: f32 },
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
}
