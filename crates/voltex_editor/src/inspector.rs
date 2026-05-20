use crate::ui_context::UiContext;
use crate::dock::Rect;
use crate::layout::LayoutState;
use voltex_ecs::world::World;
use voltex_ecs::entity::Entity;
use voltex_ecs::scene::Tag;
use voltex_ecs::hierarchy::{roots, Children, Parent};
use voltex_ecs::transform::Transform;

const COLOR_SELECTED: [u8; 4] = [0x44, 0x66, 0x88, 0xFF];
const COLOR_TEXT: [u8; 4] = [0xEE, 0xEE, 0xEE, 0xFF];
const LINE_HEIGHT: f32 = 16.0;
const INDENT: f32 = 16.0;
const PADDING: f32 = 4.0;

/// Count total nodes in subtree (including self).
pub fn count_entity_nodes(world: &World, entity: Entity) -> usize {
    let mut count = 1;
    if let Some(children) = world.get::<Children>(entity) {
        for &child in &children.0 {
            count += count_entity_nodes(world, child);
        }
    }
    count
}

/// Draw a single entity row, then recurse into children.
fn draw_entity_node(
    ui: &mut UiContext,
    world: &World,
    entity: Entity,
    depth: usize,
    selected: &mut Option<Entity>,
    base_x: f32,
    row_w: f32,
) {
    let y = ui.layout.cursor_y;
    let x = base_x + PADDING + depth as f32 * INDENT;

    // Highlight selected
    if *selected == Some(entity) {
        ui.draw_list.add_rect(base_x, y, row_w, LINE_HEIGHT, COLOR_SELECTED);
    }

    // Click detection
    if ui.mouse_clicked && ui.mouse_in_rect(base_x, y, row_w, LINE_HEIGHT) {
        *selected = Some(entity);
    }

    // Build label
    let has_children = world.get::<Children>(entity).is_some_and(|c| !c.0.is_empty());
    let prefix = if has_children { "> " } else { "  " };
    let name = if let Some(tag) = world.get::<Tag>(entity) {
        format!("{}{}", prefix, tag.0)
    } else {
        format!("{}Entity({})", prefix, entity.id)
    };

    // Draw text (inline glyph rendering)
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

    // Recurse children
    if let Some(children) = world.get::<Children>(entity) {
        let child_list: Vec<Entity> = children.0.clone();
        for child in child_list {
            draw_entity_node(ui, world, child, depth + 1, selected, base_x, row_w);
        }
    }
}

/// Hierarchy panel: displays entity tree, handles selection.
pub fn hierarchy_panel(
    ui: &mut UiContext,
    world: &World,
    selected: &mut Option<Entity>,
    rect: &Rect,
) {
    ui.layout = LayoutState::new(rect.x + PADDING, rect.y + PADDING);
    let root_entities = roots(world);
    for &entity in &root_entities {
        draw_entity_node(ui, world, entity, 0, selected, rect.x, rect.w);
    }
}

/// Inspector panel: edit Transform, Tag, Parent for selected entity.
/// `tag_buffer` is caller-owned staging buffer for Tag text input.
pub fn inspector_panel(
    ui: &mut UiContext,
    world: &mut World,
    selected: Option<Entity>,
    rect: &Rect,
    tag_buffer: &mut String,
) {
    ui.layout = LayoutState::new(rect.x + PADDING, rect.y + PADDING);

    let entity = match selected {
        Some(e) => e,
        None => {
            ui.text("No entity selected");
            return;
        }
    };

    // --- Transform ---
    if world.has_component::<Transform>(entity) {
        ui.text("-- Transform --");

        // Copy out values (immutable borrow ends with block)
        let (px, py, pz, rx, ry, rz, sx, sy, sz) = {
            let t = world.get::<Transform>(entity).unwrap();
            (t.position.x, t.position.y, t.position.z,
             t.rotation.x, t.rotation.y, t.rotation.z,
             t.scale.x, t.scale.y, t.scale.z)
        };

        // Sliders (no world borrow active)
        let new_px = ui.slider("Pos X", px, -50.0, 50.0);
        let new_py = ui.slider("Pos Y", py, -50.0, 50.0);
        let new_pz = ui.slider("Pos Z", pz, -50.0, 50.0);
        let new_rx = ui.slider("Rot X", rx, -3.15, 3.15);
        let new_ry = ui.slider("Rot Y", ry, -3.15, 3.15);
        let new_rz = ui.slider("Rot Z", rz, -3.15, 3.15);
        let new_sx = ui.slider("Scl X", sx, 0.01, 10.0);
        let new_sy = ui.slider("Scl Y", sy, 0.01, 10.0);
        let new_sz = ui.slider("Scl Z", sz, 0.01, 10.0);

        // Write back (mutable borrow)
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position.x = new_px;
            t.position.y = new_py;
            t.position.z = new_pz;
            t.rotation.x = new_rx;
            t.rotation.y = new_ry;
            t.rotation.z = new_rz;
            t.scale.x = new_sx;
            t.scale.y = new_sy;
            t.scale.z = new_sz;
        }
    }

    // --- Tag ---
    if world.has_component::<Tag>(entity) {
        ui.text("-- Tag --");

        // Sync buffer from world
        if let Some(tag) = world.get::<Tag>(entity) {
            if tag_buffer.is_empty() || *tag_buffer != tag.0 {
                *tag_buffer = tag.0.clone();
            }
        }

        let input_x = rect.x + PADDING;
        let input_y = ui.layout.cursor_y;
        let input_w = (rect.w - PADDING * 2.0).max(50.0);
        if ui.text_input(8888, tag_buffer, input_x, input_y, input_w) {
            if let Some(tag) = world.get_mut::<Tag>(entity) {
                tag.0 = tag_buffer.clone();
            }
        }
        ui.layout.advance_line();
    }

    // --- Parent ---
    if let Some(parent) = world.get::<Parent>(entity) {
        ui.text("-- Parent --");
        let parent_text = format!("Parent: Entity({})", parent.0.id);
        ui.text(&parent_text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::transform::Transform;
    use voltex_math::Vec3;

    fn make_test_world() -> (World, Entity, Entity, Entity) {
        let mut world = World::new();
        let e1 = world.spawn();
        world.add(e1, Transform::from_position(Vec3::new(0.0, 0.0, 0.0)));
        world.add(e1, Tag("Root".to_string()));

        let e2 = world.spawn();
        world.add(e2, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        world.add(e2, Tag("Child1".to_string()));

        let e3 = world.spawn();
        world.add(e3, Transform::from_position(Vec3::new(2.0, 0.0, 0.0)));
        world.add(e3, Tag("Child2".to_string()));

        voltex_ecs::hierarchy::add_child(&mut world, e1, e2);
        voltex_ecs::hierarchy::add_child(&mut world, e1, e3);

        (world, e1, e2, e3)
    }

    #[test]
    fn test_count_nodes() {
        let (world, e1, _, _) = make_test_world();
        assert_eq!(count_entity_nodes(&world, e1), 3);
    }

    #[test]
    fn test_hierarchy_draws_commands() {
        let (world, _, _, _) = make_test_world();
        let mut ui = UiContext::new(800.0, 600.0);
        let mut selected: Option<Entity> = None;
        let rect = Rect { x: 0.0, y: 0.0, w: 200.0, h: 400.0 };

        ui.begin_frame(0.0, 0.0, false);
        hierarchy_panel(&mut ui, &world, &mut selected, &rect);
        ui.end_frame();

        assert!(ui.draw_list.commands.len() > 0);
    }

    #[test]
    fn test_hierarchy_click_selects() {
        let (world, e1, _, _) = make_test_world();
        let mut ui = UiContext::new(800.0, 600.0);
        let mut selected: Option<Entity> = None;
        let rect = Rect { x: 0.0, y: 0.0, w: 200.0, h: 400.0 };

        // Click on first row (y = PADDING + small offset)
        ui.begin_frame(50.0, PADDING + 2.0, true);
        hierarchy_panel(&mut ui, &world, &mut selected, &rect);
        ui.end_frame();

        assert_eq!(selected, Some(e1));
    }

    #[test]
    fn test_hierarchy_empty_world() {
        let world = World::new();
        let mut ui = UiContext::new(800.0, 600.0);
        let mut selected: Option<Entity> = None;
        let rect = Rect { x: 0.0, y: 0.0, w: 200.0, h: 400.0 };

        ui.begin_frame(0.0, 0.0, false);
        hierarchy_panel(&mut ui, &world, &mut selected, &rect);
        ui.end_frame();

        assert!(selected.is_none());
    }

    #[test]
    fn test_inspector_no_selection() {
        let mut world = World::new();
        let mut ui = UiContext::new(800.0, 600.0);
        let rect = Rect { x: 0.0, y: 0.0, w: 300.0, h: 400.0 };

        ui.begin_frame(0.0, 0.0, false);
        let mut tag_buf = String::new();
        inspector_panel(&mut ui, &mut world, None, &rect, &mut tag_buf);
        ui.end_frame();

        // "No entity selected" text produces draw commands
        assert!(ui.draw_list.commands.len() > 0);
    }

    #[test]
    fn test_inspector_with_transform() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));

        let mut ui = UiContext::new(800.0, 600.0);
        let rect = Rect { x: 0.0, y: 0.0, w: 300.0, h: 400.0 };

        ui.begin_frame(0.0, 0.0, false);
        let mut tag_buf = String::new();
        inspector_panel(&mut ui, &mut world, Some(e), &rect, &mut tag_buf);
        ui.end_frame();

        // Header + 9 sliders produce many draw commands
        assert!(ui.draw_list.commands.len() > 10);
    }

    #[test]
    fn test_inspector_with_tag() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::new());
        world.add(e, Tag("TestTag".to_string()));

        let mut ui = UiContext::new(800.0, 600.0);
        let rect = Rect { x: 0.0, y: 0.0, w: 300.0, h: 400.0 };

        ui.begin_frame(0.0, 0.0, false);
        let mut tag_buf = String::new();
        inspector_panel(&mut ui, &mut world, Some(e), &rect, &mut tag_buf);
        ui.end_frame();

        // tag_buf should be synced from world
        assert_eq!(tag_buf, "TestTag");
    }
}
