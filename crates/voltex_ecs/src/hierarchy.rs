//! Entity hierarchy components + helpers (`Parent`, `Children`, `add_child`,
//! `remove_child`, `despawn_recursive`, `roots`).

use crate::entity::Entity;
use crate::world::World;
use crate::transform::Transform;

/// Component pointing at the entity's parent in the hierarchy.
#[derive(Debug, Clone, Copy)]
pub struct Parent(
    /// Parent entity.
    pub Entity,
);

/// Component holding the entity's immediate children.
#[derive(Debug, Clone)]
pub struct Children(
    /// Immediate children, in insertion order.
    pub Vec<Entity>,
);

/// Set `child`'s Parent to `parent` and register `child` in `parent`'s Children list.
/// Does nothing if `child` is already in the Children list (no duplicates).
pub fn add_child(world: &mut World, parent: Entity, child: Entity) {
    // Set the Parent component on the child
    world.add(child, Parent(parent));

    // Check whether parent already has a Children component
    if world.get::<Children>(parent).is_some() {
        let children = world.get_mut::<Children>(parent).unwrap();
        if !children.0.contains(&child) {
            children.0.push(child);
        }
    } else {
        world.add(parent, Children(vec![child]));
    }
}

/// Remove `child` from `parent`'s Children list and strip the Parent component from `child`.
pub fn remove_child(world: &mut World, parent: Entity, child: Entity) {
    // Remove the child from the parent's Children list
    if let Some(children) = world.get_mut::<Children>(parent) {
        children.0.retain(|&e| e != child);
    }

    // Remove the Parent component from the child
    world.remove::<Parent>(child);
}

/// Despawn `entity` and all of its descendants recursively.
/// Also removes `entity` from its own parent's Children list.
pub fn despawn_recursive(world: &mut World, entity: Entity) {
    // Collect children first to avoid borrow conflicts during recursion
    let children: Vec<Entity> = world
        .get::<Children>(entity)
        .map(|c| c.0.clone())
        .unwrap_or_default();

    // Recurse into each child
    for child in children {
        despawn_recursive(world, child);
    }

    // Remove this entity from its parent's Children list (if it has a parent)
    let parent_entity = world.get::<Parent>(entity).map(|p| p.0);
    if let Some(parent) = parent_entity {
        if let Some(siblings) = world.get_mut::<Children>(parent) {
            siblings.0.retain(|&e| e != entity);
        }
    }

    // Despawn the entity itself (this removes all its components too)
    world.despawn(entity);
}

/// Return all entities that have a Transform but no Parent — i.e. scene roots.
pub fn roots(world: &World) -> Vec<Entity> {
    world
        .query::<Transform>()
        .filter(|(entity, _)| world.get::<Parent>(*entity).is_none())
        .map(|(entity, _)| entity)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_child() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();

        add_child(&mut world, parent, child);

        // Parent component set on child
        let p = world.get::<Parent>(child).expect("child should have Parent");
        assert_eq!(p.0, parent);

        // Children component on parent contains child
        let c = world.get::<Children>(parent).expect("parent should have Children");
        assert!(c.0.contains(&child));
    }

    #[test]
    fn test_add_multiple_children() {
        let mut world = World::new();
        let parent = world.spawn();
        let child1 = world.spawn();
        let child2 = world.spawn();

        add_child(&mut world, parent, child1);
        add_child(&mut world, parent, child2);

        let c = world.get::<Children>(parent).expect("parent should have Children");
        assert_eq!(c.0.len(), 2);
        assert!(c.0.contains(&child1));
        assert!(c.0.contains(&child2));
    }

    #[test]
    fn test_remove_child() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();

        add_child(&mut world, parent, child);
        remove_child(&mut world, parent, child);

        // Child should no longer have a Parent
        assert!(world.get::<Parent>(child).is_none(), "child should have no Parent after removal");

        // Parent's Children list should be empty
        let c = world.get::<Children>(parent).expect("parent should still have Children component");
        assert!(c.0.is_empty(), "Children list should be empty after removal");
    }

    #[test]
    fn test_despawn_recursive() {
        let mut world = World::new();
        let root = world.spawn();
        let child = world.spawn();
        let grandchild = world.spawn();

        // Add transforms so they are proper scene nodes
        world.add(root, Transform::new());
        world.add(child, Transform::new());
        world.add(grandchild, Transform::new());

        add_child(&mut world, root, child);
        add_child(&mut world, child, grandchild);

        despawn_recursive(&mut world, root);

        assert!(!world.is_alive(root), "root should be despawned");
        assert!(!world.is_alive(child), "child should be despawned");
        assert!(!world.is_alive(grandchild), "grandchild should be despawned");
    }

    #[test]
    fn test_roots() {
        let mut world = World::new();
        let root1 = world.spawn();
        let root2 = world.spawn();
        let child = world.spawn();

        world.add(root1, Transform::new());
        world.add(root2, Transform::new());
        world.add(child, Transform::new());

        add_child(&mut world, root1, child);

        let r = roots(&world);
        assert_eq!(r.len(), 2, "should have exactly 2 roots");
        assert!(r.contains(&root1));
        assert!(r.contains(&root2));
        assert!(!r.contains(&child));
    }

    #[test]
    fn test_no_duplicate_child() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();

        add_child(&mut world, parent, child);
        add_child(&mut world, parent, child); // add same child again

        let c = world.get::<Children>(parent).expect("parent should have Children");
        assert_eq!(c.0.len(), 1, "Children should not contain duplicates");
    }
}
