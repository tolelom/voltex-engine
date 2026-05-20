//! EditorSession owns the editor's mutable state: the ECS world, the component
//! registry, the currently selected entity, the path of the active scene file,
//! and the append-only command log. Every command flows through `dispatch`,
//! which mutates the world, appends a jsonl log line, and (for non-Load commands)
//! re-saves the active scene file so that a watcher / AI / human reading from
//! disk always sees the latest state.
//!
//! On top of dispatch, the session maintains an undo/redo history of *inverse*
//! commands so user/AI mistakes can be rolled back without re-loading the scene.

use crate::command::{apply_command, EditorCommand};
use crate::error::EditorError;
use std::fs::OpenOptions;
use std::io::Write;
use voltex_ecs::scene::Tag;
use voltex_ecs::{ComponentRegistry, Entity, Transform, World};
use voltex_math::Vec3;

/// Editor mode: Edit allows mutation, Play freezes the user-facing dispatch
/// and lets systems (physics, scripts) drive the World instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Edit,
    Play,
}

/// The inverse of a dispatched command. The undo stack stores one of these per
/// undoable command. Commands that don't have a tractable inverse (Despawn,
/// SaveScene, LoadScene) clear the undo/redo history when they run.
#[derive(Debug, Clone)]
enum Inverse {
    /// Undo Spawn: despawn the entity that was just created.
    DespawnSpawned(Entity),
    /// Undo SetTransform: write back the previous Transform values.
    SetTransform {
        entity: Entity,
        position: Vec3,
        rotation: Vec3,
        scale: Vec3,
    },
    /// Undo SetTag: restore the previous Tag (or remove if there was none).
    SetTag {
        entity: Entity,
        prev: Option<String>,
    },
    /// Undo AddChild: detach the child from the parent.
    RemoveChild { parent: Entity, child: Entity },
    /// Undo RemoveChild: re-attach the child to the parent.
    AddChild { parent: Entity, child: Entity },
}

pub struct EditorSession {
    pub world: World,
    pub registry: ComponentRegistry,
    pub selected: Option<Entity>,
    pub scene_path: String,
    pub log_path: String,
    pub mode: EditorMode,
    play_snapshot: Option<Vec<u8>>,
    lifecycles: Vec<crate::lifecycle::LifecycleCallback>,
    /// Stack of pairs: (forward command, inverse). Top is the most recent action.
    undo_stack: Vec<(EditorCommand, Inverse)>,
    /// Stack of pairs cleared by every new forward dispatch; populated by undo().
    redo_stack: Vec<(EditorCommand, Inverse)>,
}

impl EditorSession {
    /// Create a session bound to a scene file path. The log path defaults to
    /// `<scene_path>.log` so each scene has its own append-only history.
    pub fn new(scene_path: impl Into<String>) -> Self {
        let scene_path = scene_path.into();
        let log_path = format!("{}.log", scene_path);
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        Self {
            world: World::new(),
            registry,
            selected: None,
            scene_path,
            log_path,
            mode: EditorMode::Edit,
            play_snapshot: None,
            lifecycles: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Register a callback fired on every PlayStart/PlayStop transition.
    /// Callbacks run in registration order; a panicking callback aborts the
    /// remainder of the firing pass (panic-as-bug policy).
    pub fn add_lifecycle<F>(&mut self, callback: F)
    where
        F: FnMut(crate::lifecycle::LifecycleEvent, &mut World) + 'static,
    {
        self.lifecycles.push(Box::new(callback));
    }

    fn fire_lifecycle(&mut self, event: crate::lifecycle::LifecycleEvent) {
        let mut callbacks = std::mem::take(&mut self.lifecycles);
        for cb in &mut callbacks {
            cb(event, &mut self.world);
        }
        // Preserve any callbacks registered during the fire.
        let added_during_fire = std::mem::take(&mut self.lifecycles);
        callbacks.extend(added_during_fire);
        self.lifecycles = callbacks;
    }

    /// Transition between Edit and Play. Edit→Play captures a binary
    /// snapshot of the world; Play→Edit clears the world and deserialises
    /// the snapshot. Both transitions clear undo/redo history. Same-mode
    /// calls are a no-op.
    pub fn set_mode(&mut self, target: EditorMode) {
        if self.mode == target {
            return;
        }
        match (self.mode, target) {
            (EditorMode::Edit, EditorMode::Play) => {
                self.play_snapshot = Some(
                    voltex_ecs::serialize_scene_binary(&self.world, &self.registry),
                );
                self.mode = EditorMode::Play;
                self.fire_lifecycle(crate::lifecycle::LifecycleEvent::PlayStart);
                self.undo_stack.clear();
                self.redo_stack.clear();
            }
            (EditorMode::Play, EditorMode::Edit) => {
                self.fire_lifecycle(crate::lifecycle::LifecycleEvent::PlayStop);
                self.world.clear();
                if let Some(data) = self.play_snapshot.take() {
                    let _ = voltex_ecs::deserialize_scene_binary(
                        &mut self.world, &data, &self.registry,
                    );
                }
                self.mode = EditorMode::Edit;
                self.undo_stack.clear();
                self.redo_stack.clear();
            }
            _ => {}
        }
    }

    /// Execute a command, append a jsonl log entry, and (for state-mutating
    /// commands) re-save the scene to disk so the file is always current.
    ///
    /// In Play mode all dispatches return `EditorError::ModeMismatch` — only
    /// lifecycle callbacks may mutate the World while Play is active.
    pub fn dispatch(&mut self, cmd: EditorCommand) -> Result<Entity, EditorError> {
        if self.mode != EditorMode::Edit {
            return Err(EditorError::ModeMismatch { current: self.mode });
        }
        // Capture the inverse BEFORE applying, so we read the pre-state.
        // For Spawn this returns None — we'll patch it post-apply once we know
        // the entity id.
        let pre_inverse = self.capture_inverse(&cmd);

        let result = apply_command(&mut self.world, &self.registry, &cmd);
        self.append_log(&cmd, &result);

        if let Ok(applied_entity) = &result {
            match &cmd {
                EditorCommand::Spawn => {
                    self.undo_stack
                        .push((cmd.clone(), Inverse::DespawnSpawned(*applied_entity)));
                    self.redo_stack.clear();
                }
                EditorCommand::SetTransform { .. }
                | EditorCommand::SetTag { .. }
                | EditorCommand::AddChild { .. }
                | EditorCommand::RemoveChild { .. } => {
                    if let Some(inv) = pre_inverse {
                        self.undo_stack.push((cmd.clone(), inv));
                        self.redo_stack.clear();
                    }
                }
                EditorCommand::Despawn { .. }
                | EditorCommand::SaveScene { .. }
                | EditorCommand::LoadScene { .. } => {
                    // No tractable inverse in v1 — drop history rather than
                    // produce stale, broken undo paths.
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                }
            }
        }

        // Auto-sync scene file on every state mutation except LoadScene (which
        // already read from disk) and SaveScene (already wrote).
        let is_save_or_load = matches!(
            cmd,
            EditorCommand::SaveScene { .. } | EditorCommand::LoadScene { .. }
        );
        if result.is_ok() && !is_save_or_load {
            let json = voltex_ecs::serialize_scene_json(&self.world, &self.registry);
            std::fs::write(&self.scene_path, json)
                .map_err(|e| EditorError::Io(e.to_string()))?;
        }
        result
    }

    /// Number of actions on the undo stack (mostly for tests / UI affordances).
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of actions on the redo stack.
    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }

    /// Roll back the most recent undoable command. Returns true if anything
    /// was undone. The reverse becomes redo-able.
    pub fn undo(&mut self) -> bool {
        let Some((forward, inverse)) = self.undo_stack.pop() else {
            return false;
        };
        self.apply_inverse(&inverse);
        self.redo_stack.push((forward, inverse));
        self.persist_scene_silent();
        true
    }

    /// Re-apply the most recently undone command. Returns true if anything
    /// was redone.
    pub fn redo(&mut self) -> bool {
        let Some((forward, inverse)) = self.redo_stack.pop() else {
            return false;
        };
        let result = apply_command(&mut self.world, &self.registry, &forward);
        self.append_log(&forward, &result);
        if result.is_ok() {
            self.undo_stack.push((forward, inverse));
            self.persist_scene_silent();
        }
        result.is_ok()
    }

    fn capture_inverse(&self, cmd: &EditorCommand) -> Option<Inverse> {
        match cmd {
            EditorCommand::Spawn => {
                // We don't know the entity yet; resolved post-apply in dispatch.
                None
            }
            EditorCommand::SetTransform { entity, .. } => {
                let t = self.world.get::<Transform>(*entity)?;
                Some(Inverse::SetTransform {
                    entity: *entity,
                    position: t.position,
                    rotation: t.rotation,
                    scale: t.scale,
                })
            }
            EditorCommand::SetTag { entity, .. } => Some(Inverse::SetTag {
                entity: *entity,
                prev: self.world.get::<Tag>(*entity).map(|t| t.0.clone()),
            }),
            EditorCommand::AddChild { parent, child } => Some(Inverse::RemoveChild {
                parent: *parent,
                child: *child,
            }),
            EditorCommand::RemoveChild { parent, child } => {
                // Only undoable if the child is currently attached to this parent
                // (otherwise the RemoveChild is a no-op and we shouldn't record one).
                let is_attached = self
                    .world
                    .get::<voltex_ecs::hierarchy::Parent>(*child)
                    .map(|p| p.0 == *parent)
                    .unwrap_or(false);
                if is_attached {
                    Some(Inverse::AddChild {
                        parent: *parent,
                        child: *child,
                    })
                } else {
                    None
                }
            }
            EditorCommand::Despawn { .. }
            | EditorCommand::SaveScene { .. }
            | EditorCommand::LoadScene { .. } => None,
        }
    }

    fn apply_inverse(&mut self, inverse: &Inverse) {
        match inverse {
            Inverse::DespawnSpawned(entity) => {
                if self.world.is_alive(*entity) {
                    self.world.despawn(*entity);
                    if self.selected == Some(*entity) {
                        self.selected = None;
                    }
                }
            }
            Inverse::SetTransform {
                entity,
                position,
                rotation,
                scale,
            } => {
                if let Some(t) = self.world.get_mut::<Transform>(*entity) {
                    t.position = *position;
                    t.rotation = *rotation;
                    t.scale = *scale;
                }
            }
            Inverse::SetTag { entity, prev } => {
                self.world.remove::<Tag>(*entity);
                if let Some(tag) = prev {
                    self.world.add(*entity, Tag(tag.clone()));
                }
            }
            Inverse::AddChild { parent, child } => {
                if self.world.is_alive(*parent) && self.world.is_alive(*child) {
                    voltex_ecs::hierarchy::add_child(&mut self.world, *parent, *child);
                }
            }
            Inverse::RemoveChild { parent, child } => {
                if self.world.is_alive(*parent) && self.world.is_alive(*child) {
                    voltex_ecs::hierarchy::remove_child(&mut self.world, *parent, *child);
                }
            }
        }
    }

    fn persist_scene_silent(&self) {
        let json = voltex_ecs::serialize_scene_json(&self.world, &self.registry);
        let _ = std::fs::write(&self.scene_path, json);
    }

    fn append_log(&self, cmd: &EditorCommand, result: &Result<Entity, EditorError>) {
        let outcome = match result {
            Ok(e) => format!("{{\"ok\":{{\"id\":{},\"generation\":{}}}}}", e.id, e.generation),
            Err(e) => format!("{{\"err\":{:?}}}", format!("{}", e)),
        };
        let line = format!("{{\"cmd\":{:?},\"result\":{}}}\n", format!("{:?}", cmd), outcome);
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.log_path) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
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
                rotation: Vec3::ZERO,
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
                rotation: Vec3::ZERO,
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

    #[test]
    fn despawn_clears_undo_history_v1() {
        let scene = temp_path("voltex_despawn_clears.vscn");
        cleanup(&scene);
        let mut session = EditorSession::new(scene.clone());
        let e = session.dispatch(EditorCommand::Spawn).unwrap();
        session
            .dispatch(EditorCommand::SetTag { entity: e, tag: "Doomed".into() })
            .unwrap();
        assert_eq!(session.undo_depth(), 2);

        session.dispatch(EditorCommand::Despawn { entity: e }).unwrap();
        assert_eq!(session.undo_depth(), 0);
        assert_eq!(session.redo_depth(), 0);
        cleanup(&scene);
    }

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
            rotation: Vec3::ZERO,
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
                rotation: Vec3::ZERO,
                scale: Vec3::new(1.0, 1.0, 1.0),
            })
            .unwrap();
        session
            .dispatch(EditorCommand::SetTransform {
                entity: e,
                position: Vec3::new(2.0, 0.0, 0.0),
                rotation: Vec3::ZERO,
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
}
