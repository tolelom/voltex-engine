//! Inspector panel — view/edit components of the selected entity.
//!
//! Every mutation flows through `EditorCommand` via `EditorSession::dispatch`,
//! so slider drags, tag edits, and Despawn all appear in the jsonl log and the
//! scene file stays in sync automatically.

use crate::command::EditorCommand;
use crate::session::EditorSession;
use crate::ui_context::UiContext;
use voltex_ecs::scene::Tag;
use voltex_ecs::{Entity, Transform};
use voltex_math::{Quat, Vec3};

const EPSILON: f32 = 1e-4;

/// Component types surfaced by the Inspector's "Add Component" button row.
/// `name` must match the corresponding `Component::NAME` in voltex_ecs.
struct AddComponentSpec {
    name: &'static str,
}

const ADD_COMPONENT_BUTTONS: [AddComponentSpec; 6] = [
    AddComponentSpec { name: "transform" },
    AddComponentSpec { name: "tag" },
    AddComponentSpec { name: "mesh_handle" },
    AddComponentSpec { name: "material_handle" },
    AddComponentSpec { name: "bounds" },
    AddComponentSpec { name: "script" },
];

/// Persistent buffers for the inspector's text inputs and Euler rotation
/// proxy. The Inspector itself is stateless across frames except for these
/// fields (the source of truth is the World); the Euler buffer exists only
/// to keep rotation sliders smooth — round-tripping `Quat → Euler → Quat`
/// every frame would jitter and lose info near gimbal lock.
pub struct InspectorState {
    /// Current contents of the tag text input.
    pub tag: String,
    /// Cached Euler triple (YXZ, radians) for the currently selected entity.
    /// Re-extracted from `Transform.rotation` only when selection changes
    /// or the world's quaternion drifts away from `last_quat`.
    pub euler: Vec3,
    /// The quaternion `euler` was derived from. Used to detect external
    /// changes (e.g. gizmo, undo, physics in Play mode) that should refresh
    /// the proxy without nuking an in-progress slider drag.
    pub last_quat: Quat,
    /// The world-side tag value `tag` was last synced against (or last
    /// dispatched). Detects external changes (undo, "- tag" removal, script)
    /// that should refresh the input buffer — without it, one more keystroke
    /// after an undo re-dispatches the undone value (`last_quat` twin).
    pub last_tag: String,
    /// Entity whose state populated the buffers (used to reset on selection change).
    pub last_entity: Option<Entity>,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            tag: String::new(),
            euler: Vec3::ZERO,
            last_quat: Quat::IDENTITY,
            last_tag: String::new(),
            last_entity: None,
        }
    }
}

impl InspectorState {
    /// Refresh the buffers from the world if the selected entity changed
    /// since last frame, or if an external write moved the rotation
    /// quaternion away from what the Euler proxy currently represents.
    fn sync_from(&mut self, session: &EditorSession) {
        let Some(entity) = session.selected else {
            self.last_entity = None;
            self.tag.clear();
            self.last_tag.clear();
            return;
        };

        let entity_changed = self.last_entity != Some(entity);
        // Refresh the tag buffer when the selection changed OR the world-side
        // value moved away from what the buffer was synced against (undo,
        // "- tag" removal, external write) — the `last_quat` pattern.
        let world_tag = session
            .world
            .get::<Tag>(entity)
            .map(|t| t.0.clone())
            .unwrap_or_default();
        if entity_changed || world_tag != self.last_tag {
            self.tag = world_tag.clone();
            self.last_tag = world_tag;
        }
        if entity_changed {
            self.last_entity = Some(entity);
        }

        // Refresh Euler from quat when (a) selection just changed, or
        // (b) the world's quat no longer matches the one we derived from.
        if let Some(t) = session.world.get::<Transform>(entity) {
            let drifted = (t.rotation.x - self.last_quat.x).abs() > 1e-5
                || (t.rotation.y - self.last_quat.y).abs() > 1e-5
                || (t.rotation.z - self.last_quat.z).abs() > 1e-5
                || (t.rotation.w - self.last_quat.w).abs() > 1e-5;
            if entity_changed || drifted {
                self.euler = t.rotation.to_euler_yxz();
                self.last_quat = t.rotation;
            }
        }
    }
}

fn changed(a: f32, b: f32) -> bool {
    (a - b).abs() > EPSILON
}

/// One-line read-only value summary for a *built-in* component on `entity`,
/// or `None` for components with their own editing UI (`transform`, `tag`),
/// unknown / `register_raw` names (the editor cannot decode their bytes), and
/// absent components. Built-ins are read directly via `world.get::<T>()` —
/// no reflection infrastructure.
pub fn component_value_summary(
    session: &EditorSession,
    entity: Entity,
    name: &str,
) -> Option<String> {
    let world = &session.world;
    match name {
        "bounds" => world.get::<voltex_ecs::Bounds>(entity).map(|b| {
            format!(
                "min({:.2}, {:.2}, {:.2}) max({:.2}, {:.2}, {:.2})",
                b.0.min.x, b.0.min.y, b.0.min.z, b.0.max.x, b.0.max.y, b.0.max.z
            )
        }),
        "mesh_handle" => world
            .get::<voltex_ecs::MeshHandle>(entity)
            .map(|h| format!("id={} gen={}", h.0.id(), h.0.generation())),
        "material_handle" => world
            .get::<voltex_ecs::MaterialHandle>(entity)
            .map(|h| format!("id={} gen={}", h.0.id(), h.0.generation())),
        "script" => world.get::<voltex_ecs::ScriptComponent>(entity).map(|s| {
            if s.instance_id == voltex_ecs::ScriptComponent::UNBOUND {
                s.script_path.clone()
            } else {
                format!("{} #{}", s.script_path, s.instance_id)
            }
        }),
        // transform / tag have dedicated editing UI above the list.
        _ => None,
    }
}

/// 24-byte LE payload for an `AddComponent { name: "bounds" }` dispatch.
/// Must mirror `Bounds::serialize` (min.xyz then max.xyz) — layout drift is
/// guarded by a registry round-trip test.
pub fn bounds_bytes(min: Vec3, max: Vec3) -> Vec<u8> {
    let mut data = Vec::with_capacity(24);
    for v in [min.x, min.y, min.z, max.x, max.y, max.z] {
        data.extend_from_slice(&v.to_le_bytes());
    }
    data
}

/// Six sliders editing the entity's [`Bounds`](voltex_ecs::Bounds) in place.
/// A change dispatches `AddComponent` (deserialize overwrites — the E3
/// idempotence contract), so every edit is logged, persisted, and undoes to
/// the *prior value* (the session captures pre-bytes when the component
/// already exists). Per-tick dispatch during a drag matches the Transform
/// sliders' v1 behavior.
fn bounds_edit_form(ui: &mut UiContext, session: &mut EditorSession, entity: Entity) {
    let Some(b) = session.world.get::<voltex_ecs::Bounds>(entity) else {
        return;
    };
    let (min, max) = (b.0.min, b.0.max);
    let new_min = Vec3::new(
        ui.slider("Min X", min.x, -50.0, 50.0),
        ui.slider("Min Y", min.y, -50.0, 50.0),
        ui.slider("Min Z", min.z, -50.0, 50.0),
    );
    let new_max = Vec3::new(
        ui.slider("Max X", max.x, -50.0, 50.0),
        ui.slider("Max Y", max.y, -50.0, 50.0),
        ui.slider("Max Z", max.z, -50.0, 50.0),
    );
    let edited = changed(min.x, new_min.x)
        || changed(min.y, new_min.y)
        || changed(min.z, new_min.z)
        || changed(max.x, new_max.x)
        || changed(max.y, new_max.y)
        || changed(max.z, new_max.z);
    if edited {
        let _ = session.dispatch(EditorCommand::AddComponent {
            entity,
            name: "bounds".to_string(),
            data: bounds_bytes(new_min, new_max),
        });
    }
}

/// Names of registered components currently attached to `entity`, in
/// registry order. Drives the Inspector's "Components" list and the
/// filtering of its "Add Component" buttons; exposed so headless tests and
/// external tools can share the exact UI logic.
pub fn attached_component_names(session: &EditorSession, entity: Entity) -> Vec<String> {
    session
        .registry
        .entries()
        .iter()
        .filter(|e| (e.serialize)(&session.world, entity).is_some())
        .map(|e| e.name.clone())
        .collect()
}

/// Draw the inspector panel for the entity in `session.selected`. Any change
/// produced by a slider or text input is dispatched as an EditorCommand so it
/// is logged and persisted.
pub fn inspector_panel(
    ui: &mut UiContext,
    session: &mut EditorSession,
    state: &mut InspectorState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    state.sync_from(session);
    ui.begin_panel("Inspector", x, y, w, h);

    let Some(entity) = session.selected else {
        ui.text("(no entity selected)");
        ui.end_panel();
        return;
    };

    ui.text(&format!("Entity id={} gen={}", entity.id, entity.generation));

    // ── Transform (sliders, dispatched as SetTransform on change) ──
    if let Some(t) = session.world.get::<Transform>(entity) {
        let (px, py, pz) = (t.position.x, t.position.y, t.position.z);
        // Use the Euler buffer for rotation sliders — see InspectorState
        // docs for why we don't extract from the quat every frame.
        let (rx, ry, rz) = (state.euler.x, state.euler.y, state.euler.z);
        let (sx, sy, sz) = (t.scale.x, t.scale.y, t.scale.z);

        ui.text("-- Transform --");
        let new_px = ui.slider("Pos X", px, -50.0, 50.0);
        let new_py = ui.slider("Pos Y", py, -50.0, 50.0);
        let new_pz = ui.slider("Pos Z", pz, -50.0, 50.0);
        let new_rx = ui.slider("Rot X", rx, -3.15, 3.15);
        let new_ry = ui.slider("Rot Y", ry, -3.15, 3.15);
        let new_rz = ui.slider("Rot Z", rz, -3.15, 3.15);
        let new_sx = ui.slider("Scl X", sx, 0.01, 10.0);
        let new_sy = ui.slider("Scl Y", sy, 0.01, 10.0);
        let new_sz = ui.slider("Scl Z", sz, 0.01, 10.0);

        let moved = changed(px, new_px) || changed(py, new_py) || changed(pz, new_pz);
        let rotated = changed(rx, new_rx) || changed(ry, new_ry) || changed(rz, new_rz);
        let scaled = changed(sx, new_sx) || changed(sy, new_sy) || changed(sz, new_sz);
        if moved || rotated || scaled {
            let new_euler = Vec3::new(new_rx, new_ry, new_rz);
            let new_quat = Quat::from_euler_yxz(new_euler);
            // Update the proxy in lockstep so sync_from doesn't re-extract
            // immediately and undo the slider change.
            state.euler = new_euler;
            state.last_quat = new_quat;
            let _ = session.dispatch(EditorCommand::SetTransform {
                entity,
                position: Vec3::new(new_px, new_py, new_pz),
                rotation: new_quat,
                scale: Vec3::new(new_sx, new_sy, new_sz),
            });
        }
    }

    // ── Tag (text input, dispatched as SetTag on commit) ──
    ui.text("-- Tag --");
    let tag_input_y = ui.layout.cursor_y;
    let tag_input_x = x + ui.layout.padding;
    let tag_input_w = (w - ui.layout.padding * 2.0).max(50.0);
    if ui.text_input(0x1_5A_00_01, &mut state.tag, tag_input_x, tag_input_y, tag_input_w) {
        // 버퍼가 곧 world 값이 된다 — last_tag를 함께 갱신해 다음 프레임
        // sync가 방금 친 입력을 외부 변경으로 오인해 되돌리지 않게 한다.
        state.last_tag = state.tag.clone();
        let _ = session.dispatch(EditorCommand::SetTag {
            entity,
            tag: state.tag.clone(),
        });
    }
    ui.layout.advance_line();

    // ── Parent (read-only display) ──
    if let Some(parent) = session.world.get::<voltex_ecs::hierarchy::Parent>(entity) {
        let parent_text = format!("Parent: Entity({})", parent.0.id);
        ui.text(&parent_text);
    }

    // ── Components (attached, with remove buttons) ────────────────────
    // One `- name` button per registered component on this entity.
    // RemoveComponent is fully undoable (the session captures prior bytes).
    // `register_raw` entries have a no-op remove — the dispatch still
    // succeeds (same "absent → no-op Ok" contract as the command itself).
    let attached = attached_component_names(session, entity);
    ui.text("-- Components --");
    for name in &attached {
        // Resolve the read-only summary before the button: a remove click
        // mutates the world mid-loop and the value would be gone.
        let summary = component_value_summary(session, entity, name);
        let label = format!("- {}", name);
        if ui.button(&label) {
            let _ = session.dispatch(EditorCommand::RemoveComponent {
                entity,
                name: name.clone(),
            });
        }
        // Bounds gets an edit form instead of the read-only summary; other
        // built-ins keep the v1.0.92 one-liner.
        if name == "bounds" {
            bounds_edit_form(ui, session, entity);
        } else if let Some(summary) = summary {
            ui.text(&format!("  {}", summary));
        }
    }

    // ── Add Component buttons (E3) ────────────────────────────────────
    // One button per built-in registered component not already attached.
    // AddComponent dispatches with the component's default_bytes —
    // deserialize handles overwriting, so a stale click stays idempotent.
    ui.text("-- Add Component --");
    for spec in &ADD_COMPONENT_BUTTONS {
        if attached.iter().any(|n| n == spec.name) {
            continue;
        }
        let label = format!("+ {}", spec.name);
        if ui.button(&label) {
            if let Some(entry) = session.registry.find(spec.name) {
                let bytes = (entry.default_bytes)();
                let _ = session.dispatch(EditorCommand::AddComponent {
                    entity,
                    name: spec.name.to_string(),
                    data: bytes,
                });
            }
        }
    }

    if ui.button("Despawn") {
        let _ = session.dispatch(EditorCommand::Despawn { entity });
        session.selected = None;
    }

    ui.end_panel();
    let _ = h; // reserved for future scroll/clip
}

#[cfg(test)]
mod tests;
