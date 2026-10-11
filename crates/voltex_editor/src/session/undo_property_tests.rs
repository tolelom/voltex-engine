//! Randomized undo/redo property test: a long deterministic sequence of
//! editor commands, then undo all the way back and redo all the way forward,
//! checking the world matches the recorded state at every step.

use super::*;
use voltex_ecs::hierarchy::{Children, Parent};
use voltex_ecs::Tag;
use voltex_math::{Quat, Vec3};

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        self.next() as usize % n.max(1)
    }
    fn f(&mut self) -> f32 {
        (self.next() % 2001) as f32 / 100.0 - 10.0
    }
}

/// Order-independent world state: per entity (id, gen) its transform bits,
/// tag and parent. Undo restores despawned entities at their original
/// (id, gen), so identities are comparable across the walk back.
type Snapshot = Vec<((u32, u32), Option<[u32; 10]>, Option<String>, Option<(u32, u32)>)>;

fn snapshot(world: &World) -> Snapshot {
    let mut out: Snapshot = world
        .live_entities()
        .map(|e| {
            let t = world.get::<Transform>(e).map(|t| {
                let r = t.rotation;
                [
                    t.position.x, t.position.y, t.position.z,
                    r.x, r.y, r.z, r.w,
                    t.scale.x, t.scale.y, t.scale.z,
                ]
                .map(f32::to_bits)
            });
            let tag = world.get::<Tag>(e).map(|t| t.0.clone());
            let parent = world.get::<Parent>(e).map(|p| (p.0.id, p.0.generation));
            ((e.id, e.generation), t, tag, parent)
        })
        .collect();
    out.sort_by_key(|s| s.0);
    out
}

/// Identity-free view of a snapshot: an entity revived under a new handle
/// (undo of a despawn whose slot was reused) is still "the same" entity, so
/// compare each entity's transform + tag + its parent's transform + tag.
type Look = (Option<[u32; 10]>, Option<String>);
type Shape = (Option<[u32; 10]>, Option<String>, Option<Look>);

fn structure(s: &Snapshot) -> Vec<Shape> {
    let by_id: std::collections::HashMap<(u32, u32), Look> =
        s.iter().map(|(id, t, tag, _)| (*id, (*t, tag.clone()))).collect();
    let mut v: Vec<Shape> = s
        .iter()
        .map(|(_, t, tag, p)| (*t, tag.clone(), p.and_then(|pid| by_id.get(&pid).cloned())))
        .collect();
    v.sort();
    v
}

fn random_command(rng: &mut XorShift, world: &World) -> EditorCommand {
    let live: Vec<Entity> = world.live_entities().collect();
    let pick = |rng: &mut XorShift| live.get(rng.below(live.len())).copied();
    match (rng.below(6), pick(rng), pick(rng)) {
        (0, _, _) | (_, None, _) => EditorCommand::Spawn,
        (1, Some(e), _) => EditorCommand::Despawn { entity: e },
        (2, Some(e), _) => EditorCommand::SetTransform {
            entity: e,
            position: Vec3::new(rng.f(), rng.f(), rng.f()),
            rotation: Quat::from_axis_angle(Vec3::Y, rng.f()),
            scale: Vec3::new(1.0, 1.0 + rng.f().abs(), 1.0),
        },
        (3, Some(e), _) => EditorCommand::SetTag { entity: e, tag: format!("t{}", rng.below(5)) },
        (4, Some(p), Some(c)) => EditorCommand::AddChild { parent: p, child: c },
        (_, Some(p), Some(c)) => EditorCommand::RemoveChild { parent: p, child: c },
        (_, Some(e), None) => EditorCommand::Despawn { entity: e },
    }
}

#[test]
fn undo_despawn_after_slot_was_reused_and_released_restores_the_entity() {
    // delete A -> create B (reuses A's slot) -> undo -> undo: undoing B's
    // spawn despawns the slot again, so A's (id, gen) can never be revived
    // in place. A must still come back (as a fresh entity), not vanish.
    let scene = std::env::temp_dir()
        .join(format!("voltex_undo_reuse_{}.vscn", std::process::id()))
        .to_string_lossy()
        .into_owned();
    let mut session = EditorSession::new(scene.clone());
    let a = session.dispatch(EditorCommand::Spawn).unwrap();
    session
        .dispatch(EditorCommand::SetTag { entity: a, tag: "A".into() })
        .unwrap();
    session.dispatch(EditorCommand::Despawn { entity: a }).unwrap();
    let b = session.dispatch(EditorCommand::Spawn).unwrap();
    assert_eq!(b.id, a.id, "precondition: B reuses A's slot");

    assert!(session.undo()); // B gone
    assert!(session.undo()); // A back
    let tags: Vec<String> = session.world.query::<Tag>().map(|(_, t)| t.0.clone()).collect();
    assert_eq!(tags, vec!["A".to_string()], "A must be restored");

    // The chain keeps working on the restored entity: redo deletes it again,
    // undo brings it back again.
    assert!(session.redo());
    assert_eq!(session.world.query::<Tag>().count(), 0);
    assert!(session.undo());
    assert_eq!(session.world.query::<Tag>().count(), 1);

    let _ = std::fs::remove_file(&scene);
    let _ = std::fs::remove_file(format!("{scene}.log"));
}

#[test]
fn undo_despawn_restores_hierarchy_links_both_ways() {
    let scene = std::env::temp_dir()
        .join(format!("voltex_undo_hier_{}.vscn", std::process::id()))
        .to_string_lossy()
        .into_owned();
    let mut session = EditorSession::new(scene.clone());
    let p = session.dispatch(EditorCommand::Spawn).unwrap();
    let c = session.dispatch(EditorCommand::Spawn).unwrap();
    session.dispatch(EditorCommand::AddChild { parent: p, child: c }).unwrap();

    // Despawning the child must not leave it listed under the parent, and
    // undo must re-attach it.
    session.dispatch(EditorCommand::Despawn { entity: c }).unwrap();
    assert!(session.world.get::<Children>(p).is_none_or(|ch| ch.0.is_empty()), "no dangling child");
    assert!(session.undo());
    assert_eq!(session.world.get::<Parent>(c).map(|x| x.0), Some(p));
    assert_eq!(session.world.get::<Children>(p).map(|x| x.0.clone()), Some(vec![c]));

    // Despawning the parent must not leave the child pointing at it, and undo
    // must re-attach the child under it.
    session.dispatch(EditorCommand::Despawn { entity: p }).unwrap();
    assert!(session.world.get::<Parent>(c).is_none(), "no dangling parent");
    assert!(session.undo());
    assert_eq!(session.world.get::<Parent>(c).map(|x| x.0), Some(p));
    assert_eq!(session.world.get::<Children>(p).map(|x| x.0.clone()), Some(vec![c]));

    let _ = std::fs::remove_file(&scene);
    let _ = std::fs::remove_file(format!("{scene}.log"));
}

#[test]
fn undo_reparent_returns_child_to_previous_parent_and_same_parent_is_noop() {
    let scene = std::env::temp_dir()
        .join(format!("voltex_undo_reparent_{}.vscn", std::process::id()))
        .to_string_lossy()
        .into_owned();
    let mut session = EditorSession::new(scene.clone());
    let p1 = session.dispatch(EditorCommand::Spawn).unwrap();
    let p2 = session.dispatch(EditorCommand::Spawn).unwrap();
    let c = session.dispatch(EditorCommand::Spawn).unwrap();
    session.dispatch(EditorCommand::AddChild { parent: p1, child: c }).unwrap();

    // Re-attaching under the same parent, then undoing it, keeps the link.
    session.dispatch(EditorCommand::AddChild { parent: p1, child: c }).unwrap();
    assert!(session.undo());
    assert_eq!(session.world.get::<Parent>(c).map(|x| x.0), Some(p1));

    // Re-parent to p2, undo: back under p1 (not orphaned).
    session.dispatch(EditorCommand::AddChild { parent: p2, child: c }).unwrap();
    assert!(session.undo());
    assert_eq!(session.world.get::<Parent>(c).map(|x| x.0), Some(p1));
    assert_eq!(session.world.get::<Children>(p1).map(|x| x.0.clone()), Some(vec![c]));
    assert!(session.world.get::<Children>(p2).is_none_or(|x| x.0.is_empty()));

    let _ = std::fs::remove_file(&scene);
    let _ = std::fs::remove_file(format!("{scene}.log"));
}

#[test]
fn random_command_sequences_undo_and_redo_exactly() {
    for seed in [0x1234_5678u32, 0xDEAD_BEEF, 7] {
        let scene = std::env::temp_dir()
            .join(format!("voltex_undo_prop_{seed:x}_{}.vscn", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let mut session = EditorSession::new(scene.clone());
        let mut rng = XorShift(seed);
        let mut history = vec![snapshot(&session.world)];

        for step in 0..400 {
            let before = snapshot(&session.world);
            let depth_before = session.undo_depth();
            let cmd = random_command(&mut rng, &session.world);
            let label = format!("{cmd:?}");
            match session.dispatch(cmd) {
                // Documented no-ops (AddChild that already holds, RemoveChild
                // of a non-child) succeed without an undo entry — allowed only
                // if they really changed nothing.
                Ok(_) if session.undo_depth() == depth_before => assert_eq!(
                    snapshot(&session.world),
                    before,
                    "seed {seed:x} step {step}: {label} changed the world without an undo entry"
                ),
                Ok(_) => history.push(snapshot(&session.world)),
                Err(_) => assert_eq!(
                    snapshot(&session.world),
                    before,
                    "seed {seed:x} step {step}: failed {label} changed the world"
                ),
            }
        }

        let undoable = session.undo_depth();
        assert_eq!(undoable, history.len() - 1, "seed {seed:x}: every success is undoable");
        for k in (0..undoable).rev() {
            assert!(session.undo(), "seed {seed:x}: undo #{k} refused");
            assert_eq!(
                structure(&snapshot(&session.world)),
                structure(&history[k]),
                "seed {seed:x}: state after undo back to #{k}"
            );
        }
        assert!(!session.undo(), "nothing left to undo");

        while session.redo() {}
        assert_eq!(
            structure(&snapshot(&session.world)),
            structure(history.last().unwrap()),
            "seed {seed:x}: redo-all reaches the final state"
        );

        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{scene}.log"));
    }
}
