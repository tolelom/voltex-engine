# Entity Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add entity hierarchy tree and property inspector panels to the editor, supporting Transform/Tag/Parent component display and editing.

**Architecture:** Two public functions (`hierarchy_panel`, `inspector_panel`) in a new `inspector.rs` module that take `UiContext` + `World` and draw IMGUI widgets. No new structs needed. Copy-out pattern for Transform editing to avoid borrow conflicts. Tag editing via caller-owned staging buffer.

**Tech Stack:** Rust, voltex_ecs (World, Entity, Transform, Tag, Parent, Children, roots), voltex_editor (UiContext, Rect, LayoutState, widgets)

**Spec:** `docs/superpowers/specs/2026-03-26-entity-inspector-design.md`

---

### Task 1: hierarchy_panel + tests

**Files:**
- Create: `crates/voltex_editor/src/inspector.rs`
- Modify: `crates/voltex_editor/src/lib.rs`
- Modify: `crates/voltex_editor/Cargo.toml`

- [ ] **Step 1: Add voltex_ecs dependency**

In `crates/voltex_editor/Cargo.toml`, add:
```toml
voltex_ecs.workspace = true
```

- [ ] **Step 2: Write failing tests**

Create `crates/voltex_editor/src/inspector.rs`:

```rust
use crate::ui_context::UiContext;
use crate::dock::Rect;
use crate::layout::LayoutState;
use voltex_ecs::world::World;
use voltex_ecs::entity::Entity;
use voltex_ecs::transform::Transform;
use voltex_ecs::scene::Tag;
use voltex_ecs::hierarchy::{roots, Children, Parent};

const COLOR_SELECTED: [u8; 4] = [0x44, 0x66, 0x88, 0xFF];
const COLOR_TEXT: [u8; 4] = [0xEE, 0xEE, 0xEE, 0xFF];
const LINE_HEIGHT: f32 = 16.0;
const INDENT: f32 = 16.0;
const PADDING: f32 = 4.0;

#[cfg(test)]
mod tests {
    use super::*;
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

        // e2, e3 are children of e1
        voltex_ecs::hierarchy::add_child(&mut world, e1, e2);
        voltex_ecs::hierarchy::add_child(&mut world, e1, e3);

        (world, e1, e2, e3)
    }

    #[test]
    fn test_count_nodes() {
        let (world, _, _, _) = make_test_world();
        let root_entities = roots(&world);
        let count: usize = root_entities.iter().map(|e| count_entity_nodes(&world, *e)).sum();
        assert_eq!(count, 3); // Root + Child1 + Child2
    }

    #[test]
    fn test_hierarchy_panel_draws_commands() {
        let (world, _, _, _) = make_test_world();
        let mut ui = UiContext::new(800.0, 600.0);
        let mut selected: Option<Entity> = None;
        let rect = Rect { x: 0.0, y: 0.0, w: 200.0, h: 400.0 };

        ui.begin_frame(0.0, 0.0, false);
        hierarchy_panel(&mut ui, &world, &mut selected, &rect);
        ui.end_frame();

        assert!(ui.draw_list.commands.len() > 0, "should draw entity names");
    }

    #[test]
    fn test_hierarchy_panel_click_selects() {
        let (world, e1, _, _) = make_test_world();
        let mut ui = UiContext::new(800.0, 600.0);
        let mut selected: Option<Entity> = None;
        let rect = Rect { x: 0.0, y: 0.0, w: 200.0, h: 400.0 };

        // Frame 1: mouse down on first entity row (y ~= PADDING, within LINE_HEIGHT)
        ui.begin_frame(50.0, PADDING + 2.0, true);
        hierarchy_panel(&mut ui, &world, &mut selected, &rect);
        ui.end_frame();

        assert_eq!(selected, Some(e1), "clicking first row should select root entity");
    }

    #[test]
    fn test_hierarchy_panel_empty_world() {
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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p voltex_editor --lib inspector`
Expected: FAIL (functions don't exist)

- [ ] **Step 4: Implement hierarchy_panel**

Add implementation above tests in `inspector.rs`:

```rust
/// Count total nodes in the subtree rooted at entity (including self).
fn count_entity_nodes(world: &World, entity: Entity) -> usize {
    let mut count = 1;
    if let Some(children) = world.get::<Children>(entity) {
        for &child in &children.0 {
            count += count_entity_nodes(world, child);
        }
    }
    count
}

/// Draw a single entity row in the hierarchy tree.
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

    // Build display text
    let has_children = world.get::<Children>(entity).map_or(false, |c| !c.0.is_empty());
    let prefix = if has_children { "> " } else { "  " };
    let name = if let Some(tag) = world.get::<Tag>(entity) {
        format!("{}{}", prefix, tag.0)
    } else {
        format!("{}Entity({})", prefix, entity.id)
    };

    // Draw text
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

    // Recurse into children
    if let Some(children) = world.get::<Children>(entity) {
        let child_list: Vec<Entity> = children.0.clone();
        for child in child_list {
            draw_entity_node(ui, world, child, depth + 1, selected, base_x, row_w);
        }
    }
}

/// Draw the hierarchy panel showing entity tree.
pub fn hierarchy_panel(
    ui: &mut UiContext,
    world: &World,
    selected: &mut Option<Entity>,
    rect: &Rect,
) {
    let root_entities = roots(world);

    // Count total nodes for scroll content height
    let total_nodes: usize = root_entities.iter().map(|e| count_entity_nodes(world, *e)).sum();
    let content_height = total_nodes as f32 * LINE_HEIGHT + PADDING * 2.0;

    ui.begin_scroll_panel(
        9000, // unique scroll panel ID
        rect.x, rect.y, rect.w, rect.h,
        content_height,
    );

    ui.layout.cursor_y = rect.y + PADDING - ui.scroll_offsets.get(&9000).copied().unwrap_or(0.0);

    for &entity in &root_entities {
        draw_entity_node(ui, world, entity, 0, selected, rect.x, rect.w);
    }

    ui.end_scroll_panel();
}
```

Note: The scroll panel sets cursor position internally, but we need to manually track y for our custom row rendering. The `begin_scroll_panel` pushes a scissor rect — we draw inside that clipped area.

Actually, let's simplify: don't use scroll_panel for v1. The spec mentions it, but for a first pass with few entities, just set layout cursor and draw directly. Add scroll in a follow-up if needed, to keep this task focused.

Simplified: just set `ui.layout = LayoutState::new(rect.x + PADDING, rect.y + PADDING)` and draw rows. No scroll for now.

```rust
pub fn hierarchy_panel(
    ui: &mut UiContext,
    world: &World,
    selected: &mut Option<Entity>,
    rect: &Rect,
) {
    let root_entities = roots(world);
    ui.layout = LayoutState::new(rect.x + PADDING, rect.y + PADDING);

    for &entity in &root_entities {
        draw_entity_node(ui, world, entity, 0, selected, rect.x, rect.w);
    }
}
```

- [ ] **Step 5: Add module to lib.rs**

```rust
pub mod inspector;
pub use inspector::{hierarchy_panel, inspector_panel};
```

(inspector_panel will be a stub for now — `pub fn inspector_panel(...) {}`)

- [ ] **Step 6: Run tests**

Run: `cargo test -p voltex_editor --lib inspector -- --nocapture`
Expected: all 4 tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/voltex_editor/Cargo.toml crates/voltex_editor/src/inspector.rs crates/voltex_editor/src/lib.rs
git commit -m "feat(editor): add hierarchy_panel with entity tree display and selection"
```

---

### Task 2: inspector_panel + tests

**Files:**
- Modify: `crates/voltex_editor/src/inspector.rs`

- [ ] **Step 1: Write failing tests**

Add to `mod tests` in inspector.rs:

```rust
#[test]
fn test_inspector_panel_no_selection() {
    let mut world = World::new();
    let mut ui = UiContext::new(800.0, 600.0);
    let rect = Rect { x: 0.0, y: 0.0, w: 300.0, h: 400.0 };

    ui.begin_frame(0.0, 0.0, false);
    let mut tag_buf = String::new();
    inspector_panel(&mut ui, &mut world, None, &rect, &mut tag_buf);
    ui.end_frame();

    // Should have drawn "No entity selected" text
    assert!(ui.draw_list.commands.len() > 0);
}

#[test]
fn test_inspector_panel_with_transform() {
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));

    let mut ui = UiContext::new(800.0, 600.0);
    let rect = Rect { x: 0.0, y: 0.0, w: 300.0, h: 400.0 };

    ui.begin_frame(0.0, 0.0, false);
    let mut tag_buf = String::new();
    inspector_panel(&mut ui, &mut world, Some(e), &rect, &mut tag_buf);
    ui.end_frame();

    // Should have drawn Transform header + sliders
    // Sliders produce multiple draw commands (bg + handle + label text)
    assert!(ui.draw_list.commands.len() > 10);
}

#[test]
fn test_inspector_panel_with_tag() {
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

    // tag_buf should be synced to "TestTag"
    assert_eq!(tag_buf, "TestTag");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p voltex_editor --lib inspector`
Expected: FAIL (inspector_panel is stub)

- [ ] **Step 3: Implement inspector_panel**

Replace the stub with:

```rust
/// Draw the inspector panel for the selected entity.
/// `tag_buffer` is a caller-owned staging buffer for Tag editing.
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

        // Copy out values (immutable borrow ends here)
        let (px, py, pz, rx, ry, rz, sx, sy, sz) = {
            let t = world.get::<Transform>(entity).unwrap();
            (t.position.x, t.position.y, t.position.z,
             t.rotation.x, t.rotation.y, t.rotation.z,
             t.scale.x, t.scale.y, t.scale.z)
        };

        // Sliders (no world borrow)
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

        // Sync buffer from world if it doesn't match
        if let Some(tag) = world.get::<Tag>(entity) {
            if tag_buffer.is_empty() || *tag_buffer != tag.0 {
                *tag_buffer = tag.0.clone();
            }
        }

        let input_x = rect.x + PADDING;
        let input_y = ui.layout.cursor_y;
        let input_w = rect.w - PADDING * 2.0;
        if ui.text_input(8888, tag_buffer, input_x, input_y, input_w) {
            // Buffer changed — write back to world
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
```

- [ ] **Step 4: Update the function signature in lib.rs re-export**

Make sure `inspector_panel` in `pub use` includes the new signature.

- [ ] **Step 5: Run tests**

Run: `cargo test -p voltex_editor --lib inspector -- --nocapture`
Expected: all 7 tests PASS (4 hierarchy + 3 inspector)

- [ ] **Step 6: Commit**

```bash
git add crates/voltex_editor/src/inspector.rs
git commit -m "feat(editor): add inspector_panel with Transform, Tag, Parent editing"
```

---

### Task 3: Integrate into editor_demo

**Files:**
- Modify: `examples/editor_demo/src/main.rs`

- [ ] **Step 1: Add ECS World and demo entities**

Read current `examples/editor_demo/src/main.rs`. Add to AppState:
```rust
world: World,
selected_entity: Option<Entity>,
tag_buffer: String,
```

In `resumed`, after scene mesh creation, create ECS entities that mirror the scene:
```rust
let mut world = World::new();
let ground = world.spawn();
world.add(ground, Transform::from_position(Vec3::new(0.0, 0.0, 0.0)));
world.add(ground, Tag("Ground".to_string()));

let cube1 = world.spawn();
world.add(cube1, Transform::from_position(Vec3::new(0.0, 0.5, 0.0)));
world.add(cube1, Tag("Cube1".to_string()));

let cube2 = world.spawn();
world.add(cube2, Transform::from_position(Vec3::new(2.0, 0.5, 1.0)));
world.add(cube2, Tag("Cube2".to_string()));

let cube3 = world.spawn();
world.add(cube3, Transform::from_position(Vec3::new(-1.5, 0.5, -1.0)));
world.add(cube3, Tag("Cube3".to_string()));

// Cube2 is child of Cube1
voltex_ecs::hierarchy::add_child(&mut world, cube1, cube2);
```

- [ ] **Step 2: Wire panels to hierarchy/inspector**

In the dock panel loop, replace panel 0 (Debug) with Hierarchy, and panel 2 (Properties) with Inspector:
```rust
0 => {
    // Hierarchy panel
    hierarchy_panel(&mut state.ui, &state.world, &mut state.selected_entity, rect);
}
// ... panel 1 stays as viewport ...
2 => {
    // Inspector panel
    inspector_panel(&mut state.ui, &mut state.world, state.selected_entity, rect, &mut state.tag_buffer);
}
```

Update dock panel names to `["Hierarchy", "Viewport", "Inspector", "Console"]`.

- [ ] **Step 3: Build and test**

Run: `cargo build -p editor_demo`
Expected: compiles

Run: `cargo test -p voltex_editor -- --nocapture`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add examples/editor_demo/src/main.rs
git commit -m "feat(editor): integrate hierarchy and inspector panels into editor_demo"
```

---

### Task 4: Update docs

**Files:**
- Modify: `docs/dev/STATUS.md`
- Modify: `docs/dev/DEFERRED.md`

- [ ] **Step 1: Update STATUS.md**

Add to Phase 8-4:
```
- voltex_editor: hierarchy_panel, inspector_panel (Transform/Tag/Parent editing)
```
Update test count.

- [ ] **Step 2: Update DEFERRED.md**

```
- ~~**엔티티 인스펙터**~~ ✅ Transform/Tag/Parent 편집 완료. 커스텀 컴포넌트 리플렉션 미구현.
```

- [ ] **Step 3: Commit**

```bash
git add docs/dev/STATUS.md docs/dev/DEFERRED.md
git commit -m "docs: update STATUS.md and DEFERRED.md with entity inspector"
```
