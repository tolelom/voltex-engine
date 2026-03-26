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
}
