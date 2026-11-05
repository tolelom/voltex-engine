//! Floating panel — a panel torn out of the dock tree, draggable + resizable.
//!
//! Panel state fields are `pub` for tests; we allow `missing_docs` and
//! document only the entry points ([`FloatingPanel`], [`FloatingPanelManager`]).
#![allow(missing_docs)]

use crate::dock::Rect;

/// A panel that has been "undocked" and floats freely.
#[derive(Debug, Clone)]
pub struct FloatingPanel {
    pub panel_id: u32,
    pub title: String,
    pub rect: Rect,
    pub visible: bool,
}

impl FloatingPanel {
    pub fn new(panel_id: u32, title: &str, x: f32, y: f32, w: f32, h: f32) -> Self {
        FloatingPanel {
            panel_id,
            title: title.to_string(),
            rect: Rect { x, y, w, h },
            visible: true,
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.rect.x = x;
        self.rect.y = y;
    }

    pub fn set_size(&mut self, w: f32, h: f32) {
        self.rect.w = w.max(100.0);
        self.rect.h = h.max(50.0);
    }
}

/// Manager for floating panels.
pub struct FloatingPanelManager {
    panels: Vec<FloatingPanel>,
}

impl Default for FloatingPanelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FloatingPanelManager {
    pub fn new() -> Self {
        FloatingPanelManager {
            panels: Vec::new(),
        }
    }

    pub fn add(&mut self, panel: FloatingPanel) {
        self.panels.push(panel);
    }

    pub fn remove(&mut self, panel_id: u32) {
        self.panels.retain(|p| p.panel_id != panel_id);
    }

    pub fn get(&self, panel_id: u32) -> Option<&FloatingPanel> {
        self.panels.iter().find(|p| p.panel_id == panel_id)
    }

    pub fn get_mut(&mut self, panel_id: u32) -> Option<&mut FloatingPanel> {
        self.panels.iter_mut().find(|p| p.panel_id == panel_id)
    }

    pub fn visible_panels(&self) -> Vec<&FloatingPanel> {
        self.panels.iter().filter(|p| p.visible).collect()
    }

    pub fn len(&self) -> usize {
        self.panels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_panel() {
        let p = FloatingPanel::new(1, "Test", 100.0, 100.0, 300.0, 200.0);
        assert_eq!(p.panel_id, 1);
        assert!(p.visible);
    }

    #[test]
    fn test_manager_add_remove() {
        let mut mgr = FloatingPanelManager::new();
        mgr.add(FloatingPanel::new(1, "A", 0.0, 0.0, 200.0, 100.0));
        mgr.add(FloatingPanel::new(2, "B", 0.0, 0.0, 200.0, 100.0));
        assert_eq!(mgr.len(), 2);
        mgr.remove(1);
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_visible_panels() {
        let mut mgr = FloatingPanelManager::new();
        mgr.add(FloatingPanel::new(1, "A", 0.0, 0.0, 200.0, 100.0));
        let mut p2 = FloatingPanel::new(2, "B", 0.0, 0.0, 200.0, 100.0);
        p2.visible = false;
        mgr.add(p2);
        assert_eq!(mgr.visible_panels().len(), 1);
    }

    #[test]
    fn test_min_size() {
        let mut p = FloatingPanel::new(1, "T", 0.0, 0.0, 300.0, 200.0);
        p.set_size(10.0, 10.0);
        assert!(p.rect.w >= 100.0);
        assert!(p.rect.h >= 50.0);
    }
}
