//! Single-line editable text box on `UiContext` (`text_input`): focus on click,
//! keyboard editing with a cursor, returns `true` when the buffer changed.

use crate::ui_context::{Key, UiContext};

// Color palette
const COLOR_TEXT: [u8; 4]           = [0xEE, 0xEE, 0xEE, 0xFF];
const COLOR_INPUT_BG: [u8; 4]       = [0x22, 0x22, 0x22, 0xFF];
const COLOR_INPUT_BORDER: [u8; 4]   = [0x66, 0x66, 0x66, 0xFF];
const COLOR_INPUT_FOCUSED: [u8; 4]  = [0x44, 0x88, 0xFF, 0xFF];
const COLOR_CURSOR: [u8; 4]         = [0xFF, 0xFF, 0xFF, 0xFF];

impl UiContext {
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
        self.draw_list.add_text(&self.font, buffer, text_x, text_y, COLOR_TEXT);

        // Draw cursor if focused
        if self.focused_id == Some(id) {
            let cursor_x = text_x + self.cursor_pos as f32 * gw;
            self.draw_list.add_rect(cursor_x, text_y, 1.0, gh, COLOR_CURSOR);
        }

        self.layout.record_last_widget(x, y, width);
        self.layout.advance_line();
        changed
    }
}
