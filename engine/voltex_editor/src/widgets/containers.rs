use crate::ui_context::UiContext;

// Color palette
const COLOR_BG: [u8; 4]             = [0x2B, 0x2B, 0x2B, 0xFF];
const COLOR_TEXT: [u8; 4]           = [0xEE, 0xEE, 0xEE, 0xFF];
const COLOR_PANEL: [u8; 4]          = [0x33, 0x33, 0x33, 0xFF];
const COLOR_SCROLLBAR_BG: [u8; 4]   = [0x33, 0x33, 0x33, 0xFF];
const COLOR_SCROLLBAR_THUMB: [u8; 4]= [0x66, 0x66, 0x77, 0xFF];

impl UiContext {
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
}
