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
    /// Edit mode — commands accepted, systems are paused.
    Edit,
    /// Play mode — commands rejected (read-only surface); systems run.
    Play,
}

/// The inverse of a dispatched command. The undo stack stores one of these per
/// undoable command. Only SaveScene/LoadScene clear the history (no tractable
/// inverse). Despawn became undoable in E3 via the `RestoreEntity` variant.
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
    /// Undo AddComponent: remove the just-added component by name (E3).
    RemoveComponentByName { entity: Entity, name: String },
    /// Undo RemoveComponent: re-add with prior bytes (E3). `data = None`
    /// when the component was absent before — undo leaves it absent.
    AddComponentByBytes { entity: Entity, name: String, data: Option<Vec<u8>> },
    /// Undo Despawn: restore the entity slot at its original generation and
    /// re-attach every registered component captured pre-despawn (E3).
    RestoreEntity { entity: Entity, components: Vec<(String, Vec<u8>)> },
}

/// Owns the editor's mutable state — World, ComponentRegistry, selection,
/// Edit/Play mode, lifecycle callbacks, and the undo/redo + jsonl log
/// machinery. Every mutation flows through [`EditorSession::dispatch`].
///
/// External code typically does:
///
/// ```ignore
/// let mut session = EditorSession::new("scene.vscn");
/// let e = session.dispatch(EditorCommand::Spawn)?;
/// session.dispatch(EditorCommand::SetTag { entity: e, tag: "Hero".into() })?;
/// session.undo();
/// ```
pub struct EditorSession {
    /// The ECS world being edited. Library callers may read but should
    /// prefer [`EditorSession::dispatch`] for mutations so they show up in
    /// the log + undo stack.
    pub world: World,
    /// Component registration table — drives scene serialization and the
    /// AddComponent/RemoveComponent commands.
    pub registry: ComponentRegistry,
    /// The single source of truth for "currently focused" entity (Inspector,
    /// gizmo, viewport picking all read this).
    pub selected: Option<Entity>,
    /// Scene file path written on every successful Edit-mode dispatch.
    pub scene_path: String,
    /// jsonl log path — one line per dispatch with cmd + result.
    pub log_path: String,
    /// Edit (default) or Play. Dispatch is rejected while in Play mode.
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
                | EditorCommand::RemoveChild { .. }
                | EditorCommand::AddComponent { .. }
                | EditorCommand::RemoveComponent { .. }
                | EditorCommand::Despawn { .. } => {
                    if let Some(inv) = pre_inverse {
                        self.undo_stack.push((cmd.clone(), inv));
                        self.redo_stack.clear();
                    }
                }
                EditorCommand::SaveScene { .. } | EditorCommand::LoadScene { .. } => {
                    // Scene-level commands cross persistence boundaries —
                    // dropping the inverse history is the safe path.
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
            EditorCommand::AddComponent { entity, name, .. } => {
                // Inverse is RemoveComponent — we don't capture pre-bytes
                // because AddComponent semantics overwrite any prior value.
                Some(Inverse::RemoveComponentByName {
                    entity: *entity,
                    name: name.clone(),
                })
            }
            EditorCommand::RemoveComponent { entity, name } => {
                // Capture prior bytes so undo can re-attach the exact value.
                let prior = self
                    .registry
                    .find(name)
                    .and_then(|entry| (entry.serialize)(&self.world, *entity));
                Some(Inverse::AddComponentByBytes {
                    entity: *entity,
                    name: name.clone(),
                    data: prior,
                })
            }
            EditorCommand::Despawn { entity } => {
                // Snapshot every registered component on the entity before
                // it disappears. Unregistered components are silently lost —
                // same policy as Play→Stop in E2.
                let components: Vec<(String, Vec<u8>)> = self
                    .registry
                    .entries()
                    .iter()
                    .filter_map(|entry| {
                        (entry.serialize)(&self.world, *entity)
                            .map(|data| (entry.name.clone(), data))
                    })
                    .collect();
                Some(Inverse::RestoreEntity {
                    entity: *entity,
                    components,
                })
            }
            EditorCommand::SaveScene { .. } | EditorCommand::LoadScene { .. } => None,
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
            Inverse::RemoveComponentByName { entity, name } => {
                if let Some(entry) = self.registry.find(name) {
                    (entry.remove)(&mut self.world, *entity);
                }
            }
            Inverse::AddComponentByBytes { entity, name, data } => {
                if let (Some(entry), Some(bytes)) =
                    (self.registry.find(name), data.as_ref())
                {
                    let _ = (entry.deserialize)(&mut self.world, *entity, bytes);
                }
                // data == None means the component was absent before; undo
                // leaves it absent.
            }
            Inverse::RestoreEntity { entity, components } => {
                let restored = self
                    .world
                    .restore_at_generation(entity.id, entity.generation);
                if restored {
                    for (name, bytes) in components {
                        if let Some(entry) = self.registry.find(name) {
                            let _ = (entry.deserialize)(&mut self.world, *entity, bytes);
                        }
                    }
                }
                // If restore failed (slot was reused by another spawn that
                // hasn't yet been despawned, etc.) we silently no-op. The
                // undo() caller still pops the stack, so the UI's redo
                // affordance becomes available — visible "nothing happened"
                // is the intended behaviour for this edge case.
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
mod tests;

#[cfg(test)]
mod e3_tests;
