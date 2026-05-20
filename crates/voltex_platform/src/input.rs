use winit::keyboard::KeyCode;
use std::collections::HashSet;
use winit::event::MouseButton;

pub struct InputState {
    pressed: HashSet<KeyCode>,
    just_pressed: HashSet<KeyCode>,
    just_released: HashSet<KeyCode>,
    mouse_position: (f64, f64),
    mouse_delta: (f64, f64),
    mouse_buttons: HashSet<MouseButton>,
    mouse_buttons_just_pressed: HashSet<MouseButton>,
    mouse_buttons_just_released: HashSet<MouseButton>,
    mouse_scroll_delta: f32,
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

impl InputState {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
            just_pressed: HashSet::new(),
            just_released: HashSet::new(),
            mouse_position: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
            mouse_buttons: HashSet::new(),
            mouse_buttons_just_pressed: HashSet::new(),
            mouse_buttons_just_released: HashSet::new(),
            mouse_scroll_delta: 0.0,
        }
    }

    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed.contains(&key)
    }

    pub fn is_key_just_released(&self, key: KeyCode) -> bool {
        self.just_released.contains(&key)
    }

    pub fn mouse_position(&self) -> (f64, f64) {
        self.mouse_position
    }

    pub fn mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }

    pub fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons.contains(&button)
    }

    pub fn is_mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_just_pressed.contains(&button)
    }

    pub fn mouse_scroll(&self) -> f32 {
        self.mouse_scroll_delta
    }

    pub fn begin_frame(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.mouse_buttons_just_pressed.clear();
        self.mouse_buttons_just_released.clear();
        self.mouse_delta = (0.0, 0.0);
        self.mouse_scroll_delta = 0.0;
    }

    pub fn process_key(&mut self, key: KeyCode, pressed: bool) {
        if pressed {
            if self.pressed.insert(key) {
                self.just_pressed.insert(key);
            }
        } else {
            if self.pressed.remove(&key) {
                self.just_released.insert(key);
            }
        }
    }

    pub fn process_mouse_move(&mut self, x: f64, y: f64) {
        self.mouse_delta.0 += x - self.mouse_position.0;
        self.mouse_delta.1 += y - self.mouse_position.1;
        self.mouse_position = (x, y);
    }

    pub fn process_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if pressed {
            if self.mouse_buttons.insert(button) {
                self.mouse_buttons_just_pressed.insert(button);
            }
        } else {
            if self.mouse_buttons.remove(&button) {
                self.mouse_buttons_just_released.insert(button);
            }
        }
    }

    pub fn process_scroll(&mut self, delta: f32) {
        self.mouse_scroll_delta += delta;
    }
}
