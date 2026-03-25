use crate::ui_context::UiContext;

// Color palette
const COLOR_BG: [u8; 4]             = [0x2B, 0x2B, 0x2B, 0xFF];
const COLOR_BUTTON: [u8; 4]         = [0x44, 0x44, 0x44, 0xFF];
const COLOR_BUTTON_HOT: [u8; 4]     = [0x55, 0x55, 0x55, 0xFF];
const COLOR_BUTTON_ACTIVE: [u8; 4]  = [0x66, 0x66, 0x66, 0xFF];
const COLOR_TEXT: [u8; 4]           = [0xEE, 0xEE, 0xEE, 0xFF];
const COLOR_PANEL: [u8; 4]          = [0x33, 0x33, 0x33, 0xFF];
const COLOR_SLIDER_BG: [u8; 4]      = [0x44, 0x44, 0x44, 0xFF];
const COLOR_SLIDER_HANDLE: [u8; 4]  = [0x88, 0x88, 0xFF, 0xFF];
const COLOR_CHECK_BG: [u8; 4]       = [0x44, 0x44, 0x44, 0xFF];
const COLOR_CHECK_MARK: [u8; 4]     = [0x88, 0xFF, 0x88, 0xFF];

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
}

#[cfg(test)]
mod tests {
    use crate::ui_context::UiContext;

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
}
