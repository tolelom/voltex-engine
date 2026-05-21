//! Computed world-space transform component + top-down propagation.

use voltex_math::Mat4;
use crate::{Entity, World, Transform};
use crate::hierarchy::{Parent, Children};

/// World-space affine transform — typically populated by
/// [`propagate_transforms`].
#[derive(Debug, Clone, Copy)]
pub struct WorldTransform(
    /// 4x4 world matrix.
    pub Mat4,
);

impl WorldTransform {
    /// Identity world transform.
    pub fn identity() -> Self { Self(Mat4::IDENTITY) }
}

/// Walks the hierarchy top-down and writes [`WorldTransform`] for every
/// entity that has a [`Transform`].
pub fn propagate_transforms(world: &mut World) {
    // Collect roots: entities with Transform but no Parent
    let roots: Vec<Entity> = world.query::<Transform>()
        .filter(|(e, _)| world.get::<Parent>(*e).is_none())
        .map(|(e, _)| e)
        .collect();

    for root in roots {
        propagate_entity(world, root, Mat4::IDENTITY);
    }
}

fn propagate_entity(world: &mut World, entity: Entity, parent_world: Mat4) {
    let local = match world.get::<Transform>(entity) {
        Some(t) => t.matrix(),
        None => return,
    };
    let world_matrix = parent_world * local;
    world.add(entity, WorldTransform(world_matrix));

    // Clone children to avoid borrow issues
    let children: Vec<Entity> = world.get::<Children>(entity)
        .map(|c| c.0.clone())
        .unwrap_or_default();

    for child in children {
        propagate_entity(world, child, world_matrix);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_math::{Vec3, Vec4};
    use crate::hierarchy::add_child;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn test_root_world_transform() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(5.0, 0.0, 0.0)));

        propagate_transforms(&mut world);

        let wt = world.get::<WorldTransform>(e).expect("WorldTransform should be set");
        // Transform the origin — should land at (5, 0, 0)
        let result = wt.0 * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(result.x, 5.0), "x: {}", result.x);
        assert!(approx_eq(result.y, 0.0), "y: {}", result.y);
        assert!(approx_eq(result.z, 0.0), "z: {}", result.z);
    }

    #[test]
    fn test_child_inherits_parent() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();

        world.add(parent, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        world.add(child, Transform::from_position(Vec3::new(0.0, 5.0, 0.0)));

        add_child(&mut world, parent, child);
        propagate_transforms(&mut world);

        let wt = world.get::<WorldTransform>(child).expect("child WorldTransform should be set");
        // Child origin in world space should be (10, 5, 0)
        let result = wt.0 * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(result.x, 10.0), "x: {}", result.x);
        assert!(approx_eq(result.y, 5.0),  "y: {}", result.y);
        assert!(approx_eq(result.z, 0.0),  "z: {}", result.z);
    }

    #[test]
    fn test_three_level_hierarchy() {
        let mut world = World::new();
        let root = world.spawn();
        let mid  = world.spawn();
        let leaf = world.spawn();

        world.add(root, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        world.add(mid,  Transform::from_position(Vec3::new(0.0, 2.0, 0.0)));
        world.add(leaf, Transform::from_position(Vec3::new(0.0, 0.0, 3.0)));

        add_child(&mut world, root, mid);
        add_child(&mut world, mid, leaf);
        propagate_transforms(&mut world);

        let wt = world.get::<WorldTransform>(leaf).expect("leaf WorldTransform should be set");
        // Leaf origin in world space should be (1, 2, 3)
        let result = wt.0 * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(result.x, 1.0), "x: {}", result.x);
        assert!(approx_eq(result.y, 2.0), "y: {}", result.y);
        assert!(approx_eq(result.z, 3.0), "z: {}", result.z);
    }

    #[test]
    fn test_parent_scale_affects_child() {
        let mut world = World::new();
        let parent = world.spawn();
        let child  = world.spawn();

        // Parent scaled 2x at origin
        world.add(parent, Transform::from_position_scale(
            Vec3::ZERO,
            Vec3::new(2.0, 2.0, 2.0),
        ));
        // Child at local (1, 0, 0)
        world.add(child, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));

        add_child(&mut world, parent, child);
        propagate_transforms(&mut world);

        let wt = world.get::<WorldTransform>(child).expect("child WorldTransform should be set");
        // Child origin in world space: parent scale 2x means (1,0,0) -> (2,0,0)
        let result = wt.0 * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(result.x, 2.0), "x: {}", result.x);
        assert!(approx_eq(result.y, 0.0), "y: {}", result.y);
        assert!(approx_eq(result.z, 0.0), "z: {}", result.z);
    }
}
