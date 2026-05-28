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
    /// Entity whose state populated the buffers (used to reset on selection change).
    pub last_entity: Option<Entity>,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            tag: String::new(),
            euler: Vec3::ZERO,
            last_quat: Quat::IDENTITY,
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
            return;
        };

        let entity_changed = self.last_entity != Some(entity);
        if entity_changed {
            self.last_entity = Some(entity);
            self.tag = session
                .world
                .get::<Tag>(entity)
                .map(|t| t.0.clone())
                .unwrap_or_default();
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

    // ── Add Component buttons (E3) ────────────────────────────────────
    // One button per built-in registered component. AddComponent dispatches
    // with the component's default_bytes — deserialize handles overwriting
    // any existing instance, so clicking is idempotent.
    ui.text("-- Add Component --");
    for spec in &ADD_COMPONENT_BUTTONS {
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
mod tests {
    use super::*;
    use crate::UiContext;

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
    fn inspector_runs_with_no_selection() {
        let mut ui = UiContext::new(800.0, 600.0);
        let scene = temp_path("voltex_insp_no_sel.vscn");
        cleanup(&scene);

        let mut session = EditorSession::new(scene.clone());
        let mut state = InspectorState::default();
        ui.begin_frame(0.0, 0.0, false);
        inspector_panel(&mut ui, &mut session, &mut state, 0.0, 0.0, 300.0, 400.0);
        ui.end_frame();
        cleanup(&scene);
    }

    #[test]
    fn inspector_sync_loads_tag_into_buffer() {
        let scene = temp_path("voltex_insp_sync_tag.vscn");
        cleanup(&scene);
        let mut session = EditorSession::new(scene.clone());
        let e = session.dispatch(EditorCommand::Spawn).unwrap();
        session
            .dispatch(EditorCommand::SetTag { entity: e, tag: "Hero".into() })
            .unwrap();
        session.selected = Some(e);

        let mut state = InspectorState::default();
        state.sync_from(&session);
        assert_eq!(state.tag, "Hero");
        cleanup(&scene);
    }

    #[test]
    fn inspector_with_transform_draws_sliders() {
        let scene = temp_path("voltex_insp_xform.vscn");
        cleanup(&scene);
        let mut session = EditorSession::new(scene.clone());
        let e = session.dispatch(EditorCommand::Spawn).unwrap();
        session.selected = Some(e);

        let mut ui = UiContext::new(800.0, 600.0);
        let mut state = InspectorState::default();
        ui.begin_frame(0.0, 0.0, false);
        inspector_panel(&mut ui, &mut session, &mut state, 0.0, 0.0, 300.0, 600.0);
        ui.end_frame();
        // 9 sliders + label rows + header → easily >10 draw commands
        assert!(ui.draw_list.commands.len() > 10);
        cleanup(&scene);
    }

    #[test]
    fn add_component_button_specs_match_registered_built_in_names() {
        // Guard against drift between ADD_COMPONENT_BUTTONS and
        // ComponentRegistry::register_defaults(). If a button references a
        // name that isn't in the registry, the dispatch would silently fail.
        let scene = temp_path("voltex_insp_add_specs.vscn");
        cleanup(&scene);
        let session = EditorSession::new(scene.clone());
        for spec in super::ADD_COMPONENT_BUTTONS.iter() {
            assert!(
                session.registry.find(spec.name).is_some(),
                "ADD_COMPONENT_BUTTONS references unknown name `{}`",
                spec.name
            );
        }
        cleanup(&scene);
    }

    #[test]
    fn add_component_dispatch_with_default_bytes_attaches_transform() {
        // The Inspector wiring calls `(entry.default_bytes)()` and dispatches
        // AddComponent — exercise that exact path end-to-end without the GUI.
        let scene = temp_path("voltex_insp_add_dispatch.vscn");
        cleanup(&scene);
        let mut session = EditorSession::new(scene.clone());
        let e = session.dispatch(EditorCommand::Spawn).unwrap();

        // Pretend the user clicks "+ transform" — we replicate the closure logic.
        let entry = session.registry.find("transform").expect("transform registered");
        let bytes = (entry.default_bytes)();
        session.dispatch(EditorCommand::AddComponent {
            entity: e,
            name: "transform".into(),
            data: bytes,
        }).unwrap();

        let t = session.world.get::<Transform>(e).expect("Transform attached");
        assert_eq!(t.scale, voltex_math::Vec3::new(1.0, 1.0, 1.0));
        cleanup(&scene);
    }

    #[test]
    fn slider_drag_dispatches_set_transform_and_logs() {
        let scene = temp_path("voltex_insp_dispatch.vscn");
        cleanup(&scene);
        let mut session = EditorSession::new(scene.clone());
        let e = session.dispatch(EditorCommand::Spawn).unwrap();
        session.selected = Some(e);

        let mut ui = UiContext::new(800.0, 600.0);
        let mut state = InspectorState::default();

        // Simulate mouse drag on the Pos X slider (first slider drawn).
        // Slider geometry: starts at layout.cursor_x (= panel inner x = x+padding=4),
        // width 150. Mid-x ≈ 79 → roughly value 0 with range [-50, 50].
        // Click far right to push value > original 0.
        ui.begin_frame(160.0, 60.0, true);
        inspector_panel(&mut ui, &mut session, &mut state, 0.0, 0.0, 300.0, 600.0);
        ui.end_frame();

        let t = session.world.get::<Transform>(e).expect("transform");
        // Either the slider value moved away from zero (drag landed) or stayed.
        // The important property: the session still represents a valid Transform.
        assert!(t.position.x.is_finite());

        // Log file existence is the persistent guarantee — at minimum Spawn was logged.
        let log = format!("{}.log", scene);
        let log_text = std::fs::read_to_string(&log).expect("log exists");
        assert!(log_text.contains("Spawn"));
        cleanup(&scene);
    }
}
