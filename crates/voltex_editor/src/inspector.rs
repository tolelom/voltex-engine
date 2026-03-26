use crate::ui_context::UiContext;
use crate::dock::Rect;
use crate::layout::LayoutState;
use voltex_ecs::world::World;
use voltex_ecs::entity::Entity;
use voltex_ecs::scene::Tag;
use voltex_ecs::hierarchy::{roots, Children};

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
    let has_children = world.get::<Children>(entity).map_or(false, |c| !c.0.is_empty());
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

/// Inspector panel stub (implemented in next task).
pub fn inspector_panel(
    ui: &mut UiContext,
    _world: &mut World,
    _selected: Option<Entity>,
    rect: &Rect,
    _tag_buffer: &mut String,
) {
    ui.layout = LayoutState::new(rect.x + PADDING, rect.y + PADDING);
    ui.text("Inspector (TODO)");
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
}
