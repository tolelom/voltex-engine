//! Per-frame input snapshot — keyboard + mouse + scroll.
//!
//! Owned by [`crate::VoltexWindow`]'s caller (typically
//! `voltex_runtime::RuntimeApp`). Each frame:
//!
//! 1. winit events feed [`InputState::process_key`] / `process_mouse_*`.
//! 2. User systems read state via the `is_*` / `mouse_*` accessors.
//! 3. The runtime calls [`InputState::begin_frame`] at frame end to
//!    clear the `just_*` sets and delta accumulators.

use winit::keyboard::KeyCode;
use std::collections::HashSet;
use winit::event::MouseButton;

/// Snapshot of all input state for the current frame.
pub struct InputState {
    pressed: HashSet<KeyCode>,
    just_pressed: HashSet<KeyCode>,
    just_released: HashSet<KeyCode>,
    mouse_position: (f64, f64),
    /// Tracks whether `mouse_position` has been initialised by a real
    /// winit `CursorMoved` event. The first event is treated as a warp
    /// (delta = 0) so the camera doesn't snap when the cursor enters
    /// the window — `(0, 0) → (500, 300)` used to produce a 500-px
    /// delta on the first frame.
    mouse_seen: bool,
    mouse_delta: (f64, f64),
    mouse_buttons: HashSet<MouseButton>,
    mouse_buttons_just_pressed: HashSet<MouseButton>,
    mouse_buttons_just_released: HashSet<MouseButton>,
    mouse_scroll_delta: f32,
    /// Current window inner size in **physical pixels** — same unit as
    /// [`mouse_position`](Self::mouse_position), so screen→NDC math works
    /// at any DPI scale. `(0, 0)` until the runtime injects the real size
    /// via [`set_window_size`](Self::set_window_size); callers must guard
    /// against zero before dividing.
    window_size: (u32, u32),
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

impl InputState {
    /// Construct an empty input state — no keys / buttons pressed,
    /// mouse at `(0, 0)`.
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
            just_pressed: HashSet::new(),
            just_released: HashSet::new(),
            mouse_position: (0.0, 0.0),
            mouse_seen: false,
            mouse_delta: (0.0, 0.0),
            mouse_buttons: HashSet::new(),
            mouse_buttons_just_pressed: HashSet::new(),
            mouse_buttons_just_released: HashSet::new(),
            mouse_scroll_delta: 0.0,
            window_size: (0, 0),
        }
    }

    /// `true` while `key` is held down.
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    /// `true` for one frame on the press edge of `key`.
    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed.contains(&key)
    }

    /// `true` for one frame on the release edge of `key`.
    pub fn is_key_just_released(&self, key: KeyCode) -> bool {
        self.just_released.contains(&key)
    }

    /// Mouse cursor position in physical pixels, `(x, y)` from the
    /// top-left of the window's client area.
    pub fn mouse_position(&self) -> (f64, f64) {
        self.mouse_position
    }

    /// Mouse motion `(dx, dy)` accumulated since the last
    /// [`begin_frame`](Self::begin_frame).
    pub fn mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }

    /// `true` while `button` is held down.
    pub fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons.contains(&button)
    }

    /// `true` for one frame on the press edge of `button`.
    pub fn is_mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_just_pressed.contains(&button)
    }

    /// Mouse scroll-wheel delta accumulated since the last
    /// [`begin_frame`](Self::begin_frame). Sign: positive = up / away.
    pub fn mouse_scroll(&self) -> f32 {
        self.mouse_scroll_delta
    }

    /// Window inner size in **physical pixels** (same unit as
    /// [`mouse_position`](Self::mouse_position)). Returns `(0, 0)` if the
    /// runtime has not injected a size yet — guard before dividing.
    pub fn window_size(&self) -> (u32, u32) {
        self.window_size
    }

    /// Record the window inner size in physical pixels. Called by the
    /// runtime on window creation and on every `Resized` event. Persists
    /// across [`begin_frame`](Self::begin_frame) like `mouse_seen`.
    pub fn set_window_size(&mut self, width: u32, height: u32) {
        self.window_size = (width, height);
    }

    /// Clear the `just_*` sets and reset mouse / scroll deltas to zero.
    /// Called by the runtime at the end of each frame.
    pub fn begin_frame(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.mouse_buttons_just_pressed.clear();
        self.mouse_buttons_just_released.clear();
        self.mouse_delta = (0.0, 0.0);
        self.mouse_scroll_delta = 0.0;
    }

    /// Apply a key event from winit. `pressed` is the new state.
    ///
    /// OS key-repeat delivers additional `Pressed` events while a key is
    /// held; those must NOT re-fire the `just_pressed` edge — only the
    /// true off→on transition does. A repeat on an already-held key
    /// clears any stale `just_pressed` entry so `is_key_just_pressed`
    /// stays a single-frame edge.
    pub fn process_key(&mut self, key: KeyCode, pressed: bool) {
        if pressed {
            if self.pressed.insert(key) {
                self.just_pressed.insert(key);
            } else {
                self.just_pressed.remove(&key);
            }
        } else if self.pressed.remove(&key) {
            self.just_released.insert(key);
        }
    }

    /// Apply a `CursorMoved` event from winit. Adds the delta to the
    /// accumulator and replaces the stored cursor position.
    ///
    /// The very first event after construction (or after the window
    /// regains focus and produces a CursorMoved at the cursor's
    /// arrival position) does **not** contribute to the delta — that
    /// gap is treated as a warp, not motion. Without this, the camera
    /// snapped on focus.
    pub fn process_mouse_move(&mut self, x: f64, y: f64) {
        if self.mouse_seen {
            self.mouse_delta.0 += x - self.mouse_position.0;
            self.mouse_delta.1 += y - self.mouse_position.1;
        } else {
            self.mouse_seen = true;
        }
        self.mouse_position = (x, y);
    }

    /// Apply a mouse-button event from winit. `pressed` is the new
    /// state.
    pub fn process_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if pressed {
            if self.mouse_buttons.insert(button) {
                self.mouse_buttons_just_pressed.insert(button);
            }
        } else if self.mouse_buttons.remove(&button) {
            self.mouse_buttons_just_released.insert(button);
        }
    }

    /// Apply a `MouseWheel` event from winit. Delta is added to the
    /// per-frame accumulator.
    pub fn process_scroll(&mut self, delta: f32) {
        self.mouse_scroll_delta += delta;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_mouse_move_produces_zero_delta() {
        // Pre-fix: mouse_position starts at (0,0); first CursorMoved at
        // (500,300) produced delta (500,300) which the camera then applied
        // as a huge snap.
        let mut input = InputState::new();
        input.process_mouse_move(500.0, 300.0);
        assert_eq!(input.mouse_delta(), (0.0, 0.0), "first move must not produce delta");
        // Subsequent moves produce normal deltas.
        input.process_mouse_move(505.0, 302.0);
        assert!((input.mouse_delta().0 - 5.0).abs() < 1e-9);
        assert!((input.mouse_delta().1 - 2.0).abs() < 1e-9);
    }

    #[test]
    fn begin_frame_clears_delta_but_keeps_mouse_seen() {
        // After a frame ends, the next move still uses the proper delta —
        // mouse_seen survives begin_frame so we don't get a second false zero.
        let mut input = InputState::new();
        input.process_mouse_move(100.0, 100.0); // initialise
        input.process_mouse_move(110.0, 100.0); // delta (10, 0)
        input.begin_frame();
        assert_eq!(input.mouse_delta(), (0.0, 0.0), "begin_frame clears delta");
        input.process_mouse_move(115.0, 100.0); // delta (5, 0)
        assert!((input.mouse_delta().0 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn key_press_edge_then_held_then_release() {
        let mut input = InputState::new();

        // Press edge: both just_pressed and pressed are set.
        input.process_key(KeyCode::KeyW, true);
        assert!(input.is_key_just_pressed(KeyCode::KeyW), "press edge must set just_pressed");
        assert!(input.is_key_pressed(KeyCode::KeyW), "press must set held state");
        assert!(!input.is_key_just_released(KeyCode::KeyW));

        // A second press event for an already-held key must NOT re-fire
        // just_pressed (this is the high-value no-re-fire assertion), but
        // the held state stays true.
        input.process_key(KeyCode::KeyW, true);
        assert!(!input.is_key_just_pressed(KeyCode::KeyW), "held key must not re-fire just_pressed");
        assert!(input.is_key_pressed(KeyCode::KeyW), "held state must persist");

        // Release edge: just_released set, held state cleared.
        input.process_key(KeyCode::KeyW, false);
        assert!(input.is_key_just_released(KeyCode::KeyW), "release edge must set just_released");
        assert!(!input.is_key_pressed(KeyCode::KeyW), "release must clear held state");
    }

    #[test]
    fn begin_frame_clears_just_sets_but_keeps_held() {
        let mut input = InputState::new();
        input.process_key(KeyCode::KeyW, true);
        input.process_mouse_button(MouseButton::Left, true);
        input.process_scroll(2.0);

        input.begin_frame();

        // just_* edges and scroll/delta accumulators are cleared.
        assert!(!input.is_key_just_pressed(KeyCode::KeyW), "begin_frame clears just_pressed");
        assert!(!input.is_mouse_button_just_pressed(MouseButton::Left), "begin_frame clears button just_pressed");
        assert_eq!(input.mouse_scroll(), 0.0, "begin_frame clears scroll");
        assert_eq!(input.mouse_delta(), (0.0, 0.0), "begin_frame clears delta");

        // Held state survives a frame boundary.
        assert!(input.is_key_pressed(KeyCode::KeyW), "begin_frame keeps held key");
        assert!(input.is_mouse_button_pressed(MouseButton::Left), "begin_frame keeps held button");

        // A release after begin_frame still produces a just_released edge.
        input.process_key(KeyCode::KeyW, false);
        assert!(input.is_key_just_released(KeyCode::KeyW), "release edge after begin_frame");
    }

    #[test]
    fn window_size_set_get_roundtrip() {
        // Default is (0, 0) = "not yet known" — callers must guard before
        // dividing (NDC conversion). After injection the exact value comes back.
        let mut input = InputState::new();
        assert_eq!(input.window_size(), (0, 0), "unset window size is (0, 0)");
        input.set_window_size(1600, 900);
        assert_eq!(input.window_size(), (1600, 900));
    }

    #[test]
    fn window_size_survives_begin_frame() {
        // Window size is persistent state (like mouse_seen), not a per-frame
        // accumulator — begin_frame must not reset it.
        let mut input = InputState::new();
        input.set_window_size(1600, 900);
        input.begin_frame();
        assert_eq!(input.window_size(), (1600, 900), "begin_frame keeps window size");
    }

    #[test]
    fn scroll_accumulates_within_frame() {
        let mut input = InputState::new();
        input.process_scroll(1.0);
        input.process_scroll(0.5);
        assert!((input.mouse_scroll() - 1.5).abs() < 1e-9, "scroll must accumulate: got {}", input.mouse_scroll());
    }
}
