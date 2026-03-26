const TAB_BAR_HEIGHT: f32 = 20.0;
const MIN_RATIO: f32 = 0.1;
const MAX_RATIO: f32 = 0.9;
const RESIZE_HANDLE_HALF: f32 = 3.0;
const GLYPH_W: f32 = 8.0;
const TAB_PADDING: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

pub enum DockNode {
    Leaf { tabs: Vec<u32>, active: usize },
    Split { axis: Axis, ratio: f32, children: [Box<DockNode>; 2] },
}

impl DockNode {
    pub fn leaf(tabs: Vec<u32>) -> Self {
        DockNode::Leaf { tabs, active: 0 }
    }

    pub fn split(axis: Axis, ratio: f32, a: DockNode, b: DockNode) -> Self {
        DockNode::Split {
            axis,
            ratio: ratio.clamp(MIN_RATIO, MAX_RATIO),
            children: [Box::new(a), Box::new(b)],
        }
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

enum UpdateAction {
    None,
    StartResize(ResizeState),
    SetActiveTab { leaf_index: usize, new_active: usize },
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
            assert!(!tabs.is_empty(), "DockNode::Leaf must have at least one tab");
            let idx = *leaf_counter;
            *leaf_counter += 1;
            leaves.push(LeafLayout {
                leaf_index: idx,
                tabs: tabs.clone(),
                active: (*active).min(tabs.len().saturating_sub(1)),
                tab_bar_rect: Rect { x: rect.x, y: rect.y, w: rect.w, h: TAB_BAR_HEIGHT },
                content_rect: Rect {
                    x: rect.x,
                    y: rect.y + TAB_BAR_HEIGHT,
                    w: rect.w,
                    h: (rect.h - TAB_BAR_HEIGHT).max(0.0),
                },
            });
        }
        DockNode::Split { axis, ratio, children } => {
            let (r1, r2, boundary) = match axis {
                Axis::Horizontal => {
                    let w1 = rect.w * ratio;
                    let b = rect.x + w1;
                    (
                        Rect { x: rect.x, y: rect.y, w: w1, h: rect.h },
                        Rect { x: b, y: rect.y, w: rect.w - w1, h: rect.h },
                        b,
                    )
                }
                Axis::Vertical => {
                    let h1 = rect.h * ratio;
                    let b = rect.y + h1;
                    (
                        Rect { x: rect.x, y: rect.y, w: rect.w, h: h1 },
                        Rect { x: rect.x, y: b, w: rect.w, h: rect.h - h1 },
                        b,
                    )
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
        DockTree {
            root,
            names,
            cached_leaves: Vec::new(),
            cached_splits: Vec::new(),
            resizing: None,
            prev_mouse_down: false,
        }
    }

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

        let action = self.find_click_action(mouse_x, mouse_y);

        match action {
            UpdateAction::StartResize(state) => { self.resizing = Some(state); }
            UpdateAction::SetActiveTab { leaf_index, new_active } => {
                Self::set_active_nth(&mut self.root, leaf_index, new_active, &mut 0);
            }
            UpdateAction::None => {}
        }
    }

    fn find_click_action(&self, mx: f32, my: f32) -> UpdateAction {
        // Check resize handles first (priority over tab clicks)
        for split in &self.cached_splits {
            let hit = match split.axis {
                Axis::Horizontal => {
                    mx >= split.boundary - RESIZE_HANDLE_HALF
                        && mx <= split.boundary + RESIZE_HANDLE_HALF
                        && my >= split.rect.y
                        && my < split.rect.y + split.rect.h
                }
                Axis::Vertical => {
                    my >= split.boundary - RESIZE_HANDLE_HALF
                        && my <= split.boundary + RESIZE_HANDLE_HALF
                        && mx >= split.rect.x
                        && mx < split.rect.x + split.rect.w
                }
            };
            if hit {
                let (origin, size) = match split.axis {
                    Axis::Horizontal => (split.rect.x, split.rect.w),
                    Axis::Vertical => (split.rect.y, split.rect.h),
                };
                return UpdateAction::StartResize(ResizeState {
                    path: split.path.clone(),
                    axis: split.axis,
                    origin,
                    size,
                });
            }
        }

        // Check tab bar clicks
        for leaf in &self.cached_leaves {
            if leaf.tabs.len() <= 1 { continue; }
            if !leaf.tab_bar_rect.contains(mx, my) { continue; }
            let mut tx = leaf.tab_bar_rect.x;
            for (i, &panel_id) in leaf.tabs.iter().enumerate() {
                let name_len = self.names.get(panel_id as usize).map(|n| n.len()).unwrap_or(1);
                let tab_w = name_len as f32 * GLYPH_W + TAB_PADDING;
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

    pub fn layout(&mut self, rect: Rect) -> Vec<(u32, Rect)> {
        self.cached_leaves.clear();
        self.cached_splits.clear();
        let mut path = Vec::new();
        let mut counter = 0;
        layout_recursive(
            &self.root,
            rect,
            &mut path,
            &mut counter,
            &mut self.cached_leaves,
            &mut self.cached_splits,
        );
        self.cached_leaves
            .iter()
            .map(|l| {
                let active = l.active.min(l.tabs.len().saturating_sub(1));
                (l.tabs[active], l.content_rect)
            })
            .collect()
    }
}

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
        let mut dock = DockTree::new(DockNode::leaf(vec![0]), vec!["Panel0"]);
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
        assert!((top.1.h - 130.0).abs() < 1e-3);
    }

    #[test]
    fn test_layout_nested_split() {
        let mut dock = DockTree::new(
            DockNode::split(
                Axis::Horizontal,
                0.25,
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

    #[test]
    fn test_tab_click_switches_active() {
        let mut dock = DockTree::new(
            DockNode::Leaf { tabs: vec![0, 1, 2], active: 0 },
            vec!["A", "B", "C"],
        );
        dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
        // Tab "A": w = 1*8+8 = 16, tab "B" starts at x=16. Click at x=20 (inside "B").
        dock.update(20.0, 10.0, true);
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
        dock.update(200.0, 150.0, true);  // click on boundary at x=200
        dock.update(120.0, 150.0, true);  // drag to x=120
        dock.update(120.0, 150.0, false); // release
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
        assert!((left.1.w - 40.0).abs() < 1e-3, "left w={}", left.1.w);
    }

    #[test]
    fn test_resize_priority_over_tab_click() {
        let mut dock = DockTree::new(
            DockNode::split(Axis::Horizontal, 0.5,
                DockNode::Leaf { tabs: vec![0, 1], active: 0 },
                DockNode::leaf(vec![2]),
            ),
            vec!["A", "B", "C"],
        );
        dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
        dock.update(200.0, 10.0, true);   // click at boundary within tab bar
        dock.update(180.0, 10.0, true);   // drag
        dock.update(180.0, 10.0, false);  // release
        let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 });
        let left = areas.iter().find(|(id, _)| *id == 0 || *id == 1).unwrap();
        assert!((left.1.w - 180.0).abs() < 5.0, "resize should have priority, w={}", left.1.w);
    }
}
