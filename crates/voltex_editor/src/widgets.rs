use crate::ui_context::{Key, UiContext};

// Color palette
const COLOR_BG: [u8; 4]             = [0x2B, 0x2B, 0x2B, 0xFF];
const COLOR_BUTTON: [u8; 4]         = [0x44, 0x44, 0x55, 0xFF];
const COLOR_BUTTON_HOT: [u8; 4]     = [0x55, 0x66, 0x88, 0xFF];
const COLOR_BUTTON_ACTIVE: [u8; 4]  = [0x44, 0x88, 0xFF, 0xFF];
const COLOR_TEXT: [u8; 4]           = [0xEE, 0xEE, 0xEE, 0xFF];
const COLOR_PANEL: [u8; 4]          = [0x33, 0x33, 0x33, 0xFF];
const COLOR_SLIDER_BG: [u8; 4]      = [0x44, 0x44, 0x44, 0xFF];
const COLOR_SLIDER_HANDLE: [u8; 4]  = [0x88, 0x88, 0xFF, 0xFF];
const COLOR_CHECK_BG: [u8; 4]       = [0x44, 0x44, 0x44, 0xFF];
const COLOR_CHECK_MARK: [u8; 4]     = [0x88, 0xFF, 0x88, 0xFF];
const COLOR_INPUT_BG: [u8; 4]       = [0x22, 0x22, 0x22, 0xFF];
const COLOR_INPUT_BORDER: [u8; 4]   = [0x66, 0x66, 0x66, 0xFF];
const COLOR_INPUT_FOCUSED: [u8; 4]  = [0x44, 0x88, 0xFF, 0xFF];
const COLOR_CURSOR: [u8; 4]         = [0xFF, 0xFF, 0xFF, 0xFF];
const COLOR_SCROLLBAR_BG: [u8; 4]   = [0x33, 0x33, 0x33, 0xFF];
const COLOR_SCROLLBAR_THUMB: [u8; 4]= [0x66, 0x66, 0x77, 0xFF];
const DRAG_THRESHOLD: f32           = 5.0;

impl UiContext {
    /// Draw text at the current cursor position and advance to the next line.
    pub fn text(&mut self, text: &str) {
        let x = self.layout.cursor_x;
        let y = self.layout.cursor_y;
        // We need to clone font info for the borrow checker
        let gw = self.font.glyph_width as f32;
        let gh = self.font.glyph_height as f32;

        // Draw each character
        let mut cx = x;
        for ch in text.chars() {
            let (u0, v0, u1, v1) = self.font.glyph_uv(ch);
            self.draw_list.add_rect_uv(cx, y, gw, gh, u0, v0, u1, v1, COLOR_TEXT);
            cx += gw;
        }

        self.layout.advance_line();
    }

    /// Draw a button with the given label. Returns true if clicked this frame.
    pub fn button(&mut self, label: &str) -> bool {
        let id = self.gen_id();
        let gw = self.font.glyph_width as f32;
        let gh = self.font.glyph_height as f32;
        let padding = self.layout.padding;

        let text_w = label.len() as f32 * gw;
        let btn_w = text_w + padding * 2.0;
        let btn_h = gh + padding * 2.0;

        let x = self.layout.cursor_x;
        let y = self.layout.cursor_y;

        let hovered = self.mouse_in_rect(x, y, btn_w, btn_h);

        if hovered {
            self.hot = Some(id);
            if self.mouse_down {
                self.active = Some(id);
            }
        }

        // Determine color
        let bg_color = if self.active == Some(id) && hovered {
            COLOR_BUTTON_ACTIVE
        } else if self.hot == Some(id) {
            COLOR_BUTTON_HOT
        } else {
            COLOR_BUTTON
        };

        // Draw background rect
        self.draw_list.add_rect(x, y, btn_w, btn_h, bg_color);

        // Draw text centered inside button
        let text_x = x + padding;
        let text_y = y + padding;
        let mut cx = text_x;
        for ch in label.chars() {
            let (u0, v0, u1, v1) = self.font.glyph_uv(ch);
            self.draw_list.add_rect_uv(cx, text_y, gw, gh, u0, v0, u1, v1, COLOR_TEXT);
            cx += gw;
        }

        self.layout.advance_line();

        // Return true if mouse was released over this button while it was active
        let clicked = hovered && self.mouse_released && self.active == Some(id);
        if self.mouse_released {
            if self.active == Some(id) {
                self.active = None;
            }
        }
        clicked
    }

    /// Draw a horizontal slider. Returns the (possibly new) value after interaction.
    pub fn slider(&mut self, label: &str, value: f32, min: f32, max: f32) -> f32 {
        let id = self.gen_id();
        let gw = self.font.glyph_width as f32;
        let gh = self.font.glyph_height as f32;
        let padding = self.layout.padding;

        let slider_w = 150.0_f32;
        let slider_h = gh + padding * 2.0;
        let handle_w = 10.0_f32;

        let x = self.layout.cursor_x;
        let y = self.layout.cursor_y;

        let hovered = self.mouse_in_rect(x, y, slider_w, slider_h);

        if hovered && self.mouse_clicked {
            self.active = Some(id);
        }

        let mut new_value = value;

        if self.active == Some(id) {
            if self.mouse_down {
                // Map mouse_x to value
                let t = ((self.mouse_x - x - handle_w / 2.0) / (slider_w - handle_w)).clamp(0.0, 1.0);
                new_value = min + t * (max - min);
            } else if self.mouse_released {
                self.active = None;
            }
        }

        // Clamp value
        new_value = new_value.clamp(min, max);

        // Draw background bar
        self.draw_list.add_rect(x, y, slider_w, slider_h, COLOR_SLIDER_BG);

        // Draw handle
        let t = if (max - min).abs() < 1e-6 {
            0.0
        } else {
            (new_value - min) / (max - min)
        };
        let handle_x = x + t * (slider_w - handle_w);
        self.draw_list.add_rect(handle_x, y, handle_w, slider_h, COLOR_SLIDER_HANDLE);

        // Draw label to the right of the slider
        let label_x = x + slider_w + padding;
        let label_y = y + padding;
        let mut cx = label_x;
        for ch in label.chars() {
            let (u0, v0, u1, v1) = self.font.glyph_uv(ch);
            self.draw_list.add_rect_uv(cx, label_y, gw, gh, u0, v0, u1, v1, COLOR_TEXT);
            cx += gw;
        }

        self.layout.advance_line();

        new_value
    }

    /// Draw a checkbox. Returns the new checked state (toggled on click).
    pub fn checkbox(&mut self, label: &str, checked: bool) -> bool {
        let id = self.gen_id();
        let gw = self.font.glyph_width as f32;
        let gh = self.font.glyph_height as f32;
        let padding = self.layout.padding;

        let box_size = gh;
        let x = self.layout.cursor_x;
        let y = self.layout.cursor_y;

        let hovered = self.mouse_in_rect(x, y, box_size + padding + label.len() as f32 * gw, gh + padding);

        if hovered {
            self.hot = Some(id);
        }

        let mut new_checked = checked;
        if hovered && self.mouse_clicked {
            new_checked = !checked;
        }

        // Draw checkbox background
        self.draw_list.add_rect(x, y, box_size, box_size, COLOR_CHECK_BG);

        // Draw check mark if checked
        if new_checked {
            let inner = 3.0;
            self.draw_list.add_rect(
                x + inner,
                y + inner,
                box_size - inner * 2.0,
                box_size - inner * 2.0,
                COLOR_CHECK_MARK,
            );
        }

        // Draw label
        let label_x = x + box_size + padding;
        let label_y = y;
        let mut cx = label_x;
        for ch in label.chars() {
            let (u0, v0, u1, v1) = self.font.glyph_uv(ch);
            self.draw_list.add_rect_uv(cx, label_y, gw, gh, u0, v0, u1, v1, COLOR_TEXT);
            cx += gw;
        }

        self.layout.advance_line();

        new_checked
    }

    /// Begin a panel: draw background and title, set cursor inside panel.
    pub fn begin_panel(&mut self, title: &str, x: f32, y: f32, w: f32, h: f32) {
        let gh = self.font.glyph_height as f32;
        let padding = self.layout.padding;

        // Draw panel background
        self.draw_list.add_rect(x, y, w, h, COLOR_PANEL);

        // Draw title bar (slightly darker background handled by same panel color here)
        let title_bar_h = gh + padding * 2.0;
        self.draw_list.add_rect(x, y, w, title_bar_h, COLOR_BG);

        // Draw title text
        let gw = self.font.glyph_width as f32;
        let mut cx = x + padding;
        let ty = y + padding;
        for ch in title.chars() {
            let (u0, v0, u1, v1) = self.font.glyph_uv(ch);
            self.draw_list.add_rect_uv(cx, ty, gw, gh, u0, v0, u1, v1, COLOR_TEXT);
            cx += gw;
        }

        // Set cursor to inside the panel (below title bar)
        self.layout = crate::layout::LayoutState::new(x + padding, y + title_bar_h + padding);
    }

    /// End a panel — currently a no-op; cursor remains where it was.
    pub fn end_panel(&mut self) {
        // Nothing for now; future could restore outer cursor state.
    }

    // ── Text Input Widget ─────────────────────────────────────────────

    /// Draw an editable single-line text input. Returns true if the buffer changed.
    ///
    /// `id` must be unique per text input. The widget renders a box at (x, y) with
    /// the given `width`. Height is determined by the font glyph height + padding.
    pub fn text_input(&mut self, id: u32, buffer: &mut String, x: f32, y: f32, width: f32) -> bool {
        let gw = self.font.glyph_width as f32;
        let gh = self.font.glyph_height as f32;
        let padding = self.layout.padding;
        let height = gh + padding * 2.0;

        let hovered = self.mouse_in_rect(x, y, width, height);

        // Click to focus / unfocus
        if self.mouse_clicked {
            if hovered {
                self.focused_id = Some(id);
                // Place cursor at end or at click position
                let click_offset = ((self.mouse_x - x - padding) / gw).round() as usize;
                self.cursor_pos = click_offset.min(buffer.len());
            } else if self.focused_id == Some(id) {
                self.focused_id = None;
            }
        }

        let mut changed = false;

        // Process input only if focused
        if self.focused_id == Some(id) {
            // Ensure cursor_pos is valid
            if self.cursor_pos > buffer.len() {
                self.cursor_pos = buffer.len();
            }

            // Process character input
            let chars = self.drain_chars();
            for ch in chars {
                buffer.insert(self.cursor_pos, ch);
                self.cursor_pos += 1;
                changed = true;
            }

            // Process key input
            let keys = self.drain_keys();
            for key in keys {
                match key {
                    Key::Backspace => {
                        if self.cursor_pos > 0 {
                            buffer.remove(self.cursor_pos - 1);
                            self.cursor_pos -= 1;
                            changed = true;
                        }
                    }
                    Key::Delete => {
                        if self.cursor_pos < buffer.len() {
                            buffer.remove(self.cursor_pos);
                            changed = true;
                        }
                    }
                    Key::Left => {
                        if self.cursor_pos > 0 {
                            self.cursor_pos -= 1;
                        }
                    }
                    Key::Right => {
                        if self.cursor_pos < buffer.len() {
                            self.cursor_pos += 1;
                        }
                    }
                    Key::Home => {
                        self.cursor_pos = 0;
                    }
                    Key::End => {
                        self.cursor_pos = buffer.len();
                    }
                }
            }
        }

        // Draw border
        let border_color = if self.focused_id == Some(id) {
            COLOR_INPUT_FOCUSED
        } else {
            COLOR_INPUT_BORDER
        };
        self.draw_list.add_rect(x, y, width, height, border_color);
        // Draw inner background (1px border)
        self.draw_list.add_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0, COLOR_INPUT_BG);

        // Draw text
        let text_x = x + padding;
        let text_y = y + padding;
        let mut cx = text_x;
        for ch in buffer.chars() {
            let (u0, v0, u1, v1) = self.font.glyph_uv(ch);
            self.draw_list.add_rect_uv(cx, text_y, gw, gh, u0, v0, u1, v1, COLOR_TEXT);
            cx += gw;
        }

        // Draw cursor if focused
        if self.focused_id == Some(id) {
            let cursor_x = text_x + self.cursor_pos as f32 * gw;
            self.draw_list.add_rect(cursor_x, text_y, 1.0, gh, COLOR_CURSOR);
        }

        self.layout.advance_line();
        changed
    }

    // ── Scroll Panel ──────────────────────────────────────────────────

    /// Begin a scrollable panel. Content drawn between begin/end will be clipped
    /// to the panel bounds. `content_height` is the total height of the content
    /// inside the panel (used to compute scrollbar size).
    pub fn begin_scroll_panel(&mut self, id: u32, x: f32, y: f32, w: f32, h: f32, content_height: f32) {
        let scrollbar_w = 12.0_f32;
        let panel_inner_w = w - scrollbar_w;

        // Handle mouse wheel when hovering over the panel
        let hovered = self.mouse_in_rect(x, y, w, h);
        let scroll_delta = if hovered && self.mouse_scroll.abs() > 0.0 {
            -self.mouse_scroll * 20.0
        } else {
            0.0
        };

        // Get or create scroll offset, apply delta and clamp
        let scroll = self.scroll_offsets.entry(id).or_insert(0.0);
        *scroll += scroll_delta;
        let max_scroll = (content_height - h).max(0.0);
        *scroll = scroll.clamp(0.0, max_scroll);
        let current_scroll = *scroll;

        // Draw panel background
        self.draw_list.add_rect(x, y, w, h, COLOR_PANEL);

        // Draw scrollbar track
        let sb_x = x + panel_inner_w;
        self.draw_list.add_rect(sb_x, y, scrollbar_w, h, COLOR_SCROLLBAR_BG);

        // Draw scrollbar thumb
        if content_height > h {
            let thumb_ratio = h / content_height;
            let thumb_h = (thumb_ratio * h).max(16.0);
            let scroll_ratio = if max_scroll > 0.0 { current_scroll / max_scroll } else { 0.0 };
            let thumb_y = y + scroll_ratio * (h - thumb_h);
            self.draw_list.add_rect(sb_x, thumb_y, scrollbar_w, thumb_h, COLOR_SCROLLBAR_THUMB);
        }

        // Push scissor rect for content clipping
        self.draw_list.push_scissor(x as u32, y as u32, panel_inner_w as u32, h as u32);

        // Set cursor inside panel, offset by scroll
        self.layout = crate::layout::LayoutState::new(x + self.layout.padding, y + self.layout.padding - current_scroll);
    }

    /// End a scrollable panel. Pops the scissor rect.
    pub fn end_scroll_panel(&mut self) {
        self.draw_list.pop_scissor();
    }

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
        if let Some(_) = self.dragging {
            self.drag_started
        } else {
            false
        }
    }

    /// End the current drag operation. Returns `Some((source_id, payload))` if a drag
    /// was in progress and the mouse was released, otherwise `None`.
    pub fn end_drag(&mut self) -> Option<(u32, u64)> {
        // Update drag started state based on threshold
        if let Some(_) = self.dragging {
            if !self.drag_started {
                let dx = self.mouse_x - self.drag_start.0;
                let dy = self.mouse_y - self.drag_start.1;
                if (dx * dx + dy * dy).sqrt() >= DRAG_THRESHOLD {
                    self.drag_started = true;
                }
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
        if self.mouse_released && self.drag_started {
            if self.mouse_in_rect(x, y, w, h) {
                if let Some((_src_id, payload)) = self.dragging {
                    return Some(payload);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::ui_context::{Key, UiContext};

    #[test]
    fn test_button_returns_false_when_not_clicked() {
        let mut ctx = UiContext::new(800.0, 600.0);
        // Mouse is at (500, 500) — far from any button
        ctx.begin_frame(500.0, 500.0, false);
        let result = ctx.button("Click Me");
        assert!(!result, "button should return false when mouse is not over it");
    }

    #[test]
    fn test_button_returns_true_when_clicked() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Frame 1: mouse over button, pressed down
        // Button will be at layout cursor (0, 0) with some width/height
        // glyph_width=8, "OK"=2 chars, btn_w = 2*8 + 4*2 = 24, btn_h = 12 + 4*2 = 20
        ctx.begin_frame(10.0, 5.0, true);
        let _ = ctx.button("OK");

        // Frame 2: mouse still over button, released
        ctx.begin_frame(10.0, 5.0, false);
        let result = ctx.button("OK");
        assert!(result, "button should return true when clicked and released");
    }

    #[test]
    fn test_slider_returns_clamped_value() {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.begin_frame(0.0, 0.0, false);

        // Value above max should be clamped
        let v = ctx.slider("test", 150.0, 0.0, 100.0);
        assert!((v - 100.0).abs() < 1e-6, "slider should clamp to max: got {}", v);

        ctx.begin_frame(0.0, 0.0, false);
        // Value below min should be clamped
        let v2 = ctx.slider("test", -10.0, 0.0, 100.0);
        assert!((v2 - 0.0).abs() < 1e-6, "slider should clamp to min: got {}", v2);
    }

    // ── Text Input Tests ──────────────────────────────────────────────

    #[test]
    fn test_text_input_basic_typing() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::new();

        // Click on the text input to focus it (at x=10, y=10, width=200)
        ctx.begin_frame(15.0, 15.0, true);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);

        // Now type some characters
        ctx.begin_frame(15.0, 15.0, false);
        ctx.input_char('H');
        ctx.input_char('i');
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(changed);
        assert_eq!(buf, "Hi");
    }

    #[test]
    fn test_text_input_backspace() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::from("abc");

        // Focus — click far right so cursor goes to end (padding=4, gw=8, 3 chars → need x > 10+4+24=38)
        ctx.begin_frame(50.0, 15.0, true);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);

        // Backspace
        ctx.begin_frame(50.0, 15.0, false);
        ctx.input_key(Key::Backspace);
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(changed);
        assert_eq!(buf, "ab");
    }

    #[test]
    fn test_text_input_cursor_movement() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::from("abc");

        // Focus — click far right so cursor goes to end
        ctx.begin_frame(50.0, 15.0, true);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert_eq!(ctx.cursor_pos, 3); // cursor at end of "abc"

        // Move cursor to beginning with Home
        ctx.begin_frame(15.0, 15.0, false);
        ctx.input_key(Key::Home);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert_eq!(ctx.cursor_pos, 0);

        // Type 'X' at beginning
        ctx.begin_frame(15.0, 15.0, false);
        ctx.input_char('X');
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(changed);
        assert_eq!(buf, "Xabc");
        assert_eq!(ctx.cursor_pos, 1);
    }

    #[test]
    fn test_text_input_delete_key() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::from("abc");

        // Focus
        ctx.begin_frame(15.0, 15.0, true);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);

        // Move to Home, then Delete
        ctx.begin_frame(15.0, 15.0, false);
        ctx.input_key(Key::Home);
        ctx.input_key(Key::Delete);
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(changed);
        assert_eq!(buf, "bc");
    }

    #[test]
    fn test_text_input_arrow_keys() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::from("hello");

        // Focus — click far right so cursor at end
        ctx.begin_frame(100.0, 15.0, true);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);

        // Left twice from end (pos 5→3)
        ctx.begin_frame(100.0, 15.0, false);
        ctx.input_key(Key::Left);
        ctx.input_key(Key::Left);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert_eq!(ctx.cursor_pos, 3);

        // Type 'X' at position 3
        ctx.begin_frame(100.0, 15.0, false);
        ctx.input_char('X');
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(changed);
        assert_eq!(buf, "helXlo");
    }

    #[test]
    fn test_text_input_no_change_when_not_focused() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::from("test");

        // Don't click on the input — mouse at (500, 500) far away
        ctx.begin_frame(500.0, 500.0, true);
        ctx.input_char('X');
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(!changed);
        assert_eq!(buf, "test");
    }

    // ── Scroll Panel Tests ────────────────────────────────────────────

    #[test]
    fn test_scroll_offset_clamping() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Panel at (0,0), 200x100, content_height=300
        // Scroll down a lot
        ctx.begin_frame(100.0, 50.0, false);
        ctx.set_scroll(-100.0); // scroll down
        ctx.begin_scroll_panel(1, 0.0, 0.0, 200.0, 100.0, 300.0);
        ctx.end_scroll_panel();

        // max_scroll = 300 - 100 = 200; scroll should be clamped
        let scroll = ctx.scroll_offsets.get(&1).copied().unwrap_or(0.0);
        assert!(scroll >= 0.0 && scroll <= 200.0, "scroll={}", scroll);
    }

    #[test]
    fn test_scroll_offset_does_not_go_negative() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Scroll up when already at top
        ctx.begin_frame(100.0, 50.0, false);
        ctx.set_scroll(100.0); // scroll up
        ctx.begin_scroll_panel(1, 0.0, 0.0, 200.0, 100.0, 300.0);
        ctx.end_scroll_panel();

        let scroll = ctx.scroll_offsets.get(&1).copied().unwrap_or(0.0);
        assert!((scroll - 0.0).abs() < 1e-6, "scroll should be 0, got {}", scroll);
    }

    #[test]
    fn test_scroll_panel_content_clipping() {
        let mut ctx = UiContext::new(800.0, 600.0);

        ctx.begin_frame(100.0, 50.0, false);
        ctx.begin_scroll_panel(1, 10.0, 20.0, 200.0, 100.0, 300.0);

        // Draw some content inside
        ctx.text("Inside scroll");
        // Commands drawn inside should have a scissor rect
        let has_scissor = ctx.draw_list.commands.iter().any(|c| c.scissor.is_some());
        assert!(has_scissor, "commands inside scroll panel should have scissor rects");

        ctx.end_scroll_panel();

        // Commands drawn after end_scroll_panel should NOT have scissor
        let cmds_before = ctx.draw_list.commands.len();
        ctx.text("Outside scroll");
        let new_cmds = &ctx.draw_list.commands[cmds_before..];
        let has_scissor_after = new_cmds.iter().any(|c| c.scissor.is_some());
        assert!(!has_scissor_after, "commands after end_scroll_panel should not have scissor");
    }

    // ── Drag and Drop Tests ──────────────────────────────────────────

    #[test]
    fn test_drag_start_and_end() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Frame 1: mouse down — begin drag
        ctx.begin_frame(100.0, 100.0, true);
        ctx.begin_drag(1, 42);
        assert!(!ctx.is_dragging(), "should not be dragging yet (below threshold)");
        let _ = ctx.end_drag();

        // Frame 2: mouse moved past threshold, still down
        ctx.begin_frame(110.0, 100.0, true);
        let _ = ctx.end_drag();
        assert!(ctx.is_dragging(), "should be dragging after moving past threshold");

        // Frame 3: mouse released
        ctx.begin_frame(120.0, 100.0, false);
        let result = ctx.end_drag();
        assert!(result.is_some());
        let (src_id, payload) = result.unwrap();
        assert_eq!(src_id, 1);
        assert_eq!(payload, 42);
    }

    #[test]
    fn test_drop_on_target() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Frame 1: begin drag
        ctx.begin_frame(100.0, 100.0, true);
        ctx.begin_drag(1, 99);
        let _ = ctx.end_drag();

        // Frame 2: move past threshold
        ctx.begin_frame(110.0, 100.0, true);
        let _ = ctx.end_drag();

        // Frame 3: release over drop target at (200, 200, 50, 50)
        ctx.begin_frame(220.0, 220.0, false);
        let drop_result = ctx.drop_target(2, 200.0, 200.0, 50.0, 50.0);
        assert_eq!(drop_result, Some(99));
        let _ = ctx.end_drag();
    }

    #[test]
    fn test_drop_outside_target() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Frame 1: begin drag
        ctx.begin_frame(100.0, 100.0, true);
        ctx.begin_drag(1, 77);
        let _ = ctx.end_drag();

        // Frame 2: move past threshold
        ctx.begin_frame(110.0, 100.0, true);
        let _ = ctx.end_drag();

        // Frame 3: release far from drop target
        ctx.begin_frame(500.0, 500.0, false);
        let drop_result = ctx.drop_target(2, 200.0, 200.0, 50.0, 50.0);
        assert_eq!(drop_result, None);
    }
}
