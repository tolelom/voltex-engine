//! World-level raycasting — broadphase ray-vs-AABB filter + per-shape
//! narrowphase. Returns the closest hit or every hit sorted by distance.

use voltex_ecs::{World, Entity};
use voltex_ecs::Transform;
use voltex_math::{Vec3, Ray};

use crate::collider::Collider;
use crate::ray as ray_tests;

/// One ray hit produced by [`raycast`] / [`raycast_all`].
#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    /// Entity whose collider was hit.
    pub entity: Entity,
    /// Distance along the ray to the hit point.
    pub t: f32,
    /// Hit point in world space.
    pub point: Vec3,
    /// Surface normal at the hit point.
    pub normal: Vec3,
}

/// Casts `ray` against every entity with a [`Transform`] + [`Collider`] and
/// returns the closest hit within `max_dist`, or `None`.
pub fn raycast(world: &World, ray: &Ray, max_dist: f32) -> Option<RayHit> {
    let entities: Vec<(Entity, Vec3, Collider)> = world
        .query2::<Transform, Collider>()
        .into_iter()
        .map(|(e, t, c)| (e, t.position, c.clone()))
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
            Collider::Capsule { radius, half_height } => {
                ray_tests::ray_vs_capsule(ray, *pos, *radius, *half_height)
            }
            Collider::ConvexHull(_) => {
                // Use AABB test as approximation for convex hull raycasting
                let aabb = collider.aabb(*pos);
                ray_tests::ray_vs_aabb(ray, &aabb).map(|t| (t, Vec3::Y))
            }
        };

        if let Some((t, normal)) = result {
            if t <= max_dist
                && (closest.is_none() || t < closest.as_ref().unwrap().t) {
                    closest = Some(RayHit {
                        entity: *entity,
                        t,
                        point: ray.at(t),
                        normal,
                    });
                }
        }
    }

    closest
}

/// Cast a ray and return ALL hits sorted by distance.
pub fn raycast_all(world: &World, ray: &Ray, max_dist: f32) -> Vec<RayHit> {
    let entities: Vec<(Entity, Vec3, Collider)> = world
        .query2::<Transform, Collider>()
        .into_iter()
        .map(|(e, t, c)| (e, t.position, c.clone()))
        .collect();

    if entities.is_empty() {
        return Vec::new();
    }

    let mut hits = Vec::new();

    for (entity, pos, collider) in &entities {
        let aabb = collider.aabb(*pos);

        // Broad phase: ray vs AABB
        match ray_tests::ray_vs_aabb(ray, &aabb) {
            Some(t) if t <= max_dist => {}
            _ => continue,
        };

        // Narrow phase
        let result = match collider {
            Collider::Sphere { radius } => {
                ray_tests::ray_vs_sphere(ray, *pos, *radius)
            }
            Collider::Box { half_extents } => {
                ray_tests::ray_vs_box(ray, *pos, *half_extents)
            }
            Collider::Capsule { radius, half_height } => {
                ray_tests::ray_vs_capsule(ray, *pos, *radius, *half_height)
            }
            Collider::ConvexHull(_) => {
                // Use AABB test as approximation for convex hull raycasting
                let aabb = collider.aabb(*pos);
                ray_tests::ray_vs_aabb(ray, &aabb).map(|t| (t, Vec3::Y))
            }
        };

        if let Some((t, normal)) = result {
            if t <= max_dist {
                hits.push(RayHit {
                    entity: *entity,
                    t,
                    point: ray.at(t),
                    normal,
                });
            }
        }
    }

    hits.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    hits
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

    // --- raycast_all tests ---

    #[test]
    fn test_raycast_all_multiple_hits() {
        let mut world = World::new();

        let near = world.spawn();
        world.add(near, Transform::from_position(Vec3::new(3.0, 0.0, 0.0)));
        world.add(near, Collider::Sphere { radius: 0.5 });

        let mid = world.spawn();
        world.add(mid, Transform::from_position(Vec3::new(6.0, 0.0, 0.0)));
        world.add(mid, Collider::Sphere { radius: 0.5 });

        let far = world.spawn();
        world.add(far, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        world.add(far, Collider::Sphere { radius: 0.5 });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hits = raycast_all(&world, &ray, 100.0);

        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].entity, near);
        assert_eq!(hits[1].entity, mid);
        assert_eq!(hits[2].entity, far);
        assert!(hits[0].t < hits[1].t);
        assert!(hits[1].t < hits[2].t);
    }

    #[test]
    fn test_raycast_all_empty() {
        let world = World::new();
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hits = raycast_all(&world, &ray, 100.0);
        assert!(hits.is_empty());
    }

    #[test]
    fn test_raycast_all_max_dist() {
        let mut world = World::new();

        let near = world.spawn();
        world.add(near, Transform::from_position(Vec3::new(3.0, 0.0, 0.0)));
        world.add(near, Collider::Sphere { radius: 0.5 });

        let far = world.spawn();
        world.add(far, Transform::from_position(Vec3::new(50.0, 0.0, 0.0)));
        world.add(far, Collider::Sphere { radius: 0.5 });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hits = raycast_all(&world, &ray, 10.0);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity, near);
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

    #[test]
    fn raycast_all_with_nan_position_does_not_panic() {
        // A Transform with NaN position propagates NaN into the collider AABB,
        // which can produce NaN ray-hit `t`. The sort must not panic on NaN.
        let mut world = World::new();
        let nan = f32::NAN;

        let good = world.spawn();
        world.add(good, Transform::from_position(Vec3::new(3.0, 0.0, 0.0)));
        world.add(good, Collider::Sphere { radius: 0.5 });

        let bad = world.spawn();
        world.add(bad, Transform::from_position(Vec3::new(nan, 0.0, 0.0)));
        world.add(bad, Collider::Sphere { radius: 0.5 });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        // Should not panic.
        let _ = raycast_all(&world, &ray, 100.0);
    }
}
