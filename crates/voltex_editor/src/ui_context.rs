use crate::draw_list::DrawList;
use crate::font::FontAtlas;
use crate::layout::LayoutState;

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
    pub screen_width: f32,
    pub screen_height: f32,
    pub font: FontAtlas,
    id_counter: u64,
    prev_mouse_down: bool,
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
            screen_width: screen_w,
            screen_height: screen_h,
            font: FontAtlas::generate(),
            id_counter: 0,
            prev_mouse_down: false,
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

    /// End the current frame.
    pub fn end_frame(&mut self) {
        // Nothing for now — GPU submission will hook in here later.
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
}
