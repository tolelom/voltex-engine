# Editor Docking System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a binary split tree docking system to voltex_editor with resizable panels and tab switching.

**Architecture:** Binary tree of Split/Leaf nodes. Split nodes divide space by ratio, Leaf nodes hold tabs. DockTree owns the tree, caches layout results, handles resize drag and tab clicks internally. No unsafe code — borrow issues resolved by collecting mutation targets before applying.

**Tech Stack:** Rust, existing voltex_editor IMGUI (UiContext, DrawList, FontAtlas, LayoutState)

**Spec:** `docs/superpowers/specs/2026-03-26-editor-docking-design.md`

---

### Task 1: Data model + layout algorithm

**Files:**
- Create: `crates/voltex_editor/src/dock.rs`
- Modify: `crates/voltex_editor/src/lib.rs`

- [ ] **Step 1: Write failing tests for Rect and layout**

Add to `crates/voltex_editor/src/dock.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_contains() {
        let r = Rect { x: 10.0, y: 20.0, w: 100.0, h: 50.0 };
        assert!(r.contains(50.0, 40.0));
        assert!(!r.contains(5.0, 40.0));
        assert!(!r.contains(50.0, 80.0));
    }

    #[test]
    fn test_layout_single_leaf() {
        let mut dock = DockTree::new(
            DockNode::leaf(vec![0]),
            vec!["Panel0"],
        );
        let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
        assert_eq!(areas.len(), 1);
        assert_eq!(areas[0].0, 0);
        let r = &areas[0].1;
        assert!((r.y - 20.0).abs() < 1e-3);
        assert!((r.h - 280.0).abs() < 1e-3);
    }

    #[test]
    fn test_layout_horizontal_split() {
        let mut dock = DockTree::new(
            DockNode::split(Axis::Horizontal, 0.25, DockNode::leaf(vec![0]), DockNode::leaf(vec![1])),
            vec!["Left", "Right"],
        );
        let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
        assert_eq!(areas.len(), 2);
        let left = areas.iter().find(|(id, _)| *id == 0).unwrap();
        assert!((left.1.w - 100.0).abs() < 1e-3);
        let right = areas.iter().find(|(id, _)| *id == 1).unwrap();
        assert!((right.1.w - 300.0).abs() < 1e-3);
    }

    #[test]
    fn test_layout_vertical_split() {
        let mut dock = DockTree::new(
            DockNode::split(Axis::Vertical, 0.5, DockNode::leaf(vec![0]), DockNode::leaf(vec![1])),
            vec!["Top", "Bottom"],
        );
        let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
        let top = areas.iter().find(|(id, _)| *id == 0).unwrap();
        assert!((top.1.h - 130.0).abs() < 1e-3); // 150 - 20 tab bar
    }

    #[test]
    fn test_layout_nested_split() {
        let mut dock = DockTree::new(
            DockNode::split(
                Axis::Horizontal, 0.25,
                DockNode::leaf(vec![0]),
                DockNode::split(Axis::Vertical, 0.5, DockNode::leaf(vec![1]), DockNode::leaf(vec![2])),
            ),
            vec!["A", "B", "C"],
        );
        let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
        assert_eq!(areas.len(), 3);
    }

    #[test]
    fn test_layout_active_tab_only() {
        let mut dock = DockTree::new(
            DockNode::Leaf { tabs: vec![0, 1, 2], active: 1 },
            vec!["A", "B", "C"],
        );
        let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
        assert_eq!(areas[0].0, 1);
    }

    #[test]
    fn test_active_clamped_if_out_of_bounds() {
        let mut dock = DockTree::new(
            DockNode::Leaf { tabs: vec![0], active: 5 },
            vec!["A"],
        );
        let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
        assert_eq!(areas[0].0, 0);
    }

    #[test]
    #[should_panic]
    fn test_empty_tabs_panics() {
        let mut dock = DockTree::new(
            DockNode::Leaf { tabs: vec![], active: 0 },
            vec![],
        );
        dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p voltex_editor --lib dock -- --nocapture`
Expected: compilation errors (types don't exist)

- [ ] **Step 3: Implement data model and layout**

Write the implementation in `crates/voltex_editor/src/dock.rs`. Key design decisions:

- `layout_recursive` is a **free function** taking `&DockNode`, `Rect`, and `&mut Vec<LeafLayout>` / `&mut Vec<SplitLayout>` — avoids borrow issues with `&mut self` vs `&self.root`.
- No `unsafe` code. No raw pointers.
- `debug_assert!(!tabs.is_empty())` in `layout_recursive` for the empty tabs invariant.
- `LeafLayout` includes `leaf_index: usize` for mutation targeting.

```rust
const TAB_BAR_HEIGHT: f32 = 20.0;
const MIN_RATIO: f32 = 0.1;
const MAX_RATIO: f32 = 0.9;
const RESIZE_HANDLE_HALF: f32 = 3.0;
const GLYPH_W: f32 = 8.0;
const TAB_PADDING: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32, pub y: f32, pub w: f32, pub h: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Axis { Horizontal, Vertical }

pub enum DockNode {
    Leaf { tabs: Vec<u32>, active: usize },
    Split { axis: Axis, ratio: f32, children: [Box<DockNode>; 2] },
}

impl DockNode {
    pub fn leaf(tabs: Vec<u32>) -> Self {
        DockNode::Leaf { tabs, active: 0 }
    }
    pub fn split(axis: Axis, ratio: f32, a: DockNode, b: DockNode) -> Self {
        DockNode::Split { axis, ratio: ratio.clamp(MIN_RATIO, MAX_RATIO), children: [Box::new(a), Box::new(b)] }
    }
}

pub struct LeafLayout {
    pub leaf_index: usize,
    pub tabs: Vec<u32>,
    pub active: usize,
    pub tab_bar_rect: Rect,
    pub content_rect: Rect,
}

struct SplitLayout {
    rect: Rect,
    axis: Axis,
    boundary: f32,
    path: Vec<usize>,
}

struct ResizeState {
    path: Vec<usize>,
    axis: Axis,
    origin: f32,
    size: f32,
}

pub struct DockTree {
    root: DockNode,
    names: Vec<&'static str>,
    cached_leaves: Vec<LeafLayout>,
    cached_splits: Vec<SplitLayout>,
    resizing: Option<ResizeState>,
    prev_mouse_down: bool,
}

fn layout_recursive(
    node: &DockNode,
    rect: Rect,
    path: &mut Vec<usize>,
    leaf_counter: &mut usize,
    leaves: &mut Vec<LeafLayout>,
    splits: &mut Vec<SplitLayout>,
) {
    match node {
        DockNode::Leaf { tabs, active } => {
            debug_assert!(!tabs.is_empty(), "DockNode::Leaf must have at least one tab");
            let idx = *leaf_counter;
            *leaf_counter += 1;
            leaves.push(LeafLayout {
                leaf_index: idx,
                tabs: tabs.clone(),
                active: (*active).min(tabs.len() - 1),
                tab_bar_rect: Rect { x: rect.x, y: rect.y, w: rect.w, h: TAB_BAR_HEIGHT },
                content_rect: Rect { x: rect.x, y: rect.y + TAB_BAR_HEIGHT, w: rect.w, h: (rect.h - TAB_BAR_HEIGHT).max(0.0) },
            });
        }
        DockNode::Split { axis, ratio, children } => {
            let (r1, r2, boundary) = match axis {
                Axis::Horizontal => {
                    let w1 = rect.w * ratio;
                    let b = rect.x + w1;
                    (Rect { x: rect.x, y: rect.y, w: w1, h: rect.h },
                     Rect { x: b, y: rect.y, w: rect.w - w1, h: rect.h }, b)
                }
                Axis::Vertical => {
                    let h1 = rect.h * ratio;
                    let b = rect.y + h1;
                    (Rect { x: rect.x, y: rect.y, w: rect.w, h: h1 },
                     Rect { x: rect.x, y: b, w: rect.w, h: rect.h - h1 }, b)
                }
            };
            splits.push(SplitLayout { rect, axis: *axis, boundary, path: path.clone() });
            path.push(0);
            layout_recursive(&children[0], r1, path, leaf_counter, leaves, splits);
            path.pop();
            path.push(1);
            layout_recursive(&children[1], r2, path, leaf_counter, leaves, splits);
            path.pop();
        }
    }
}

impl DockTree {
    pub fn new(root: DockNode, names: Vec<&'static str>) -> Self {
        DockTree { root, names, cached_leaves: Vec::new(), cached_splits: Vec::new(), resizing: None, prev_mouse_down: false }
    }

    pub fn layout(&mut self, rect: Rect) -> Vec<(u32, Rect)> {
        self.cached_leaves.clear();
        self.cached_splits.clear();
        let mut path = Vec::new();
        let mut counter = 0;
        layout_recursive(&self.root, rect, &mut path, &mut counter, &mut self.cached_leaves, &mut self.cached_splits);
        self.cached_leaves.iter().map(|l| {
            let active = l.active.min(l.tabs.len().saturating_sub(1));
            (l.tabs[active], l.content_rect)
        }).collect()
    }
}
```

- [ ] **Step 4: Add module to lib.rs**

```rust
pub mod dock;
pub use dock::{DockTree, DockNode, Axis, Rect, LeafLayout};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p voltex_editor --lib dock -- --nocapture`
Expected: all 8 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/voltex_editor/src/dock.rs crates/voltex_editor/src/lib.rs
git commit -m "feat(editor): add dock tree data model and layout algorithm"
```

---

### Task 2: Tab click + resize drag (update method)

**Files:**
- Modify: `crates/voltex_editor/src/dock.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn test_tab_click_switches_active() {
    let mut dock = DockTree::new(
        DockNode::Leaf { tabs: vec![0, 1, 2], active: 0 },
        vec!["A", "B", "C"],
    );
    dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    // Tab "A": w = 1*8+8 = 16, tab "B" starts at x=16. Click at x=20 (inside "B").
    dock.update(20.0, 10.0, true);  // just_clicked detected here
    dock.update(20.0, 10.0, false);
    let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    assert_eq!(areas[0].0, 1);
}

#[test]
fn test_tab_click_no_change_on_single_tab() {
    let mut dock = DockTree::new(DockNode::leaf(vec![0]), vec!["Only"]);
    dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    dock.update(10.0, 10.0, true);
    dock.update(10.0, 10.0, false);
    let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    assert_eq!(areas[0].0, 0);
}

#[test]
fn test_resize_horizontal_drag() {
    let mut dock = DockTree::new(
        DockNode::split(Axis::Horizontal, 0.5, DockNode::leaf(vec![0]), DockNode::leaf(vec![1])),
        vec!["L", "R"],
    );
    dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    // Boundary at x=200. Click on it, drag to x=120.
    dock.update(200.0, 150.0, true);
    dock.update(120.0, 150.0, true);
    dock.update(120.0, 150.0, false);
    let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    let left = areas.iter().find(|(id, _)| *id == 0).unwrap();
    assert!((left.1.w - 120.0).abs() < 5.0, "left w={}", left.1.w);
}

#[test]
fn test_resize_clamps_ratio() {
    let mut dock = DockTree::new(
        DockNode::split(Axis::Horizontal, 0.5, DockNode::leaf(vec![0]), DockNode::leaf(vec![1])),
        vec!["L", "R"],
    );
    dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    dock.update(200.0, 150.0, true);
    dock.update(5.0, 150.0, true);
    dock.update(5.0, 150.0, false);
    let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    let left = areas.iter().find(|(id, _)| *id == 0).unwrap();
    assert!((left.1.w - 40.0).abs() < 1e-3, "left w={}", left.1.w); // 400*0.1=40
}

#[test]
fn test_resize_priority_over_tab_click() {
    // Resize handle should take priority when overlapping with tab bar
    let mut dock = DockTree::new(
        DockNode::split(Axis::Horizontal, 0.5,
            DockNode::Leaf { tabs: vec![0, 1], active: 0 },
            DockNode::leaf(vec![2]),
        ),
        vec!["A", "B", "C"],
    );
    dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    // Click right at the boundary (x=200) within tab bar height (y=10)
    dock.update(200.0, 10.0, true);
    // If resize took priority, resizing should be Some
    // Drag to verify ratio changes
    dock.update(180.0, 10.0, true);
    dock.update(180.0, 10.0, false);
    let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    let left = areas.iter().find(|(id, _)| *id == 0 || *id == 1).unwrap();
    assert!((left.1.w - 180.0).abs() < 5.0, "resize should have priority, w={}", left.1.w);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p voltex_editor --lib dock::tests::test_tab_click -- --nocapture`
Expected: FAIL (update doesn't exist)

- [ ] **Step 3: Implement update method**

Key design: collect mutation target into a local variable, then apply outside the loop to avoid borrow conflicts.

```rust
/// Possible mutation from update
enum UpdateAction {
    None,
    StartResize(ResizeState),
    SetActiveTab { leaf_index: usize, new_active: usize },
}

impl DockTree {
    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, mouse_down: bool) {
        let just_clicked = mouse_down && !self.prev_mouse_down;
        self.prev_mouse_down = mouse_down;

        // Active resize in progress
        if let Some(ref state) = self.resizing {
            if mouse_down {
                let new_ratio = match state.axis {
                    Axis::Horizontal => (mouse_x - state.origin) / state.size,
                    Axis::Vertical => (mouse_y - state.origin) / state.size,
                };
                let clamped = new_ratio.clamp(MIN_RATIO, MAX_RATIO);
                let path = state.path.clone();
                Self::set_ratio_at_path(&mut self.root, &path, clamped);
            } else {
                self.resizing = None;
            }
            return;
        }

        if !just_clicked { return; }

        // Determine action by reading cached layouts (immutable)
        let action = self.find_click_action(mouse_x, mouse_y);

        // Apply action (mutable)
        match action {
            UpdateAction::StartResize(state) => { self.resizing = Some(state); }
            UpdateAction::SetActiveTab { leaf_index, new_active } => {
                Self::set_active_nth(&mut self.root, leaf_index, new_active, &mut 0);
            }
            UpdateAction::None => {}
        }
    }

    fn find_click_action(&self, mx: f32, my: f32) -> UpdateAction {
        // Check resize handles first (priority)
        for split in &self.cached_splits {
            let hit = match split.axis {
                Axis::Horizontal => mx >= split.boundary - RESIZE_HANDLE_HALF && mx <= split.boundary + RESIZE_HANDLE_HALF
                    && my >= split.rect.y && my < split.rect.y + split.rect.h,
                Axis::Vertical => my >= split.boundary - RESIZE_HANDLE_HALF && my <= split.boundary + RESIZE_HANDLE_HALF
                    && mx >= split.rect.x && mx < split.rect.x + split.rect.w,
            };
            if hit {
                let (origin, size) = match split.axis {
                    Axis::Horizontal => (split.rect.x, split.rect.w),
                    Axis::Vertical => (split.rect.y, split.rect.h),
                };
                return UpdateAction::StartResize(ResizeState { path: split.path.clone(), axis: split.axis, origin, size });
            }
        }

        // Check tab bar clicks
        for leaf in &self.cached_leaves {
            if leaf.tabs.len() <= 1 { continue; }
            if !leaf.tab_bar_rect.contains(mx, my) { continue; }
            let mut tx = leaf.tab_bar_rect.x;
            for (i, &panel_id) in leaf.tabs.iter().enumerate() {
                let name = self.names.get(panel_id as usize).map(|n| n.len()).unwrap_or(1);
                let tab_w = name as f32 * GLYPH_W + TAB_PADDING;
                if mx >= tx && mx < tx + tab_w {
                    return UpdateAction::SetActiveTab { leaf_index: leaf.leaf_index, new_active: i };
                }
                tx += tab_w;
            }
        }

        UpdateAction::None
    }

    fn set_ratio_at_path(node: &mut DockNode, path: &[usize], ratio: f32) {
        if path.is_empty() {
            if let DockNode::Split { ratio: r, .. } = node { *r = ratio; }
            return;
        }
        if let DockNode::Split { children, .. } = node {
            Self::set_ratio_at_path(&mut children[path[0]], &path[1..], ratio);
        }
    }

    fn set_active_nth(node: &mut DockNode, target: usize, new_active: usize, count: &mut usize) {
        match node {
            DockNode::Leaf { active, .. } => {
                if *count == target { *active = new_active; }
                *count += 1;
            }
            DockNode::Split { children, .. } => {
                Self::set_active_nth(&mut children[0], target, new_active, count);
                Self::set_active_nth(&mut children[1], target, new_active, count);
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p voltex_editor --lib dock -- --nocapture`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_editor/src/dock.rs
git commit -m "feat(editor): add update method with tab click and resize handling"
```

---

### Task 3: draw_chrome — tab bars and split lines

**Files:**
- Modify: `crates/voltex_editor/src/dock.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn test_draw_chrome_produces_draw_commands() {
    let mut dock = DockTree::new(
        DockNode::split(Axis::Horizontal, 0.5,
            DockNode::Leaf { tabs: vec![0, 1], active: 0 },
            DockNode::leaf(vec![2]),
        ),
        vec!["A", "B", "C"],
    );
    let mut ui = UiContext::new(800.0, 600.0);
    ui.begin_frame(0.0, 0.0, false);
    dock.layout(Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 });
    dock.draw_chrome(&mut ui);
    assert!(ui.draw_list.commands.len() >= 5);
}

#[test]
fn test_draw_chrome_active_tab_color() {
    let mut dock = DockTree::new(
        DockNode::Leaf { tabs: vec![0, 1], active: 1 },
        vec!["AA", "BB"],
    );
    let mut ui = UiContext::new(800.0, 600.0);
    ui.begin_frame(0.0, 0.0, false);
    dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
    dock.draw_chrome(&mut ui);
    // First tab bg (inactive): color [40, 40, 40, 255]
    let first_bg = &ui.draw_list.vertices[0];
    assert_eq!(first_bg.color, [40, 40, 40, 255]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p voltex_editor --lib dock::tests::test_draw_chrome -- --nocapture`
Expected: FAIL (draw_chrome doesn't exist)

- [ ] **Step 3: Implement draw_chrome**

Inline glyph rendering to avoid `&mut ui.draw_list` + `&ui.font` borrow conflict (same pattern as existing `widgets.rs`):

```rust
impl DockTree {
    pub fn draw_chrome(&self, ui: &mut UiContext) {
        let glyph_w = ui.font.glyph_width as f32;
        let glyph_h = ui.font.glyph_height as f32;

        const COLOR_TAB_ACTIVE: [u8; 4] = [60, 60, 60, 255];
        const COLOR_TAB_INACTIVE: [u8; 4] = [40, 40, 40, 255];
        const COLOR_TEXT: [u8; 4] = [0xEE, 0xEE, 0xEE, 0xFF];
        const COLOR_SPLIT: [u8; 4] = [30, 30, 30, 255];
        const COLOR_SPLIT_ACTIVE: [u8; 4] = [100, 100, 200, 255];
        const COLOR_SEPARATOR: [u8; 4] = [50, 50, 50, 255];

        // Draw tab bars
        for leaf in &self.cached_leaves {
            let bar = &leaf.tab_bar_rect;
            let active = leaf.active.min(leaf.tabs.len().saturating_sub(1));
            let mut tx = bar.x;
            for (i, &panel_id) in leaf.tabs.iter().enumerate() {
                let name = self.names.get(panel_id as usize).copied().unwrap_or("?");
                let tab_w = name.len() as f32 * glyph_w + TAB_PADDING;
                let bg = if i == active { COLOR_TAB_ACTIVE } else { COLOR_TAB_INACTIVE };
                ui.draw_list.add_rect(tx, bar.y, tab_w, bar.h, bg);
                // Inline text rendering
                let text_x = tx + TAB_PADDING * 0.5;
                let text_y = bar.y + (bar.h - glyph_h) * 0.5;
                let mut cx = text_x;
                for ch in name.chars() {
                    let (u0, v0, u1, v1) = ui.font.glyph_uv(ch);
                    ui.draw_list.add_rect_uv(cx, text_y, glyph_w, glyph_h, u0, v0, u1, v1, COLOR_TEXT);
                    cx += glyph_w;
                }
                tx += tab_w;
            }
            // Tab bar separator
            ui.draw_list.add_rect(bar.x, bar.y + bar.h - 1.0, bar.w, 1.0, COLOR_SEPARATOR);
        }

        // Draw split lines
        for split in &self.cached_splits {
            let color = if self.resizing.as_ref().map(|r| &r.path) == Some(&split.path) {
                COLOR_SPLIT_ACTIVE
            } else {
                COLOR_SPLIT
            };
            match split.axis {
                Axis::Horizontal => ui.draw_list.add_rect(split.boundary - 0.5, split.rect.y, 1.0, split.rect.h, color),
                Axis::Vertical => ui.draw_list.add_rect(split.rect.x, split.boundary - 0.5, split.rect.w, 1.0, color),
            }
        }
    }
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p voltex_editor --lib dock -- --nocapture`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_editor/src/dock.rs
git commit -m "feat(editor): add draw_chrome for tab bars and split lines"
```

---

### Task 4: Integration test + update editor_demo

**Files:**
- Modify: `crates/voltex_editor/src/dock.rs`
- Modify: `examples/editor_demo/src/main.rs`

- [ ] **Step 1: Write integration test**

```rust
#[test]
fn test_full_frame_cycle() {
    let mut dock = DockTree::new(
        DockNode::split(Axis::Horizontal, 0.3,
            DockNode::Leaf { tabs: vec![0, 1], active: 0 },
            DockNode::split(Axis::Vertical, 0.6, DockNode::leaf(vec![2]), DockNode::leaf(vec![3])),
        ),
        vec!["Hierarchy", "Inspector", "Viewport", "Console"],
    );
    let mut ui = UiContext::new(1280.0, 720.0);
    for _ in 0..3 {
        ui.begin_frame(100.0, 100.0, false);
        let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 1280.0, h: 720.0 });
        dock.update(100.0, 100.0, false);
        dock.draw_chrome(&mut ui);
        assert_eq!(areas.len(), 4);
        for (_, r) in &areas {
            assert!(r.w > 0.0);
            assert!(r.h > 0.0);
        }
        ui.end_frame();
    }
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p voltex_editor --lib dock::tests::test_full_frame -- --nocapture`
Expected: PASS

- [ ] **Step 3: Update editor_demo to use docking**

Read `examples/editor_demo/src/main.rs`. Replace the single `begin_panel`/`end_panel` block with a docked layout:

- Add `dock: DockTree` to the app state struct
- Initialize with 4-panel layout (Debug, Viewport, Properties, Console)
- In render loop: call `dock.layout()`, `dock.update()`, `dock.draw_chrome()`
- Draw panel contents by matching `panel_id` and setting `ui.layout = LayoutState::new(rect.x + 4.0, rect.y + 4.0)`

- [ ] **Step 4: Build**

Run: `cargo build --example editor_demo`
Expected: compiles

- [ ] **Step 5: Run all tests**

Run: `cargo test -p voltex_editor -- --nocapture`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/voltex_editor/src/dock.rs examples/editor_demo/src/main.rs
git commit -m "feat(editor): integrate docking into editor_demo"
```

---

### Task 5: Update docs

**Files:**
- Modify: `docs/STATUS.md`
- Modify: `docs/DEFERRED.md`

- [ ] **Step 1: Update STATUS.md**

Add to Phase 8-4 section:
```
- voltex_editor: DockTree (binary split layout, resize, tabs)
```

Update test count.

- [ ] **Step 2: Update DEFERRED.md**

```
- ~~**도킹, 탭, 윈도우 드래그**~~ ✅ DockTree (이진 분할, 리사이즈, 탭 전환) 완료. 플로팅 윈도우/탭 드래그 이동 미구현.
```

- [ ] **Step 3: Commit**

```bash
git add docs/STATUS.md docs/DEFERRED.md
git commit -m "docs: update STATUS.md and DEFERRED.md with docking system"
```
