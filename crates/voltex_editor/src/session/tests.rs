use super::*;
use voltex_math::{Quat, Vec3};

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
fn dispatch_spawn_writes_scene_to_disk() {
    let scene = temp_path("voltex_session_spawn.vscn");
    cleanup(&scene);

    let mut session = EditorSession::new(scene.clone());
    session.dispatch(EditorCommand::Spawn).unwrap();

    let on_disk = std::fs::read_to_string(&scene).expect("scene file written");
    assert!(on_disk.contains("\"entities\""));
    cleanup(&scene);
}

#[test]
fn dispatch_appends_to_log() {
    let scene = temp_path("voltex_session_log.vscn");
    cleanup(&scene);

    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session
        .dispatch(EditorCommand::SetTransform {
            entity: e,
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(1.0, 1.0, 1.0),
        })
        .unwrap();

    let log = format!("{}.log", scene);
    let log_text = std::fs::read_to_string(&log).expect("log written");
    let lines: Vec<&str> = log_text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("Spawn"));
    assert!(lines[1].contains("SetTransform"));
    cleanup(&scene);
}

#[test]
fn undo_spawn_removes_the_entity() {
    let scene = temp_path("voltex_undo_spawn.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    assert!(session.world.is_alive(e));
    assert_eq!(session.undo_depth(), 1);

    assert!(session.undo());
    assert!(!session.world.is_alive(e));
    assert_eq!(session.undo_depth(), 0);
    assert_eq!(session.redo_depth(), 1);
    cleanup(&scene);
}

#[test]
fn undo_set_transform_restores_previous_values() {
    let scene = temp_path("voltex_undo_xform.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    // Spawn writes default Transform (all zeros + scale=1).
    session
        .dispatch(EditorCommand::SetTransform {
            entity: e,
            position: Vec3::new(5.0, 6.0, 7.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(2.0, 2.0, 2.0),
        })
        .unwrap();
    assert_eq!(session.world.get::<Transform>(e).unwrap().position, Vec3::new(5.0, 6.0, 7.0));

    assert!(session.undo());
    let t = session.world.get::<Transform>(e).unwrap();
    assert_eq!(t.position, Vec3::ZERO);
    assert_eq!(t.scale, Vec3::new(1.0, 1.0, 1.0));
    cleanup(&scene);
}

#[test]
fn undo_set_tag_restores_or_removes() {
    let scene = temp_path("voltex_undo_tag.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    assert!(session.world.get::<Tag>(e).is_none());

    session
        .dispatch(EditorCommand::SetTag { entity: e, tag: "Hero".into() })
        .unwrap();
    assert_eq!(session.world.get::<Tag>(e).unwrap().0, "Hero");

    // Undo SetTag → no tag again
    assert!(session.undo());
    assert!(session.world.get::<Tag>(e).is_none());

    // Redo restores
    assert!(session.redo());
    assert_eq!(session.world.get::<Tag>(e).unwrap().0, "Hero");
    cleanup(&scene);
}

#[test]
fn redo_after_new_dispatch_is_cleared() {
    let scene = temp_path("voltex_redo_cleared.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session
        .dispatch(EditorCommand::SetTag { entity: e, tag: "A".into() })
        .unwrap();
    session.undo();
    assert_eq!(session.redo_depth(), 1);

    // New action clears the redo stack
    session
        .dispatch(EditorCommand::SetTag { entity: e, tag: "B".into() })
        .unwrap();
    assert_eq!(session.redo_depth(), 0);
    cleanup(&scene);
}

// (E3 obsoletes the prior `despawn_clears_undo_history_v1` test — see
// `despawn_no_longer_clears_undo_history` below for the new behaviour.)

#[test]
fn undo_add_child_detaches() {
    use voltex_ecs::hierarchy::{Children, Parent};
    let scene = temp_path("voltex_undo_addchild.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let p = session.dispatch(EditorCommand::Spawn).unwrap();
    let c = session.dispatch(EditorCommand::Spawn).unwrap();
    session
        .dispatch(EditorCommand::AddChild { parent: p, child: c })
        .unwrap();
    assert_eq!(session.world.get::<Parent>(c).map(|x| x.0), Some(p));

    assert!(session.undo());
    assert!(session.world.get::<Parent>(c).is_none());
    assert!(session.world.get::<Children>(p).map(|x| x.0.is_empty()).unwrap_or(true));

    assert!(session.redo());
    assert_eq!(session.world.get::<Parent>(c).map(|x| x.0), Some(p));
    cleanup(&scene);
}

#[test]
fn undo_remove_child_reattaches() {
    use voltex_ecs::hierarchy::Parent;
    let scene = temp_path("voltex_undo_rmchild.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let p = session.dispatch(EditorCommand::Spawn).unwrap();
    let c = session.dispatch(EditorCommand::Spawn).unwrap();
    // Manually attach (not via command — we want to test RemoveChild's undo path
    // without conflating with the AddChild undo case above).
    voltex_ecs::hierarchy::add_child(&mut session.world, p, c);

    session
        .dispatch(EditorCommand::RemoveChild { parent: p, child: c })
        .unwrap();
    assert!(session.world.get::<Parent>(c).is_none());

    assert!(session.undo());
    assert_eq!(session.world.get::<Parent>(c).map(|x| x.0), Some(p));
    cleanup(&scene);
}

#[test]
fn remove_child_when_not_attached_is_not_recorded() {
    let scene = temp_path("voltex_rmchild_noop.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let p = session.dispatch(EditorCommand::Spawn).unwrap();
    let c = session.dispatch(EditorCommand::Spawn).unwrap();
    let depth_before = session.undo_depth();

    // Not currently attached → RemoveChild is a no-op and should not push history.
    session
        .dispatch(EditorCommand::RemoveChild { parent: p, child: c })
        .unwrap();
    assert_eq!(session.undo_depth(), depth_before);
    cleanup(&scene);
}

use voltex_ecs::{Bounds, MaterialHandle, MeshHandle};
use voltex_asset::Handle;
use voltex_math::AABB;

#[test]
fn set_mode_edit_to_play_stores_snapshot_and_changes_mode() {
    let scene = temp_path("voltex_e2_e2p.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let _ = session.dispatch(EditorCommand::Spawn).unwrap();

    assert_eq!(session.mode, EditorMode::Edit);
    session.set_mode(EditorMode::Play);
    assert_eq!(session.mode, EditorMode::Play);
    assert!(session.play_snapshot.is_some(),
        "snapshot must be present after entering Play");
    cleanup(&scene);
}

#[test]
fn set_mode_play_to_edit_restores_world_state() {
    let scene = temp_path("voltex_e2_restore.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session.dispatch(EditorCommand::SetTransform {
        entity: e,
        position: Vec3::new(10.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::new(1.0, 1.0, 1.0),
    }).unwrap();

    session.set_mode(EditorMode::Play);
    // Simulate physics moving the cube while in Play mode.
    if let Some(t) = session.world.get_mut::<Transform>(e) {
        t.position = Vec3::new(10.0, -5.0, 0.0);
    }

    session.set_mode(EditorMode::Edit);
    assert_eq!(session.mode, EditorMode::Edit);

    // The restored World should hold a new entity (entity allocator may
    // assign a fresh id), so look it up via Transform query.
    let positions: Vec<Vec3> = session.world
        .query::<Transform>()
        .map(|(_, t)| t.position)
        .collect();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0], Vec3::new(10.0, 0.0, 0.0),
        "Transform must restore to the pre-Play value, not the mutated one");
    cleanup(&scene);
}

#[test]
fn set_mode_same_mode_is_noop() {
    let scene = temp_path("voltex_e2_noop.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    session.set_mode(EditorMode::Edit);
    assert!(session.play_snapshot.is_none());
    assert_eq!(session.mode, EditorMode::Edit);
    cleanup(&scene);
}

#[test]
fn play_to_edit_drops_components_added_during_play() {
    let scene = temp_path("voltex_e2_drop.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();

    session.set_mode(EditorMode::Play);

    // Health is not in ComponentRegistry → snapshot won't preserve it.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Health(i32);
    session.world.add(e, Health(42));
    assert!(session.world.get::<Health>(e).is_some());

    session.set_mode(EditorMode::Edit);

    let any_health = session.world
        .query::<Transform>()
        .any(|(ent, _)| session.world.get::<Health>(ent).is_some());
    assert!(!any_health, "non-registered component must be dropped on Stop");
    cleanup(&scene);
}

#[test]
fn entering_play_clears_undo_redo_history() {
    let scene = temp_path("voltex_e2_clear_in.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session.dispatch(EditorCommand::SetTag { entity: e, tag: "X".into() }).unwrap();
    assert!(session.undo_depth() >= 2);

    session.set_mode(EditorMode::Play);
    assert_eq!(session.undo_depth(), 0);
    assert_eq!(session.redo_depth(), 0);
    cleanup(&scene);
}

#[test]
fn exiting_play_clears_undo_redo_history() {
    let scene = temp_path("voltex_e2_clear_out.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    session.dispatch(EditorCommand::Spawn).unwrap();
    session.set_mode(EditorMode::Play);
    session.set_mode(EditorMode::Edit);
    assert_eq!(session.undo_depth(), 0);
    assert_eq!(session.redo_depth(), 0);
    cleanup(&scene);
}

#[test]
fn snapshot_preserves_hierarchy() {
    use voltex_ecs::hierarchy::Parent;
    let scene = temp_path("voltex_e2_hier.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let p = session.dispatch(EditorCommand::Spawn).unwrap();
    let c = session.dispatch(EditorCommand::Spawn).unwrap();
    session.dispatch(EditorCommand::AddChild { parent: p, child: c }).unwrap();
    assert_eq!(session.world.get::<Parent>(c).map(|x| x.0), Some(p));

    session.set_mode(EditorMode::Play);
    session.set_mode(EditorMode::Edit);

    let parent_edges: Vec<(Entity, Entity)> = session.world
        .query::<Parent>()
        .map(|(child, parent)| (child, parent.0))
        .collect();
    assert_eq!(parent_edges.len(), 1, "exactly one parent edge after restore");
    cleanup(&scene);
}

#[test]
fn snapshot_preserves_render_handles() {
    let scene = temp_path("voltex_e2_handles.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session.world.add(e, MeshHandle(Handle::from_raw_parts(7, 1)));
    session.world.add(e, MaterialHandle(Handle::from_raw_parts(9, 2)));
    session.world.add(e, Bounds(AABB::new(
        Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0),
    )));

    session.set_mode(EditorMode::Play);
    session.set_mode(EditorMode::Edit);

    let restored = session.world.query::<Transform>().next().expect("entity").0;
    let mh = session.world.get::<MeshHandle>(restored).expect("MeshHandle");
    let math = session.world.get::<MaterialHandle>(restored).expect("MaterialHandle");
    let bounds = session.world.get::<Bounds>(restored).expect("Bounds");
    assert_eq!(mh.0.id(), 7);
    assert_eq!(mh.0.generation(), 1);
    assert_eq!(math.0.id(), 9);
    assert_eq!(math.0.generation(), 2);
    assert!((bounds.0.min.x - (-1.0)).abs() < 1e-6);
    cleanup(&scene);
}

use crate::lifecycle::LifecycleEvent;
use std::sync::{Arc, Mutex};

#[test]
fn dispatch_in_play_mode_returns_mode_mismatch() {
    let scene = temp_path("voltex_e2_dispatch.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let _ = session.dispatch(EditorCommand::Spawn).unwrap();
    session.set_mode(EditorMode::Play);

    let result = session.dispatch(EditorCommand::Spawn);
    assert!(matches!(
        result,
        Err(EditorError::ModeMismatch { current: EditorMode::Play })
    ));
    cleanup(&scene);
}

#[test]
fn play_start_callback_invoked_with_world() {
    let scene = temp_path("voltex_e2_cb_start.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();

    let log = Arc::new(Mutex::new(Vec::new()));
    let log_cb = log.clone();
    session.add_lifecycle(move |event, world| {
        let has_entity = world.is_alive(e);
        log_cb.lock().unwrap().push((event, has_entity));
    });

    session.set_mode(EditorMode::Play);
    let entries = log.lock().unwrap().clone();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, LifecycleEvent::PlayStart);
    assert!(entries[0].1, "PlayStart callback must see the live world");
    cleanup(&scene);
}

#[test]
fn play_stop_callback_invoked_before_clear() {
    let scene = temp_path("voltex_e2_cb_stop.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();

    let log = Arc::new(Mutex::new(Vec::new()));
    let log_cb = log.clone();
    session.add_lifecycle(move |event, world| {
        log_cb.lock().unwrap().push((event, world.is_alive(e)));
    });

    session.set_mode(EditorMode::Play);
    session.set_mode(EditorMode::Edit);

    let entries = log.lock().unwrap().clone();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].0, LifecycleEvent::PlayStop);
    assert!(entries[1].1, "PlayStop must run before World::clear");
    cleanup(&scene);
}

#[test]
fn lifecycles_called_in_registration_order() {
    let scene = temp_path("voltex_e2_order.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());

    let order = Arc::new(Mutex::new(Vec::<u32>::new()));
    for id in [1u32, 2, 3] {
        let order_cb = order.clone();
        session.add_lifecycle(move |_event, _world| {
            order_cb.lock().unwrap().push(id);
        });
    }
    session.set_mode(EditorMode::Play);
    let captured = order.lock().unwrap().clone();
    assert_eq!(captured, vec![1, 2, 3]);
    cleanup(&scene);
}

#[test]
fn multi_step_undo_redo_walks_both_ways() {
    let scene = temp_path("voltex_undo_multi.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session
        .dispatch(EditorCommand::SetTransform {
            entity: e,
            position: Vec3::new(1.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(1.0, 1.0, 1.0),
        })
        .unwrap();
    session
        .dispatch(EditorCommand::SetTransform {
            entity: e,
            position: Vec3::new(2.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(1.0, 1.0, 1.0),
        })
        .unwrap();
    assert_eq!(session.world.get::<Transform>(e).unwrap().position.x, 2.0);

    session.undo();
    assert_eq!(session.world.get::<Transform>(e).unwrap().position.x, 1.0);
    session.undo();
    assert_eq!(session.world.get::<Transform>(e).unwrap().position.x, 0.0);
    session.redo();
    assert_eq!(session.world.get::<Transform>(e).unwrap().position.x, 1.0);
    session.redo();
    assert_eq!(session.world.get::<Transform>(e).unwrap().position.x, 2.0);
    cleanup(&scene);
}


#[test]
fn redo_spawn_then_undo_cleans_up_the_new_entity() {
    // Regression: Spawn -> Undo -> Redo -> Undo used to leak the
    // entity created by Redo because the stored DespawnSpawned
    // inverse referenced the original (now dead) entity. Redo
    // must patch the inverse to point at the freshly-allocated
    // entity slot before pushing back onto undo_stack.
    let scene = temp_path("voltex_redo_spawn.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());

    // 1. Initial spawn.
    let e1 = session.dispatch(EditorCommand::Spawn).unwrap();
    assert!(session.world.is_alive(e1));
    assert_eq!(session.world.entity_count(), 1);

    // 2. Undo.
    assert!(session.undo());
    assert!(!session.world.is_alive(e1));
    assert_eq!(session.world.entity_count(), 0);

    // 3. Redo — allocates a fresh slot (same id, bumped generation).
    assert!(session.redo());
    assert_eq!(session.world.entity_count(), 1);
    // Whichever entity is now live, it is not e1.
    assert!(!session.world.is_alive(e1));

    // 4. Undo again — pre-fix this silent-no-op'd because the inverse
    //    targeted e1. The new entity leaked. Now the inverse is
    //    patched at redo time, so this undo cleans up properly.
    assert!(session.undo());
    assert_eq!(session.world.entity_count(), 0);
    cleanup(&scene);
}

