use std::collections::HashMap;
use crate::draw_list::DrawList;
use crate::font::FontAtlas;
use crate::layout::LayoutState;

/// Key events the UI system understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Backspace,
    Delete,
    Home,
    End,
}

pub struct UiContext {
    pub hot: Option<u64>,
    pub active: Option<u64>,
    pub draw_list: DrawList,
    pub layout: LayoutState,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_down: bool,
    pub mouse_clicked: bool,
    pub mouse_released: bool,
    pub mouse_scroll: f32,
    pub screen_width: f32,
    pub screen_height: f32,
    pub font: FontAtlas,
    id_counter: u64,
    prev_mouse_down: bool,
    // Text input state
    pub focused_id: Option<u32>,
    pub cursor_pos: usize,
    input_chars: Vec<char>,
    input_keys: Vec<Key>,
    // Scroll panel state
    pub scroll_offsets: HashMap<u32, f32>,
    // Drag and drop state
    pub dragging: Option<(u32, u64)>,
    pub drag_start: (f32, f32),
    pub(crate) drag_started: bool,
    /// TTF font for high-quality text rendering (None = bitmap fallback).
    pub ttf_font: Option<crate::ttf_font::TtfFont>,
}

impl UiContext {
    /// Create a new UiContext for the given screen dimensions.
    pub fn new(screen_w: f32, screen_h: f32) -> Self {
        UiContext {
            hot: None,
            active: None,
            draw_list: DrawList::new(),
            layout: LayoutState::new(0.0, 0.0),
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_down: false,
            mouse_clicked: false,
            mouse_released: false,
            mouse_scroll: 0.0,
            screen_width: screen_w,
            screen_height: screen_h,
            font: FontAtlas::generate(),
            id_counter: 0,
            prev_mouse_down: false,
            focused_id: None,
            cursor_pos: 0,
            input_chars: Vec::new(),
            input_keys: Vec::new(),
            scroll_offsets: HashMap::new(),
            dragging: None,
            drag_start: (0.0, 0.0),
            drag_started: false,
            ttf_font: None,
        }
    }

    /// Begin a new frame: clear draw list, reset id counter, update mouse state.
    pub fn begin_frame(&mut self, mx: f32, my: f32, mouse_down: bool) {
        self.draw_list.clear();
        self.id_counter = 0;
        self.hot = None;

        self.mouse_x = mx;
        self.mouse_y = my;

        // Compute transitions
        self.mouse_clicked = !self.prev_mouse_down && mouse_down;
        self.mouse_released = self.prev_mouse_down && !mouse_down;

        self.mouse_down = mouse_down;
        self.prev_mouse_down = mouse_down;

        // Reset layout to top-left
        self.layout = LayoutState::new(0.0, 0.0);
    }

    /// Feed a character input event (printable ASCII) for text input widgets.
    pub fn input_char(&mut self, ch: char) {
        if ch.is_ascii() && !ch.is_ascii_control() {
            self.input_chars.push(ch);
        }
    }

    /// Feed a key input event for text input widgets.
    pub fn input_key(&mut self, key: Key) {
        self.input_keys.push(key);
    }

    /// Set mouse scroll delta for this frame (positive = scroll up).
    pub fn set_scroll(&mut self, delta: f32) {
        self.mouse_scroll = delta;
    }

    /// Drain all pending input chars (consumed by text_input widget).
    pub(crate) fn drain_chars(&mut self) -> Vec<char> {
        std::mem::take(&mut self.input_chars)
    }

    /// Drain all pending key events (consumed by text_input widget).
    pub(crate) fn drain_keys(&mut self) -> Vec<Key> {
        std::mem::take(&mut self.input_keys)
    }

    /// End the current frame.
    pub fn end_frame(&mut self) {
        self.mouse_scroll = 0.0;
        // Clear any unconsumed input
        self.input_chars.clear();
        self.input_keys.clear();
    }

    /// Generate a new unique ID for this frame.
    pub fn gen_id(&mut self) -> u64 {
        self.id_counter += 1;
        self.id_counter
    }

    /// Check if the mouse cursor is inside the given rectangle.
    pub fn mouse_in_rect(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse_x >= x
            && self.mouse_x < x + w
            && self.mouse_y >= y
            && self.mouse_y < y + h
    }

    /// Draw text using TTF font if available, otherwise bitmap font fallback.
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32, color: [u8; 4]) {
        if let Some(ref mut ttf) = self.ttf_font {
            let ascender = ttf.ascender;
            let mut cx = x;
            for ch in text.chars() {
                let info = ttf.glyph(ch).clone();
                if info.width > 0.0 && info.height > 0.0 {
                    let gx = cx + info.bearing_x;
                    let gy = y + ascender - info.bearing_y;
                    let (u0, v0, u1, v1) = (info.uv[0], info.uv[1], info.uv[2], info.uv[3]);
                    self.draw_list.add_rect_uv(gx, gy, info.width, info.height, u0, v0, u1, v1, color);
                }
                cx += info.advance;
            }
        } else {
            // Bitmap font fallback
            let gw = self.font.glyph_width as f32;
            let gh = self.font.glyph_height as f32;
            let mut cx = x;
            for ch in text.chars() {
                let (u0, v0, u1, v1) = self.font.glyph_uv(ch);
                self.draw_list.add_rect_uv(cx, y, gw, gh, u0, v0, u1, v1, color);
                cx += gw;
            }
        }
    }

    /// Calculate text width using TTF or bitmap font.
    pub fn ttf_text_width(&mut self, text: &str) -> f32 {
        if let Some(ref mut ttf) = self.ttf_font {
            ttf.text_width(text)
        } else {
            text.len() as f32 * self.font.glyph_width as f32
        }
    }
}
