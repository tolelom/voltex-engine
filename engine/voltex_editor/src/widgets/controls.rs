use crate::ui_context::UiContext;

// Color palette
const COLOR_BUTTON: [u8; 4]         = [0x44, 0x44, 0x55, 0xFF];
const COLOR_BUTTON_HOT: [u8; 4]     = [0x55, 0x66, 0x88, 0xFF];
const COLOR_BUTTON_ACTIVE: [u8; 4]  = [0x44, 0x88, 0xFF, 0xFF];
const COLOR_TEXT: [u8; 4]           = [0xEE, 0xEE, 0xEE, 0xFF];
const COLOR_SLIDER_BG: [u8; 4]      = [0x44, 0x44, 0x44, 0xFF];
const COLOR_SLIDER_HANDLE: [u8; 4]  = [0x88, 0x88, 0xFF, 0xFF];
const COLOR_CHECK_BG: [u8; 4]       = [0x44, 0x44, 0x44, 0xFF];
const COLOR_CHECK_MARK: [u8; 4]     = [0x88, 0xFF, 0x88, 0xFF];

impl UiContext {
    /// Draw text at the current cursor position and advance to the next line.
    pub fn text(&mut self, text: &str) {
        let x = self.layout.cursor_x;
        let y = self.layout.cursor_y;
        let gw = self.font.glyph_width as f32;
        self.draw_list.add_text(&self.font, text, x, y, COLOR_TEXT);

        let text_w = text.chars().count() as f32 * gw;
        self.layout.record_last_widget(x, y, text_w);
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
        self.draw_list.add_text(&self.font, label, text_x, text_y, COLOR_TEXT);

        self.layout.record_last_widget(x, y, btn_w);
        self.layout.advance_line();

        // Return true if mouse was released over this button while it was active
        let clicked = hovered && self.mouse_released && self.active == Some(id);
        if self.mouse_released
            && self.active == Some(id) {
                self.active = None;
            }
        clicked
    }

    /// Draw a registered UI texture (slot from `UiRenderer::register_texture`)
    /// as a `w`×`h` image at the layout cursor. The layout advances past the
    /// full image height — not just one text line — so following widgets land
    /// below it. An unregistered slot renders via the font-atlas fallback.
    pub fn image(&mut self, texture_slot: u32, w: f32, h: f32) {
        let x = self.layout.cursor_x;
        let y = self.layout.cursor_y;
        self.draw_list.add_image(x, y, w, h, texture_slot);
        self.layout.record_last_widget(x, y, w);
        self.layout.cursor_y = y + h + self.layout.padding;
        self.layout.cursor_x = self.layout.indent;
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
        self.draw_list.add_text(&self.font, label, label_x, label_y, COLOR_TEXT);

        let total_w = slider_w + padding + label.chars().count() as f32 * gw;
        self.layout.record_last_widget(x, y, total_w);
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
        self.draw_list.add_text(&self.font, label, label_x, label_y, COLOR_TEXT);

        let total_w = box_size + padding + label.chars().count() as f32 * gw;
        self.layout.record_last_widget(x, y, total_w);
        self.layout.advance_line();

        new_checked
    }
}

#[cfg(test)]
mod tests {
    use crate::ui_context::UiContext;

    #[test]
    fn image_widget_advances_layout_past_its_height() {
        let mut ui = UiContext::new(800.0, 600.0);
        ui.begin_frame(0.0, 0.0, false);
        let y_before = ui.layout.cursor_y;

        ui.image(0, 96.0, 96.0);

        // The next widget must land BELOW the image, not one text line down.
        assert!(
            ui.layout.cursor_y >= y_before + 96.0,
            "cursor_y advanced {} but image is 96 tall",
            ui.layout.cursor_y - y_before
        );
        let cmd = ui.draw_list.commands.last().expect("image emitted a command");
        assert_eq!(cmd.texture, Some(0));
        ui.end_frame();
    }

    #[test]
    fn buttons_stack_vertically_by_default() {
        let mut ui = UiContext::new(800.0, 600.0);
        ui.begin_frame(0.0, 0.0, false);
        ui.layout.cursor_x = 10.0;
        ui.layout.cursor_y = 10.0;
        let _ = ui.button("A");
        // Without same_line the next widget is on a new line below.
        assert_eq!(ui.layout.cursor_x, ui.layout.indent);
        assert!(ui.layout.cursor_y > 10.0, "cursor should advance down");
    }

    #[test]
    fn same_line_places_next_widget_to_the_right() {
        let mut ui = UiContext::new(800.0, 600.0);
        ui.begin_frame(0.0, 0.0, false);
        ui.layout.cursor_x = 10.0;
        ui.layout.cursor_y = 10.0;
        let _ = ui.button("A");
        ui.same_line();
        // Same row as A, advanced to the right of it.
        assert_eq!(ui.layout.cursor_y, 10.0, "same line keeps the original y");
        assert!(ui.layout.cursor_x > 10.0, "cursor should move right past button A");
    }

    #[test]
    fn toolbar_row_stays_within_its_height() {
        // Four buttons with same_line between them must not overflow a 32px
        // toolbar (the editor_demo toolbar bug this fixes).
        let mut ui = UiContext::new(1280.0, 720.0);
        ui.begin_frame(0.0, 0.0, false);
        ui.layout.cursor_x = 0.0;
        ui.layout.cursor_y = 0.0;
        let labels = ["Play", "Spawn", "Save", "Load"];
        for (i, l) in labels.iter().enumerate() {
            if i > 0 {
                ui.same_line();
            }
            let _ = ui.button(l);
        }
        // All four drew on the same row (y stayed at 0 via same_line).
        assert_eq!(ui.layout.prev_y, 0.0, "all toolbar buttons share one row");
    }
}
