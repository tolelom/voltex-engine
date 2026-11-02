//! Hierarchy panel — tree view of entities (Parent/Children), click to select.
//!
//! Single source of truth for selection: `session.selected`. Click on a row
//! sets `session.selected = Some(entity)` so the inspector picks it up.

use crate::session::EditorSession;
use crate::ui_context::UiContext;
use voltex_ecs::hierarchy::{roots, Children};
use voltex_ecs::scene::Tag;
use voltex_ecs::{Entity, World};

const LINE_HEIGHT: f32 = 16.0;
const INDENT: f32 = 16.0;
const PADDING: f32 = 4.0;
const COLOR_SELECTED: [u8; 4] = [0x44, 0x66, 0x88, 0xFF];
const COLOR_TEXT: [u8; 4] = [0xEE, 0xEE, 0xEE, 0xFF];

/// Count total nodes in subtree (including self). Useful for scroll sizing.
pub fn count_entity_nodes(world: &World, entity: Entity) -> usize {
    let mut count = 1;
    if let Some(children) = world.get::<Children>(entity) {
        for &child in &children.0 {
            count += count_entity_nodes(world, child);
        }
    }
    count
}

#[allow(clippy::too_many_arguments)] // Recursive draw — packing into a struct would only add noise.
fn draw_entity_node(
    ui: &mut UiContext,
    world: &World,
    entity: Entity,
    depth: usize,
    selected: &mut Option<Entity>,
    selection_changed: &mut bool,
    base_x: f32,
    row_w: f32,
) {
    let y = ui.layout.cursor_y;
    let x = base_x + PADDING + depth as f32 * INDENT;

    if *selected == Some(entity) {
        ui.draw_list.add_rect(base_x, y, row_w, LINE_HEIGHT, COLOR_SELECTED);
    }

    if ui.mouse_clicked && ui.mouse_in_rect(base_x, y, row_w, LINE_HEIGHT) {
        *selected = Some(entity);
        *selection_changed = true;
    }

    let has_children = world.get::<Children>(entity).is_some_and(|c| !c.0.is_empty());
    let prefix = if has_children { "> " } else { "  " };
    let name = if let Some(tag) = world.get::<Tag>(entity) {
        format!("{}{}", prefix, tag.0)
    } else {
        format!("{}Entity({})", prefix, entity.id)
    };

    let gw = ui.font.glyph_width as f32;
    let gh = ui.font.glyph_height as f32;
    let text_y = y + (LINE_HEIGHT - gh) * 0.5;
    let mut cx = x;
    for ch in name.chars() {
        let (u0, v0, u1, v1) = ui.font.glyph_uv(ch);
        ui.draw_list.add_rect_uv(cx, text_y, gw, gh, u0, v0, u1, v1, COLOR_TEXT);
        cx += gw;
    }

    ui.layout.cursor_y += LINE_HEIGHT;

    if let Some(children) = world.get::<Children>(entity) {
        let child_list: Vec<Entity> = children.0.clone();
        for child in child_list {
            draw_entity_node(ui, world, child, depth + 1, selected, selection_changed, base_x, row_w);
        }
    }
}

/// Hierarchy panel: tree view rooted at entities without a Parent, recursing
/// into Children. Clicking a row sets `session.selected`. Returns true if
/// selection changed this frame.
pub fn hierarchy_panel(
    ui: &mut UiContext,
    session: &mut EditorSession,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> bool {
    ui.begin_panel("Hierarchy", x, y, w, h);

    // begin_panel already positioned layout.cursor_y just under the title bar.
    // Use that y as the start of the tree rows.
    let root_entities = roots(&session.world);
    let mut selection_changed = false;
    for entity in root_entities {
        draw_entity_node(
            ui,
            &session.world,
            entity,
            0,
            &mut session.selected,
            &mut selection_changed,
            x,
            w,
        );
    }

    ui.end_panel();
    let _ = (y, h); // currently unused (no scrolling); reserved for future
    selection_changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UiContext;
    use voltex_ecs::Transform;
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

    fn make_session_with_hierarchy() -> (EditorSession, Entity, Entity, Entity, String) {
        let scene = temp_path("voltex_hier_unified.vscn");
        cleanup(&scene);
        let mut session = EditorSession::new(scene.clone());
        let e1 = session.world.spawn();
        session.world.add(e1, Transform::from_position(Vec3::new(0.0, 0.0, 0.0)));
        session.world.add(e1, Tag("Root".to_string()));
        let e2 = session.world.spawn();
        session.world.add(e2, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        session.world.add(e2, Tag("Child1".to_string()));
        let e3 = session.world.spawn();
        session.world.add(e3, Transform::from_position(Vec3::new(2.0, 0.0, 0.0)));
        session.world.add(e3, Tag("Child2".to_string()));
        voltex_ecs::hierarchy::add_child(&mut session.world, e1, e2);
        voltex_ecs::hierarchy::add_child(&mut session.world, e1, e3);
        (session, e1, e2, e3, scene)
    }

    #[test]
    fn count_nodes_recurses_into_children() {
        let (session, e1, _, _, scene) = make_session_with_hierarchy();
        assert_eq!(count_entity_nodes(&session.world, e1), 3);
        cleanup(&scene);
    }

    #[test]
    fn hierarchy_panel_runs_on_empty_world() {
        let scene = temp_path("voltex_hier_empty.vscn");
        cleanup(&scene);
        let mut session = EditorSession::new(scene.clone());
        let mut ui = UiContext::new(800.0, 600.0);
        ui.begin_frame(0.0, 0.0, false);
        let changed = hierarchy_panel(&mut ui, &mut session, 0.0, 0.0, 200.0, 400.0);
        ui.end_frame();
        assert!(!changed);
        assert_eq!(session.selected, None);
        cleanup(&scene);
    }

    #[test]
    fn hierarchy_panel_lists_root_and_children() {
        let (mut session, _, _, _, scene) = make_session_with_hierarchy();
        let mut ui = UiContext::new(800.0, 600.0);
        ui.begin_frame(0.0, 0.0, false);
        hierarchy_panel(&mut ui, &mut session, 0.0, 0.0, 200.0, 400.0);
        ui.end_frame();
        assert!(!ui.draw_list.commands.is_empty());
        cleanup(&scene);
    }

    #[test]
    fn hierarchy_click_selects_root_entity() {
        let (mut session, e1, _, _, scene) = make_session_with_hierarchy();
        // Title bar takes glyph_height + padding * 2 = 12 + 8 = 20px (approx),
        // so click around y=24 lands on the first row.
        let mut ui = UiContext::new(800.0, 600.0);
        ui.begin_frame(50.0, 28.0, true);
        let changed = hierarchy_panel(&mut ui, &mut session, 0.0, 0.0, 200.0, 400.0);
        ui.end_frame();
        assert!(changed);
        assert_eq!(session.selected, Some(e1));
        cleanup(&scene);
    }
}
