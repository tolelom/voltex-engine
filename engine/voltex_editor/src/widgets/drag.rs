//! Drag-and-drop on `UiContext`: `begin_drag` starts carrying a `u64` payload,
//! `drop_target` returns it when released over a region, `end_drag` finishes the drag.

use crate::ui_context::UiContext;

const DRAG_THRESHOLD: f32 = 5.0;

impl UiContext {
    // ── Drag and Drop ─────────────────────────────────────────────────

    /// Begin dragging an item. Call this when the user presses down on a draggable element.
    /// `id` identifies the source, `payload` is an arbitrary u64 value transferred on drop.
    pub fn begin_drag(&mut self, id: u32, payload: u64) {
        if self.mouse_clicked {
            self.dragging = Some((id, payload));
            self.drag_start = (self.mouse_x, self.mouse_y);
            self.drag_started = false;
        }
    }

    /// Returns true if a drag operation is currently in progress (past the threshold).
    pub fn is_dragging(&self) -> bool {
        if self.dragging.is_some() {
            self.drag_started
        } else {
            false
        }
    }

    /// End the current drag operation. Returns `Some((source_id, payload))` if a drag
    /// was in progress and the mouse was released, otherwise `None`.
    pub fn end_drag(&mut self) -> Option<(u32, u64)> {
        // Update drag started state based on threshold
        if self.dragging.is_some() && !self.drag_started {
            let dx = self.mouse_x - self.drag_start.0;
            let dy = self.mouse_y - self.drag_start.1;
            if (dx * dx + dy * dy).sqrt() >= DRAG_THRESHOLD {
                self.drag_started = true;
            }
        }

        if self.mouse_released {
            let result = if self.drag_started { self.dragging } else { None };
            self.dragging = None;
            self.drag_started = false;
            result
        } else {
            None
        }
    }

    /// Declare a drop target region. If a drag is released over this target,
    /// returns the payload that was dropped. Otherwise returns `None`.
    pub fn drop_target(&mut self, _id: u32, x: f32, y: f32, w: f32, h: f32) -> Option<u64> {
        if self.mouse_released && self.drag_started
            && self.mouse_in_rect(x, y, w, h) {
                if let Some((_src_id, payload)) = self.dragging {
                    return Some(payload);
                }
            }
        None
    }
}
