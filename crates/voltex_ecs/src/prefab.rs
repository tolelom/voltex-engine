//! Prefabs — named entity templates you register once and instantiate by name,
//! the way Unity prefabs / Godot `PackedScene`s work. Builds directly on
//! [`World::spawn_bundle`](crate::World::spawn_bundle): a prefab is just a
//! factory closure that spawns a configured entity.
//!
//! ```ignore
//! use voltex_ecs::{Prefabs, World, Transform, Tag};
//!
//! let mut prefabs = Prefabs::new();
//! prefabs.register("enemy", |world| {
//!     world.spawn_bundle((Transform::default(), Tag("enemy".into()), Health(30)))
//! });
//!
//! // instantiate many; configure per-instance afterwards (the Unity pattern):
//! let e = prefabs.spawn(&mut world, "enemy").unwrap();
//! world.get_mut::<Transform>(e).unwrap().position = spawn_point;
//! ```
//!
//! Factories take `&mut World` and return the new [`Entity`], so they can use
//! any spawn path (`spawn_bundle`, manual `add`, hierarchy) and reference
//! components from any crate — the registry itself is component-agnostic.

use std::collections::HashMap;

use crate::entity::Entity;
use crate::world::World;

/// A closure that spawns one instance of a prefab into the world.
type PrefabFactory = Box<dyn Fn(&mut World) -> Entity>;

/// A registry of named entity templates.
#[derive(Default)]
pub struct Prefabs {
    factories: HashMap<String, PrefabFactory>,
}

impl Prefabs {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) the prefab named `name` with a factory closure.
    /// Returns `&mut self` for chaining.
    pub fn register<F>(&mut self, name: &str, factory: F) -> &mut Self
    where
        F: Fn(&mut World) -> Entity + 'static,
    {
        self.factories.insert(name.to_string(), Box::new(factory));
        self
    }

    /// Instantiate the prefab named `name` into `world`, returning the new
    /// entity. `None` if no prefab with that name is registered.
    pub fn spawn(&self, world: &mut World, name: &str) -> Option<Entity> {
        let factory = self.factories.get(name)?;
        Some(factory(world))
    }

    /// Instantiate the prefab named `name` as a child of `parent` (links it via
    /// [`add_child`](crate::add_child)). `None` if the prefab is unregistered.
    pub fn spawn_child(&self, world: &mut World, name: &str, parent: Entity) -> Option<Entity> {
        let child = self.spawn(world, name)?;
        crate::hierarchy::add_child(world, parent, child);
        Some(child)
    }

    /// True if a prefab named `name` is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.factories.contains_key(name)
    }

    /// Number of registered prefabs.
    pub fn len(&self) -> usize {
        self.factories.len()
    }

    /// True if no prefabs are registered.
    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::Transform;
    use voltex_math::Vec3;

    #[derive(Debug, PartialEq)]
    struct Health(u32);

    fn enemy_registry() -> Prefabs {
        let mut prefabs = Prefabs::new();
        prefabs.register("enemy", |world| {
            world.spawn_bundle((Transform::default(), Health(30)))
        });
        prefabs
    }

    #[test]
    fn spawn_instantiates_the_template_components() {
        let prefabs = enemy_registry();
        let mut world = World::new();
        let e = prefabs.spawn(&mut world, "enemy").unwrap();
        assert!(world.is_alive(e));
        assert_eq!(world.get::<Health>(e), Some(&Health(30)));
        assert!(world.get::<Transform>(e).is_some());
    }

    #[test]
    fn spawn_unknown_prefab_returns_none() {
        let prefabs = enemy_registry();
        let mut world = World::new();
        assert_eq!(prefabs.spawn(&mut world, "boss"), None);
        assert_eq!(world.entity_count(), 0, "a missing prefab must not spawn anything");
    }

    #[test]
    fn spawn_twice_makes_two_distinct_entities() {
        let prefabs = enemy_registry();
        let mut world = World::new();
        let a = prefabs.spawn(&mut world, "enemy").unwrap();
        let b = prefabs.spawn(&mut world, "enemy").unwrap();
        assert_ne!(a, b);
        assert_eq!(world.entity_count(), 2);
    }

    #[test]
    fn per_instance_configuration_after_spawn() {
        let prefabs = enemy_registry();
        let mut world = World::new();
        let e = prefabs.spawn(&mut world, "enemy").unwrap();
        world.get_mut::<Transform>(e).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
        assert_eq!(world.get::<Transform>(e).unwrap().position, Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn spawn_child_links_instance_to_parent() {
        use crate::hierarchy::{Children, Parent};

        let prefabs = enemy_registry();
        let mut world = World::new();
        let parent = world.spawn();
        let child = prefabs.spawn_child(&mut world, "enemy", parent).unwrap();

        assert_eq!(world.get::<Health>(child), Some(&Health(30)));
        assert_eq!(world.get::<Parent>(child).map(|p| p.0), Some(parent));
        assert!(world.get::<Children>(parent).unwrap().0.contains(&child));
    }

    #[test]
    fn spawn_child_unknown_prefab_returns_none() {
        let prefabs = enemy_registry();
        let mut world = World::new();
        let parent = world.spawn();
        assert_eq!(prefabs.spawn_child(&mut world, "boss", parent), None);
    }

    #[test]
    fn contains_and_counts() {
        let prefabs = enemy_registry();
        assert!(prefabs.contains("enemy"));
        assert!(!prefabs.contains("boss"));
        assert_eq!(prefabs.len(), 1);
        assert!(!prefabs.is_empty());
    }

    #[test]
    fn register_replaces_existing_prefab() {
        let mut prefabs = enemy_registry();
        prefabs.register("enemy", |world| world.spawn_bundle((Health(999),)));
        let mut world = World::new();
        let e = prefabs.spawn(&mut world, "enemy").unwrap();
        assert_eq!(world.get::<Health>(e), Some(&Health(999)));
        assert_eq!(prefabs.len(), 1, "re-registering the same name must not add a second entry");
    }
}
