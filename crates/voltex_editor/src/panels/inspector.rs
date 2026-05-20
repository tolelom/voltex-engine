//! Inspector panel — view/edit components of the selected entity.

use crate::command::EditorCommand;
use crate::session::EditorSession;
use crate::ui_context::UiContext;
use voltex_ecs::Transform;
use voltex_math::Vec3;

/// Persistent buffers for the inspector's text inputs. The inspector is stateless
/// across frames except for these strings (the source of truth is the World).
#[derive(Default)]
pub struct InspectorState {
    pub px: String,
    pub py: String,
    pub pz: String,
    pub rx: String,
    pub ry: String,
    pub rz: String,
    pub sx: String,
    pub sy: String,
    pub sz: String,
    pub tag: String,
    pub last_entity_id: Option<u32>,
}

impl InspectorState {
    /// Refresh the buffers from the world if the selected entity changed since
    /// last frame. Idempotent if selection is stable.
    fn sync_from(&mut self, session: &EditorSession) {
        let Some(entity) = session.selected else {
            self.last_entity_id = None;
            return;
        };
        if self.last_entity_id == Some(entity.id) {
            return;
        }
        self.last_entity_id = Some(entity.id);

        if let Some(t) = session.world.get::<Transform>(entity) {
            self.px = format!("{}", t.position.x);
            self.py = format!("{}", t.position.y);
            self.pz = format!("{}", t.position.z);
            self.rx = format!("{}", t.rotation.x);
            self.ry = format!("{}", t.rotation.y);
            self.rz = format!("{}", t.rotation.z);
            self.sx = format!("{}", t.scale.x);
            self.sy = format!("{}", t.scale.y);
            self.sz = format!("{}", t.scale.z);
        }
        self.tag = session
            .world
            .get::<voltex_ecs::scene::Tag>(entity)
            .map(|t| t.0.clone())
            .unwrap_or_default();
    }

    fn parsed_vec(&self, x: &str, y: &str, z: &str) -> Option<Vec3> {
        let x = x.parse::<f32>().ok()?;
        let y = y.parse::<f32>().ok()?;
        let z = z.parse::<f32>().ok()?;
        Some(Vec3::new(x, y, z))
    }
}

/// Draw the inspector panel. When any text_input commits a parseable change,
/// dispatch a SetTransform or SetTag command.
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

    let mut tag_changed = false;
    let mut transform_changed = false;
    let row_y = |i: f32| y + 48.0 + i * 22.0;

    if ui.text_input(0x1_5A_00_01, &mut state.tag, x + 6.0, row_y(0.0), w - 12.0) {
        tag_changed = true;
    }

    ui.text("position x/y/z:");
    if ui.text_input(0x1_5A_01_01, &mut state.px, x + 6.0, row_y(2.0), (w - 12.0) / 3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_01_02, &mut state.py, x + 6.0 + (w - 12.0)/3.0, row_y(2.0), (w - 12.0)/3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_01_03, &mut state.pz, x + 6.0 + 2.0*(w - 12.0)/3.0, row_y(2.0), (w - 12.0)/3.0) { transform_changed = true; }

    ui.text("rotation x/y/z:");
    if ui.text_input(0x1_5A_02_01, &mut state.rx, x + 6.0, row_y(4.0), (w - 12.0)/3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_02_02, &mut state.ry, x + 6.0 + (w - 12.0)/3.0, row_y(4.0), (w - 12.0)/3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_02_03, &mut state.rz, x + 6.0 + 2.0*(w - 12.0)/3.0, row_y(4.0), (w - 12.0)/3.0) { transform_changed = true; }

    ui.text("scale x/y/z:");
    if ui.text_input(0x1_5A_03_01, &mut state.sx, x + 6.0, row_y(6.0), (w - 12.0)/3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_03_02, &mut state.sy, x + 6.0 + (w - 12.0)/3.0, row_y(6.0), (w - 12.0)/3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_03_03, &mut state.sz, x + 6.0 + 2.0*(w - 12.0)/3.0, row_y(6.0), (w - 12.0)/3.0) { transform_changed = true; }

    if ui.button("Despawn") {
        let _ = session.dispatch(EditorCommand::Despawn { entity });
        session.selected = None;
        ui.end_panel();
        return;
    }

    if tag_changed {
        let _ = session.dispatch(EditorCommand::SetTag {
            entity,
            tag: state.tag.clone(),
        });
    }

    if transform_changed {
        let p = state.parsed_vec(&state.px, &state.py, &state.pz);
        let r = state.parsed_vec(&state.rx, &state.ry, &state.rz);
        let s = state.parsed_vec(&state.sx, &state.sy, &state.sz);
        if let (Some(p), Some(r), Some(s)) = (p, r, s) {
            let _ = session.dispatch(EditorCommand::SetTransform {
                entity,
                position: p,
                rotation: r,
                scale: s,
            });
        }
    }

    ui.end_panel();
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

    #[test]
    fn inspector_runs_with_no_selection() {
        let mut ui = UiContext::new(800.0, 600.0);
        let scene = temp_path("voltex_insp_test.vscn");
        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", &scene));

        let mut session = EditorSession::new(scene.clone());
        let mut state = InspectorState::default();
        ui.begin_frame(0.0, 0.0, false);
        inspector_panel(&mut ui, &mut session, &mut state, 0.0, 0.0, 300.0, 400.0);
        ui.end_frame();

        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", &scene));
    }

    #[test]
    fn inspector_sync_loads_transform_into_buffers() {
        let scene = temp_path("voltex_insp_sync.vscn");
        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", &scene));

        let mut session = EditorSession::new(scene.clone());
        let e = session.dispatch(EditorCommand::Spawn).unwrap();
        session.dispatch(EditorCommand::SetTransform {
            entity: e,
            position: Vec3::new(4.5, 0.0, -2.0),
            rotation: Vec3::ZERO,
            scale: Vec3::new(1.0, 1.0, 1.0),
        }).unwrap();
        session.selected = Some(e);

        let mut state = InspectorState::default();
        state.sync_from(&session);
        assert_eq!(state.px, "4.5");
        assert_eq!(state.pz, "-2");

        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", &scene));
    }
}
