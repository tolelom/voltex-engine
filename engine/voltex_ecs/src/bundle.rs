//! Component bundles — spawn an entity and attach a whole set of components in
//! one call, instead of `spawn()` followed by N `add()`s. This is the Bevy
//! `Bundle` / Unity-prefab ergonomic: the most common authoring operation
//! (make an entity that *is* something) reads as one statement.
//!
//! ```ignore
//! use voltex_ecs::{World, Transform, Tag};
//!
//! let mut world = World::new();
//! let player = world.spawn_bundle((
//!     Transform::from_position(Vec3::new(0.0, 1.0, 0.0)),
//!     Tag("player".into()),
//! ));
//! // later, add more to an existing entity:
//! world.insert_bundle(player, (Health(100), Velocity::ZERO));
//! ```
//!
//! A [`Bundle`] is any tuple of `'static` components (1..=12 elements). The
//! elements are added in order, so a later element overwrites an earlier one of
//! the same type (same rule as repeated [`World::add`]).

use crate::entity::Entity;
use crate::world::World;

/// A set of components that can be attached to an entity in one operation.
/// Implemented for tuples of up to 12 `'static` components.
pub trait Bundle {
    /// Attach every component in this bundle to `entity` in `world`.
    fn insert(self, world: &mut World, entity: Entity);
}

macro_rules! impl_bundle_for_tuple {
    ($($name:ident),+) => {
        impl<$($name: 'static),+> Bundle for ($($name,)+) {
            #[allow(non_snake_case)]
            fn insert(self, world: &mut World, entity: Entity) {
                let ($($name,)+) = self;
                $( world.add(entity, $name); )+
            }
        }
    };
}

impl_bundle_for_tuple!(A);
impl_bundle_for_tuple!(A, B);
impl_bundle_for_tuple!(A, B, C);
impl_bundle_for_tuple!(A, B, C, D);
impl_bundle_for_tuple!(A, B, C, D, E);
impl_bundle_for_tuple!(A, B, C, D, E, F);
impl_bundle_for_tuple!(A, B, C, D, E, F, G);
impl_bundle_for_tuple!(A, B, C, D, E, F, G, H);
impl_bundle_for_tuple!(A, B, C, D, E, F, G, H, I);
impl_bundle_for_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_bundle_for_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_bundle_for_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

impl World {
    /// Allocate a fresh entity and attach every component in `bundle`, returning
    /// the new [`Entity`]. Equivalent to `spawn()` + one `add()` per component.
    pub fn spawn_bundle<B: Bundle>(&mut self, bundle: B) -> Entity {
        let entity = self.spawn();
        bundle.insert(self, entity);
        entity
    }

    /// Attach every component in `bundle` to an existing `entity` (overwriting
    /// any of the same types).
    pub fn insert_bundle<B: Bundle>(&mut self, entity: Entity, bundle: B) {
        bundle.insert(self, entity);
    }

    /// Spawn a bundle as a child of `parent`: like [`spawn_bundle`] but also
    /// links the new entity into `parent`'s hierarchy via
    /// [`add_child`](crate::add_child) (so `despawn_recursive(parent)` removes
    /// it, and `WorldTransform` propagation treats it as a child). Returns the
    /// new child entity.
    ///
    /// [`spawn_bundle`]: World::spawn_bundle
    pub fn spawn_child_bundle<B: Bundle>(&mut self, parent: Entity, bundle: B) -> Entity {
        let child = self.spawn_bundle(bundle);
        crate::hierarchy::add_child(self, parent, child);
        child
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Health(u32);
    #[derive(Debug, PartialEq)]
    struct Name(&'static str);
    #[derive(Debug, PartialEq)]
    struct Speed(f32);
    #[derive(Debug, PartialEq)]
    struct Level(u8);

    #[test]
    fn spawn_bundle_attaches_every_component() {
        let mut world = World::new();
        let e = world.spawn_bundle((Health(100), Name("hero")));
        assert!(world.is_alive(e));
        assert_eq!(world.get::<Health>(e), Some(&Health(100)));
        assert_eq!(world.get::<Name>(e), Some(&Name("hero")));
    }

    #[test]
    fn spawn_bundle_single_element_tuple() {
        let mut world = World::new();
        let e = world.spawn_bundle((Health(7),));
        assert_eq!(world.get::<Health>(e), Some(&Health(7)));
    }

    #[test]
    fn spawn_bundle_increments_entity_count() {
        let mut world = World::new();
        assert_eq!(world.entity_count(), 0);
        world.spawn_bundle((Health(1), Name("a")));
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn insert_bundle_adds_to_existing_entity() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Health(50));
        world.insert_bundle(e, (Name("npc"), Speed(3.5)));
        assert_eq!(world.get::<Health>(e), Some(&Health(50)));
        assert_eq!(world.get::<Name>(e), Some(&Name("npc")));
        assert_eq!(world.get::<Speed>(e), Some(&Speed(3.5)));
    }

    #[test]
    fn later_element_overwrites_same_type() {
        let mut world = World::new();
        // Two Health values in one bundle: the last one wins, matching repeated add().
        let e = world.spawn_bundle((Health(1), Name("x"), Health(2)));
        assert_eq!(world.get::<Health>(e), Some(&Health(2)));
    }

    #[test]
    fn spawn_child_bundle_links_into_parent_hierarchy() {
        use crate::hierarchy::{Children, Parent};

        let mut world = World::new();
        let parent = world.spawn_bundle((Health(100),));
        let child = world.spawn_child_bundle(parent, (Name("turret"), Speed(0.0)));

        // Child has its own components…
        assert_eq!(world.get::<Name>(child), Some(&Name("turret")));
        // …and is linked to the parent both ways.
        assert_eq!(world.get::<Parent>(child).map(|p| p.0), Some(parent));
        let kids = &world.get::<Children>(parent).expect("parent has Children").0;
        assert!(kids.contains(&child));
    }

    #[test]
    fn despawn_recursive_removes_a_spawn_child_bundle_child() {
        let mut world = World::new();
        let parent = world.spawn_bundle((Health(1),));
        let child = world.spawn_child_bundle(parent, (Name("attached"),));
        assert!(world.is_alive(child));

        crate::hierarchy::despawn_recursive(&mut world, parent);
        assert!(!world.is_alive(parent));
        assert!(!world.is_alive(child), "despawning the parent must take the child");
    }

    #[test]
    fn large_bundle_attaches_all_four() {
        let mut world = World::new();
        let e = world.spawn_bundle((Health(10), Name("boss"), Speed(1.0), Level(9)));
        assert_eq!(world.get::<Health>(e), Some(&Health(10)));
        assert_eq!(world.get::<Name>(e), Some(&Name("boss")));
        assert_eq!(world.get::<Speed>(e), Some(&Speed(1.0)));
        assert_eq!(world.get::<Level>(e), Some(&Level(9)));
    }
}
