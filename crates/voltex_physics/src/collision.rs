use voltex_ecs::{World, Entity};
use voltex_ecs::Transform;
use voltex_math::Vec3;

use crate::collider::Collider;
use crate::contact::ContactPoint;
use crate::bvh::BvhTree;
use crate::narrow;

pub fn detect_collisions(world: &World) -> Vec<ContactPoint> {
    // 1. Gather entities with Transform + Collider
    let pairs_data: Vec<(Entity, Vec3, Collider)> = world
        .query2::<Transform, Collider>()
        .into_iter()
        .map(|(e, t, c)| (e, t.position, *c))
        .collect();

    if pairs_data.len() < 2 {
        return Vec::new();
    }

    // 2. Build AABBs
    let entries: Vec<(Entity, voltex_math::AABB)> = pairs_data
        .iter()
        .map(|(e, pos, col)| (*e, col.aabb(*pos)))
        .collect();

    // 3. Broad phase
    let bvh = BvhTree::build(&entries);
    let broad_pairs = bvh.query_pairs();

    // 4. Narrow phase
    let mut contacts = Vec::new();

    let lookup = |entity: Entity| -> Option<(Vec3, Collider)> {
        pairs_data.iter().find(|(e, _, _)| *e == entity).map(|(_, p, c)| (*p, *c))
    };

    for (ea, eb) in broad_pairs {
        let (pos_a, col_a) = match lookup(ea) { Some(v) => v, None => continue };
        let (pos_b, col_b) = match lookup(eb) { Some(v) => v, None => continue };

        let result = match (&col_a, &col_b) {
            (Collider::Sphere { radius: ra }, Collider::Sphere { radius: rb }) => {
                narrow::sphere_vs_sphere(pos_a, *ra, pos_b, *rb)
            }
            (Collider::Sphere { radius }, Collider::Box { half_extents }) => {
                narrow::sphere_vs_box(pos_a, *radius, pos_b, *half_extents)
            }
            (Collider::Box { half_extents }, Collider::Sphere { radius }) => {
                narrow::sphere_vs_box(pos_b, *radius, pos_a, *half_extents)
                    .map(|(n, d, pa, pb)| (-n, d, pb, pa))
            }
            (Collider::Box { half_extents: ha }, Collider::Box { half_extents: hb }) => {
                narrow::box_vs_box(pos_a, *ha, pos_b, *hb)
            }
        };

        if let Some((normal, depth, point_on_a, point_on_b)) = result {
            contacts.push(ContactPoint {
                entity_a: ea,
                entity_b: eb,
                normal,
                depth,
                point_on_a,
                point_on_b,
            });
        }
    }

    contacts
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
