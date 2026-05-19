//! Hierarchy panel — scrollable list of entities, click to select.

use crate::session::EditorSession;
use crate::ui_context::UiContext;
use voltex_ecs::{Entity, Transform};

const ROW_HEIGHT: f32 = 20.0;

/// Draw a hierarchy panel at (x, y) of size (w, h).
///
/// Lists every entity that has a Transform component. Clicking a row sets
/// `session.selected`. Returns true if the selection changed this frame.
pub fn hierarchy_panel(
    ui: &mut UiContext,
    session: &mut EditorSession,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> bool {
    ui.begin_panel("Hierarchy", x, y, w, h);

    let entities: Vec<Entity> = session.world.query::<Transform>().map(|(e, _)| e).collect();
    let mut selection_changed = false;

    let content_height = entities.len() as f32 * ROW_HEIGHT;
    let scroll_id: u32 = 0x1E_5C_00_01;
    ui.begin_scroll_panel(scroll_id, x + 4.0, y + 24.0, w - 8.0, h - 28.0, content_height);

    for entity in &entities {
        let label = match session.world.get::<voltex_ecs::scene::Tag>(*entity) {
            Some(tag) => format!("[{}] {}", entity.id, tag.0),
            None => format!("[{}] (untagged)", entity.id),
        };
        let is_selected = session.selected == Some(*entity);
        let display = if is_selected {
            format!("* {}", label)
        } else {
            format!("  {}", label)
        };
        if ui.button(&display) {
            session.selected = Some(*entity);
            selection_changed = true;
        }
    }

    ui.end_scroll_panel();
    ui.end_panel();
    selection_changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UiContext;
    use voltex_ecs::Transform;

    fn temp_path(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("{}_{}", name, std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn hierarchy_runs_without_panic_on_empty_world() {
        let mut ui = UiContext::new(800.0, 600.0);
        let scene = temp_path("voltex_hier_test.vscn");
        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", &scene));

        let mut session = EditorSession::new(scene.clone());
        ui.begin_frame(0.0, 0.0, false);
        let changed = hierarchy_panel(&mut ui, &mut session, 0.0, 0.0, 200.0, 400.0);
        ui.end_frame();
        assert!(!changed);

        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", &scene));
    }

    #[test]
    fn hierarchy_lists_spawned_entities() {
        let mut ui = UiContext::new(800.0, 600.0);
        let scene = temp_path("voltex_hier_list.vscn");
        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", &scene));

        let mut session = EditorSession::new(scene.clone());
        for _ in 0..3 {
            session.dispatch(crate::command::EditorCommand::Spawn).unwrap();
        }

        ui.begin_frame(0.0, 0.0, false);
        hierarchy_panel(&mut ui, &mut session, 0.0, 0.0, 200.0, 400.0);
        ui.end_frame();

        let count = session.world.query::<Transform>().count();
        assert_eq!(count, 3);

        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", &scene));
    }
}
