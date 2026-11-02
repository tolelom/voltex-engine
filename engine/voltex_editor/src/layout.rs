//! Immediate-mode cursor-based layout — tracks the next widget origin
//! within a panel.
#![allow(missing_docs)]

/// Simple cursor-based layout state for immediate mode UI.
pub struct LayoutState {
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub indent: f32,
    pub line_height: f32,
    pub padding: f32,
    /// Y origin of the most recently placed widget (for `same_line`).
    pub prev_y: f32,
    /// X just past the most recently placed widget, including padding
    /// (where `same_line` puts the next widget).
    pub prev_end_x: f32,
}

impl LayoutState {
    /// Create a new layout state starting at (x, y).
    pub fn new(x: f32, y: f32) -> Self {
        LayoutState {
            cursor_x: x,
            cursor_y: y,
            indent: x,
            line_height: 12.0,
            padding: 4.0,
            prev_y: y,
            prev_end_x: x,
        }
    }

    /// Record the rect a widget just occupied so a following [`same_line`]
    /// (on `UiContext`) can place the next widget to its right. Call this
    /// immediately before [`advance_line`](Self::advance_line).
    pub fn record(&mut self, x: f32, y: f32, width: f32) {
        self.prev_y = y;
        self.prev_end_x = x + width + self.padding;
    }

    /// Advance to the next line: cursor_y += line_height + padding, cursor_x = indent.
    pub fn advance_line(&mut self) {
        self.cursor_y += self.line_height + self.padding;
        self.cursor_x = self.indent;
    }

    /// Advance horizontally: cursor_x += width + padding.
    pub fn advance_x(&mut self, width: f32) {
        self.cursor_x += width + self.padding;
    }
}
