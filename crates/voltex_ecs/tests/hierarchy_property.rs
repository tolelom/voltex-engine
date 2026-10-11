//! Randomized hierarchy invariants: after any sequence of `add_child`,
//! `remove_child`, `despawn_recursive` and plain `despawn` (including calls
//! on dead or unrelated entities), the links between *live* entities stay
//! consistent — a child's `Parent` and the parent's `Children` agree both
//! ways, `Children` has no duplicates, and no cycle exists.

use std::collections::HashSet;
use voltex_ecs::{add_child, despawn_recursive, remove_child, Children, Entity, Parent, Transform, World};

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

fn check(world: &World, ever: &[Entity], ctx: &str) {
    for &e in ever.iter().filter(|e| world.is_alive(**e)) {
        if let Some(p) = world.get::<Parent>(e).map(|p| p.0).filter(|p| world.is_alive(*p)) {
            let listed = world.get::<Children>(p).is_some_and(|c| c.0.contains(&e));
            assert!(listed, "{ctx}: {e:?} has Parent {p:?} but is not in its Children");
        }
        if let Some(children) = world.get::<Children>(e) {
            let unique: HashSet<_> = children.0.iter().collect();
            assert_eq!(unique.len(), children.0.len(), "{ctx}: duplicate in Children of {e:?}");
            for &c in children.0.iter().filter(|c| world.is_alive(**c)) {
                assert_eq!(world.get::<Parent>(c).map(|p| p.0), Some(e), "{ctx}: {c:?} listed under {e:?} but its Parent differs");
            }
        }
        // No cycle: walking up from e terminates without revisiting.
        let mut seen = HashSet::from([e]);
        let mut walk = e;
        while let Some(p) = world.get::<Parent>(walk).map(|p| p.0) {
            assert!(seen.insert(p), "{ctx}: cycle through {e:?}");
            walk = p;
        }
    }
}

#[test]
fn hierarchy_links_stay_consistent_under_random_edits() {
    for seed in [0x41E7_0001u32, 0xCAFE, 0x7777_0101] {
        let mut rng = XorShift(seed);
        let mut world = World::new();
        let mut ever: Vec<Entity> = Vec::new();
        for step in 0..3000 {
            let pick = |rng: &mut XorShift| ever.get(rng.next() as usize % ever.len().max(1)).copied();
            let ctx = format!("seed {seed:x} step {step}");
            match rng.next() % 10 {
                0..=2 => {
                    let e = world.spawn();
                    world.add(e, Transform::new());
                    ever.push(e);
                }
                3..=5 => {
                    if let (Some(p), Some(c)) = (pick(&mut rng), pick(&mut rng)) {
                        add_child(&mut world, p, c);
                    }
                }
                6 | 7 => {
                    if let (Some(p), Some(c)) = (pick(&mut rng), pick(&mut rng)) {
                        remove_child(&mut world, p, c);
                    }
                }
                8 => {
                    if let Some(e) = pick(&mut rng) {
                        despawn_recursive(&mut world, e);
                    }
                }
                _ => {
                    if let Some(e) = pick(&mut rng) {
                        world.despawn(e);
                    }
                }
            }
            check(&world, &ever, &ctx);
        }
    }
}

#[test]
fn adding_a_component_through_a_stale_handle_leaves_the_live_entity_alone() {
    // SparseSet storage is keyed by slot id: `world.add(stale, ..)` used to
    // overwrite the component of the live entity now occupying that slot
    // (and re-tag the slot with the stale generation, hiding it). Found by
    // the hierarchy property above via `add_child(parent, stale_child)`.
    let mut world = World::new();
    let old = world.spawn();
    world.despawn(old);
    let live = world.spawn();
    assert_eq!(old.id, live.id, "slot reused");
    world.add(live, Transform::from_position(voltex_math::Vec3::new(1.0, 2.0, 3.0)));

    world.add(old, Transform::new());
    let kept = world.get::<Transform>(live).map(|t| t.position.x);
    assert_eq!(kept, Some(1.0), "live entity's component clobbered by a stale handle");
    assert!(world.get::<Transform>(old).is_none());
}
