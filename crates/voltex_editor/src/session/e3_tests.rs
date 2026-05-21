//! E3 tests — AddComponent/RemoveComponent + Despawn full undo. Split off
//! from session/tests.rs in v1.0.2 to keep both test files under the 600-
//! line guideline.

use super::*;
use voltex_math::Vec3;

fn temp_path(name: &str) -> String {
    std::env::temp_dir()
        .join(format!("{}_{}", name, std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn cleanup(scene: &str) {
    let _ = std::fs::remove_file(scene);
    let _ = std::fs::remove_file(format!("{}.log", scene));
}


#[test]
fn add_component_undo_removes_it() {
    let scene = temp_path("voltex_e3_add_undo.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();

    // Initial Tag-less state.
    assert!(session.world.get::<voltex_ecs::scene::Tag>(e).is_none());

    let tag_bytes = b"Hero".to_vec();
    session
        .dispatch(EditorCommand::AddComponent {
            entity: e,
            name: "tag".into(),
            data: tag_bytes,
        })
        .unwrap();
    assert_eq!(session.world.get::<voltex_ecs::scene::Tag>(e).unwrap().0, "Hero");

    assert!(session.undo());
    assert!(session.world.get::<voltex_ecs::scene::Tag>(e).is_none());
    cleanup(&scene);
}

#[test]
fn remove_component_undo_restores_prior_bytes() {
    let scene = temp_path("voltex_e3_remove_undo.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session.dispatch(EditorCommand::SetTag { entity: e, tag: "Original".into() }).unwrap();

    session
        .dispatch(EditorCommand::RemoveComponent {
            entity: e,
            name: "tag".into(),
        })
        .unwrap();
    assert!(session.world.get::<voltex_ecs::scene::Tag>(e).is_none());

    assert!(session.undo());
    assert_eq!(session.world.get::<voltex_ecs::scene::Tag>(e).unwrap().0, "Original");
    cleanup(&scene);
}

#[test]
fn despawn_undo_restores_entity_and_components() {
    let scene = temp_path("voltex_e3_despawn_undo.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());

    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session
        .dispatch(EditorCommand::SetTransform {
            entity: e,
            position: Vec3::new(5.0, 6.0, 7.0),
            rotation: Vec3::ZERO,
            scale: Vec3::new(2.0, 2.0, 2.0),
        })
        .unwrap();
    session.dispatch(EditorCommand::SetTag { entity: e, tag: "Hero".into() }).unwrap();

    // Despawn it.
    session.dispatch(EditorCommand::Despawn { entity: e }).unwrap();
    assert!(!session.world.is_alive(e));

    // Undo despawn — entity comes back with the same handle.
    assert!(session.undo());
    assert!(session.world.is_alive(e), "entity must be alive again at original (id, gen)");
    let t = session.world.get::<Transform>(e).expect("Transform restored");
    assert_eq!(t.position, Vec3::new(5.0, 6.0, 7.0));
    assert_eq!(t.scale, Vec3::new(2.0, 2.0, 2.0));
    let tag = session.world.get::<voltex_ecs::scene::Tag>(e).expect("Tag restored");
    assert_eq!(tag.0, "Hero");
    cleanup(&scene);
}

#[test]
fn despawn_no_longer_clears_undo_history() {
    // Pre-E3 behaviour: Despawn cleared all undo history.
    // Post-E3: Despawn is itself undoable + earlier history survives.
    let scene = temp_path("voltex_e3_despawn_history.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());

    let a = session.dispatch(EditorCommand::Spawn).unwrap();
    let b = session.dispatch(EditorCommand::Spawn).unwrap();
    assert_eq!(session.undo_depth(), 2);

    session.dispatch(EditorCommand::Despawn { entity: b }).unwrap();
    // History: Spawn(a), Spawn(b), Despawn(b) — 3 undoable actions.
    assert_eq!(session.undo_depth(), 3);

    // Undo despawn → Spawn(b) re-lives.
    assert!(session.undo());
    assert!(session.world.is_alive(b));
    assert!(session.world.is_alive(a));
    assert_eq!(session.undo_depth(), 2);
    cleanup(&scene);
}

#[test]
fn despawn_undo_silently_fails_when_slot_reused() {
    // Despawn, then spawn (steals the slot), then undo despawn — restore
    // can't recover the original handle. Expect silent skip (undo returns
    // true because the stack popped, but world is unchanged for the
    // original entity).
    let scene = temp_path("voltex_e3_despawn_reused.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());

    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session.dispatch(EditorCommand::Despawn { entity: e }).unwrap();
    // World gives us a new entity in the same slot.
    let reused = session.dispatch(EditorCommand::Spawn).unwrap();
    assert_eq!(reused.id, e.id);
    assert_ne!(reused.generation, e.generation);

    // The Despawn entry is *not* on top — Spawn(reused) is. Undo Spawn first.
    assert!(session.undo());
    assert!(!session.world.is_alive(reused));

    // Now top of undo stack is Despawn(e). Undoing it should silently fail
    // because entry.generation has advanced beyond e.generation + 1.
    let depth_before = session.undo_depth();
    assert!(session.undo()); // pop succeeds
    assert_eq!(session.undo_depth(), depth_before - 1);
    // e never gets restored — generation moved past it.
    assert!(!session.world.is_alive(e));
    cleanup(&scene);
}
