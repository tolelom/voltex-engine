use voltex_ecs::{World, Entity};
use voltex_ecs::Transform;
use voltex_math::{Vec3, Ray};

use crate::collider::Collider;
use crate::bvh::BvhTree;
use crate::ray as ray_tests;

#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub entity: Entity,
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

pub fn raycast(world: &World, ray: &Ray, max_dist: f32) -> Option<RayHit> {
    let entities: Vec<(Entity, Vec3, Collider)> = world
        .query2::<Transform, Collider>()
        .into_iter()
        .map(|(e, t, c)| (e, t.position, *c))
        .collect();

    if entities.is_empty() {
        return None;
    }

    let mut closest: Option<RayHit> = None;

    for (entity, pos, collider) in &entities {
        let aabb = collider.aabb(*pos);

        // Broad phase: ray vs AABB
        let aabb_t = match ray_tests::ray_vs_aabb(ray, &aabb) {
            Some(t) if t <= max_dist => t,
            _ => continue,
        };

        // Early skip if we already have a closer hit
        if let Some(ref hit) = closest {
            if aabb_t >= hit.t {
                continue;
            }
        }

        // Narrow phase
        let result = match collider {
            Collider::Sphere { radius } => {
                ray_tests::ray_vs_sphere(ray, *pos, *radius)
            }
            Collider::Box { half_extents } => {
                ray_tests::ray_vs_box(ray, *pos, *half_extents)
            }
        };

        if let Some((t, normal)) = result {
            if t <= max_dist {
                if closest.is_none() || t < closest.as_ref().unwrap().t {
                    closest = Some(RayHit {
                        entity: *entity,
                        t,
                        point: ray.at(t),
                        normal,
                    });
                }
            }
        }
    }

    closest
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::World;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;
    use crate::Collider;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn test_empty_world() {
        let world = World::new();
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        assert!(raycast(&world, &ray, 100.0).is_none());
    }

    #[test]
    fn test_hit_sphere() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(5.0, 0.0, 0.0)));
        world.add(e, Collider::Sphere { radius: 1.0 });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hit = raycast(&world, &ray, 100.0).unwrap();

        assert_eq!(hit.entity, e);
        assert!(approx(hit.t, 4.0));
        assert!(approx(hit.point.x, 4.0));
    }

    #[test]
    fn test_closest_of_multiple() {
        let mut world = World::new();

        let far = world.spawn();
        world.add(far, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        world.add(far, Collider::Sphere { radius: 1.0 });

        let near = world.spawn();
        world.add(near, Transform::from_position(Vec3::new(3.0, 0.0, 0.0)));
        world.add(near, Collider::Sphere { radius: 1.0 });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hit = raycast(&world, &ray, 100.0).unwrap();

        assert_eq!(hit.entity, near);
        assert!(approx(hit.t, 2.0));
    }

    #[test]
    fn test_max_dist() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(50.0, 0.0, 0.0)));
        world.add(e, Collider::Sphere { radius: 1.0 });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        assert!(raycast(&world, &ray, 10.0).is_none());
    }

    #[test]
    fn test_hit_box() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(5.0, 0.0, 0.0)));
        world.add(e, Collider::Box { half_extents: Vec3::ONE });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hit = raycast(&world, &ray, 100.0).unwrap();

        assert_eq!(hit.entity, e);
        assert!(approx(hit.t, 4.0));
    }

    #[test]
    fn test_mixed_sphere_box() {
        let mut world = World::new();

        let sphere = world.spawn();
        world.add(sphere, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        world.add(sphere, Collider::Sphere { radius: 1.0 });

        let box_e = world.spawn();
        world.add(box_e, Transform::from_position(Vec3::new(3.0, 0.0, 0.0)));
        world.add(box_e, Collider::Box { half_extents: Vec3::ONE });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hit = raycast(&world, &ray, 100.0).unwrap();

        assert_eq!(hit.entity, box_e);
        assert!(approx(hit.t, 2.0));
    }
}
