//! World-level collision detection — gathers `(Transform, Collider)`
//! entities, runs broadphase + narrowphase, returns the contact manifold.

use voltex_ecs::{World, Entity};
use voltex_ecs::Transform;
use voltex_math::{Vec3, AABB};

use crate::collider::Collider;
use crate::contact::{ContactPoint, Penetration};
use crate::bvh::BvhTree;
use crate::narrow;
use crate::gjk;

/// A collider placed in the world.
pub(crate) struct PlacedCollider {
    pub entity: Entity,
    /// World position (the entity's `Transform::position`; rotation is ignored).
    pub position: Vec3,
    pub collider: Collider,
}

/// Every entity with both a [`Transform`] and a [`Collider`].
pub(crate) fn placed_colliders(world: &World) -> Vec<PlacedCollider> {
    world
        .query2::<Transform, Collider>()
        .map(|(entity, transform, collider)| PlacedCollider {
            entity,
            position: transform.position,
            collider: collider.clone(),
        })
        .collect()
}

/// Runs broadphase + narrowphase on every `(Transform, Collider)` pair in the
/// world. Returns all generated [`ContactPoint`]s.
pub fn detect_collisions(world: &World) -> Vec<ContactPoint> {
    let bodies = placed_colliders(world);
    if bodies.len() < 2 {
        return Vec::new();
    }

    // Broad phase: BVH over world AABBs
    let entries: Vec<(Entity, AABB)> = bodies
        .iter()
        .map(|body| (body.entity, body.collider.aabb(body.position)))
        .collect();
    let broad_pairs = BvhTree::build(&entries).query_pairs();

    // Narrow phase
    let find = |entity: Entity| bodies.iter().find(|body| body.entity == entity);
    let mut contacts = Vec::new();
    for (entity_a, entity_b) in broad_pairs {
        let (Some(body_a), Some(body_b)) = (find(entity_a), find(entity_b)) else {
            continue;
        };
        if let Some(penetration) = penetration_between(body_a, body_b) {
            contacts.push(ContactPoint {
                entity_a,
                entity_b,
                normal: penetration.normal,
                depth: penetration.depth,
                point_on_a: penetration.point_on_a,
                point_on_b: penetration.point_on_b,
            });
        }
    }

    contacts
}

/// Picks the narrowphase test for the shape pair. Normal points A→B.
fn penetration_between(a: &PlacedCollider, b: &PlacedCollider) -> Option<Penetration> {
    let (pos_a, pos_b) = (a.position, b.position);
    match (&a.collider, &b.collider) {
        (Collider::Sphere { radius: radius_a }, Collider::Sphere { radius: radius_b }) => {
            narrow::sphere_vs_sphere(pos_a, *radius_a, pos_b, *radius_b)
        }
        (Collider::Sphere { radius }, Collider::Box { half_extents }) => {
            narrow::sphere_vs_box(pos_a, *radius, pos_b, *half_extents)
        }
        (Collider::Box { half_extents }, Collider::Sphere { radius }) => {
            narrow::sphere_vs_box(pos_b, *radius, pos_a, *half_extents).map(Penetration::flipped)
        }
        (Collider::Box { half_extents: half_a }, Collider::Box { half_extents: half_b }) => {
            narrow::box_vs_box(pos_a, *half_a, pos_b, *half_b)
        }
        // Any combination involving Capsule or ConvexHull uses GJK/EPA
        (Collider::Capsule { .. }, _) | (_, Collider::Capsule { .. })
        | (Collider::ConvexHull(_), _) | (_, Collider::ConvexHull(_)) => {
            gjk::gjk_epa(&a.collider, pos_a, &b.collider, pos_b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::World;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;
    use crate::Collider;

    #[test]
    fn test_no_colliders() {
        let world = World::new();
        let contacts = detect_collisions(&world);
        assert!(contacts.is_empty());
    }

    #[test]
    fn test_single_entity() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        world.add(e, Collider::Sphere { radius: 1.0 });
        let contacts = detect_collisions(&world);
        assert!(contacts.is_empty());
    }

    #[test]
    fn test_two_spheres_colliding() {
        let mut world = World::new();
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.5, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);
        assert!((contacts[0].depth - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_two_spheres_separated() {
        let mut world = World::new();
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });

        let contacts = detect_collisions(&world);
        assert!(contacts.is_empty());
    }

    #[test]
    fn test_sphere_vs_box_collision() {
        let mut world = World::new();
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.5, 0.0, 0.0)));
        world.add(b, Collider::Box { half_extents: Vec3::ONE });

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].depth > 0.0);
    }

    #[test]
    fn test_box_vs_box_collision() {
        let mut world = World::new();
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Box { half_extents: Vec3::ONE });

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.5, 0.0, 0.0)));
        world.add(b, Collider::Box { half_extents: Vec3::ONE });

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);
        assert!((contacts[0].depth - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_three_entities_mixed() {
        let mut world = World::new();

        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.5, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });

        let c = world.spawn();
        world.add(c, Transform::from_position(Vec3::new(100.0, 0.0, 0.0)));
        world.add(c, Collider::Box { half_extents: Vec3::ONE });

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);
    }
}
