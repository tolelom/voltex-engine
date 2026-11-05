//! Entity hierarchy components + helpers (`Parent`, `Children`, `add_child`,
//! `remove_child`, `despawn_recursive`, `roots`).

use crate::entity::Entity;
use crate::transform::Transform;
use crate::world::World;

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
///
/// **Validation**:
/// - Self-parenting (`parent == child`) is rejected silently — no change is
///   made. Previously the call succeeded and produced an entity that was its
///   own ancestor, which made [`despawn_recursive`] infinite-recurse.
/// - If `child` is being re-parented (already has a `Parent` component
///   pointing at some other entity), the old parent's `Children` list is
///   cleaned up first.
/// - If `parent` is already an ancestor of `child`, the call is rejected
///   silently to prevent introducing a cycle into the hierarchy.
/// - Existing membership in `parent`'s `Children` is preserved without
///   duplication.
pub fn add_child(world: &mut World, parent: Entity, child: Entity) {
    // Self-parenting is degenerate; reject silently.
    if parent == child {
        return;
    }

    // Cycle check: if `parent` is currently a descendant of `child`, adding
    // child→parent edge would close a cycle. Walk up from `parent` via the
    // existing Parent chain; if we hit `child`, abort.
    let mut walk = parent;
    let mut steps = 0usize;
    // Conservative cap matches the cycle test below — a healthy hierarchy
    // does not need millions of ancestors. The cap also defends against
    // pre-existing cycles in malformed scenes.
    const MAX_ANCESTOR_WALK: usize = 1 << 20;
    while let Some(p) = world.get::<Parent>(walk) {
        if p.0 == child {
            return; // would create a cycle
        }
        walk = p.0;
        steps += 1;
        if steps > MAX_ANCESTOR_WALK {
            return; // pre-existing cycle or absurd depth; bail
        }
    }

    // Re-parent: if child has a Parent already, clean up the old parent's
    // Children list before overwriting.
    if let Some(old) = world.get::<Parent>(child).map(|p| p.0) {
        if old != parent {
            if let Some(old_children) = world.get_mut::<Children>(old) {
                old_children.0.retain(|&e| e != child);
            }
        }
    }

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

    // Only clear the child's Parent if it actually points at `parent` — a
    // mismatched call must not detach the child from its real parent (whose
    // Children would still list it).
    if world.get::<Parent>(child).is_some_and(|p| p.0 == parent) {
        world.remove::<Parent>(child);
    }
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

/// Maximum ancestor-walk depth tolerated by [`repair_hierarchy_cycles`]. Mirrors
/// `MAX_HIERARCHY_DEPTH` used by transform propagation so a hierarchy that
/// this pass leaves in place is one the propagator can also walk safely.
const MAX_PARENT_CHAIN_DEPTH: usize = 1024;

/// Scan every `Parent` edge for cycles or pathologically deep chains and
/// break them by removing the offending entity's `Parent` component.
///
/// `add_child` already refuses to introduce a cycle, but raw `world.add(e,
/// Parent(p))` writes — used by `ComponentRegistry`-driven scene
/// deserialization for arbitrary user components, by hand-edited save
/// files, or by adversarial input — can land cycles in the world without
/// the helper's guard ever running. Without this pass, downstream walkers
/// (`despawn_recursive`, transform propagation, editor selection) rely on
/// internal depth caps to avoid hangs but the world itself stays
/// corrupted.
///
/// The pass walks each entity's ancestor chain. Returns the list of
/// entities whose `Parent` was stripped — callers can log them as a
/// "scene was repaired" warning. After this call:
/// - No entity has a `Parent` component that participates in a cycle.
/// - No entity has a `Parent` chain deeper than [`MAX_PARENT_CHAIN_DEPTH`].
/// - The matching parent's `Children` list (if present) is also cleaned
///   so the bidirectional view stays consistent.
///
/// Call this from scene deserializers after all `add_child` writes, or
/// after any custom load path that bypasses [`add_child`].
pub fn repair_hierarchy_cycles(world: &mut World) -> Vec<Entity> {
    // Snapshot entities with a Parent. We may mutate Parent storage during
    // the walk, so we can't hold a borrow into it.
    let with_parent: Vec<Entity> = world.query::<Parent>().map(|(entity, _)| entity).collect();

    let mut repaired = Vec::new();
    let mut visited: std::collections::HashSet<Entity> = std::collections::HashSet::new();

    for start in with_parent {
        // If a previous iteration already stripped this entity's Parent
        // (because it was the cycle entry point of another walk), skip.
        if world.get::<Parent>(start).is_none() {
            continue;
        }

        visited.clear();
        visited.insert(start);

        let mut current = start;
        let mut steps = 0usize;
        let cycle_or_too_deep = loop {
            let next = match world.get::<Parent>(current) {
                Some(p) => p.0,
                None => break false, // reached a root — chain is clean
            };
            if !visited.insert(next) {
                // Revisited an entity → cycle.
                break true;
            }
            current = next;
            steps += 1;
            if steps > MAX_PARENT_CHAIN_DEPTH {
                // Chain too deep — treat as cycle / corruption.
                break true;
            }
        };

        if cycle_or_too_deep {
            // Break the cycle by stripping `start`'s Parent and removing
            // it from the (immediate) parent's Children list. This is the
            // minimal incision that restores a DAG — we don't try to be
            // clever about which edge is "wrong" because all edges in a
            // cycle are equally to blame.
            let parent_of_start = world.get::<Parent>(start).map(|p| p.0);
            world.remove::<Parent>(start);
            if let Some(p) = parent_of_start {
                if let Some(children) = world.get_mut::<Children>(p) {
                    children.0.retain(|&e| e != start);
                }
            }
            repaired.push(start);
        }
    }

    repaired
}

/// Old name of [`repair_hierarchy_cycles`].
#[deprecated(note = "use `repair_hierarchy_cycles`")]
pub fn validate_hierarchy(world: &mut World) -> Vec<Entity> {
    repair_hierarchy_cycles(world)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_child_with_the_wrong_parent_leaves_the_real_link_alone() {
        // Found by the editor undo property test: the child's Parent was
        // removed unconditionally, detaching it from its *real* parent while
        // that parent's Children still listed it.
        let mut world = World::new();
        let real = world.spawn();
        let other = world.spawn();
        let child = world.spawn();
        add_child(&mut world, real, child);

        remove_child(&mut world, other, child);

        assert_eq!(world.get::<Parent>(child).map(|p| p.0), Some(real));
        assert_eq!(world.get::<Children>(real).map(|c| c.0.clone()), Some(vec![child]));
    }

    #[test]
    fn test_add_child() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();

        add_child(&mut world, parent, child);

        // Parent component set on child
        let p = world
            .get::<Parent>(child)
            .expect("child should have Parent");
        assert_eq!(p.0, parent);

        // Children component on parent contains child
        let c = world
            .get::<Children>(parent)
            .expect("parent should have Children");
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

        let c = world
            .get::<Children>(parent)
            .expect("parent should have Children");
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
        assert!(
            world.get::<Parent>(child).is_none(),
            "child should have no Parent after removal"
        );

        // Parent's Children list should be empty
        let c = world
            .get::<Children>(parent)
            .expect("parent should still have Children component");
        assert!(
            c.0.is_empty(),
            "Children list should be empty after removal"
        );
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
        assert!(
            !world.is_alive(grandchild),
            "grandchild should be despawned"
        );
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

        let c = world
            .get::<Children>(parent)
            .expect("parent should have Children");
        assert_eq!(c.0.len(), 1, "Children should not contain duplicates");
    }

    #[test]
    fn add_child_rejects_self_parent() {
        // An entity claiming itself as its own parent would make
        // despawn_recursive infinite-recurse. add_child must silently
        // refuse the operation.
        let mut world = World::new();
        let e = world.spawn();
        add_child(&mut world, e, e);
        assert!(
            world.get::<Parent>(e).is_none(),
            "self-parent must not assign Parent"
        );
        assert!(
            world.get::<Children>(e).is_none(),
            "self-parent must not assign Children"
        );
    }

    #[test]
    fn add_child_rejects_simple_cycle() {
        // After A→B, attempting B→A would form a cycle.
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        add_child(&mut world, a, b);
        // This second add_child would create a cycle A↔B.
        add_child(&mut world, b, a);
        // A's parent must still be None (the cycle was refused).
        assert!(
            world.get::<Parent>(a).is_none(),
            "cycle-creating add must be refused"
        );
        // B's parent is still A.
        let bp = world.get::<Parent>(b).expect("b still has parent");
        assert_eq!(bp.0, a);
    }

    #[test]
    fn add_child_rejects_deeper_cycle() {
        // A→B→C, then attempting C→A would close a 3-cycle.
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        add_child(&mut world, a, b);
        add_child(&mut world, b, c);
        add_child(&mut world, c, a); // would form A↔B↔C↔A
        assert!(
            world.get::<Parent>(a).is_none(),
            "deep cycle must be refused"
        );
    }

    #[test]
    fn add_child_reparents_cleanly() {
        // Move `child` from old_parent to new_parent and confirm the old
        // parent's Children list no longer references the child.
        let mut world = World::new();
        let old_parent = world.spawn();
        let new_parent = world.spawn();
        let child = world.spawn();
        add_child(&mut world, old_parent, child);
        add_child(&mut world, new_parent, child);

        // child now points at new_parent only.
        let p = world.get::<Parent>(child).expect("child has parent");
        assert_eq!(p.0, new_parent);

        // Old parent's Children list must no longer contain `child`.
        let old_children = world
            .get::<Children>(old_parent)
            .expect("old still has Children comp");
        assert!(
            !old_children.0.contains(&child),
            "stale entry in old parent's Children"
        );

        // New parent's Children contains child exactly once.
        let new_children = world
            .get::<Children>(new_parent)
            .expect("new has Children comp");
        assert_eq!(new_children.0, vec![child]);
    }

    #[test]
    fn despawn_recursive_terminates_on_clean_hierarchy() {
        // Sanity: with the cycle guard above, a chain that's deep but
        // acyclic still despawns. (Earlier hidden cycles would have stack-
        // overflowed this entire test crate.)
        let mut world = World::new();
        let mut prev = world.spawn();
        world.add(prev, Transform::new());
        let root = prev;
        for _ in 0..256 {
            let next = world.spawn();
            world.add(next, Transform::new());
            add_child(&mut world, prev, next);
            prev = next;
        }
        despawn_recursive(&mut world, root);
        assert!(!world.is_alive(root));
        assert!(!world.is_alive(prev));
    }

    #[test]
    fn repair_hierarchy_cycles_breaks_raw_two_cycle() {
        // Bypass add_child entirely: write the cyclic Parent components
        // directly the way a registry-driven deserializer would for a
        // user-registered Parent component. repair_hierarchy_cycles must repair.
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        // A→B (B's parent is A) and B→A (A's parent is B) — a 2-cycle.
        world.add(a, Parent(b));
        world.add(b, Parent(a));

        let repaired = repair_hierarchy_cycles(&mut world);
        // Exactly one entity's Parent gets stripped (whichever the walk
        // hits first); the other survives as a root.
        assert_eq!(repaired.len(), 1, "exactly one edge must be broken");
        let only_root_remains = world.get::<Parent>(a).is_none() ^ world.get::<Parent>(b).is_none();
        assert!(
            only_root_remains,
            "after repair exactly one of {{A,B}} is root"
        );
    }

    #[test]
    fn repair_hierarchy_cycles_keeps_clean_dag_intact() {
        let mut world = World::new();
        let root = world.spawn();
        let child = world.spawn();
        let grandchild = world.spawn();
        add_child(&mut world, root, child);
        add_child(&mut world, child, grandchild);

        let repaired = repair_hierarchy_cycles(&mut world);
        assert!(repaired.is_empty(), "no repair needed on a clean DAG");
        // Edges preserved.
        assert_eq!(world.get::<Parent>(child).unwrap().0, root);
        assert_eq!(world.get::<Parent>(grandchild).unwrap().0, child);
    }

    #[test]
    fn repair_hierarchy_cycles_breaks_three_cycle_and_keeps_unrelated_dag() {
        // A→B→C→A is a 3-cycle written raw. Plus an unrelated D→E chain.
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        let d = world.spawn();
        let e = world.spawn();
        world.add(a, Parent(c)); // a's parent = c
        world.add(b, Parent(a)); // b's parent = a
        world.add(c, Parent(b)); // c's parent = b  → cycle a→c→b→a
        add_child(&mut world, d, e); // clean DAG e→d (e's parent = d)

        let repaired = repair_hierarchy_cycles(&mut world);
        assert!(!repaired.is_empty(), "cycle in {{A,B,C}} must be repaired");
        // The unrelated DAG survives.
        assert_eq!(world.get::<Parent>(e).unwrap().0, d);
        // The cycle is broken (one node lost its Parent).
        let cycle_parents_present = [a, b, c]
            .iter()
            .filter(|&&n| world.get::<Parent>(n).is_some())
            .count();
        assert!(
            cycle_parents_present < 3,
            "at least one Parent in the cycle must be removed"
        );
    }

    #[test]
    fn repair_hierarchy_cycles_caps_pathologically_deep_chain() {
        // A linear chain longer than MAX_PARENT_CHAIN_DEPTH is treated as
        // corruption — we don't want propagate_transforms to ever see one.
        let mut world = World::new();
        let mut prev = world.spawn();
        let leaf = prev;
        for _ in 0..(MAX_PARENT_CHAIN_DEPTH + 50) {
            let next = world.spawn();
            // Raw Parent write — add_child would also walk and bail, but we
            // want to land the chain unguarded.
            world.add(prev, Parent(next));
            prev = next;
        }
        let repaired = repair_hierarchy_cycles(&mut world);
        // The repair strips Parent from `leaf` (the first start whose
        // walk runs over the cap) — at minimum that one is repaired.
        assert!(repaired.contains(&leaf), "deep-chain leaf must be repaired");
    }
}
