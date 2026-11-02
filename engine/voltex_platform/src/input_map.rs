//! Input action mapping — a layer over [`InputState`] that lets a game query
//! semantic *actions* ("jump", "fire") and *axes* ("move_x") instead of raw
//! keycodes. This is the equivalent of Unity's Input Actions / Godot's
//! `InputMap`: rebind a control in one place and every query follows.
//!
//! [`InputMap`] is pure state-free mapping — it holds only the bindings and
//! reads a borrowed [`InputState`] on each query, so it is trivially testable
//! without a window or GPU.
//!
//! ```ignore
//! use voltex_platform::{InputMap, InputState, KeyCode};
//!
//! let mut map = InputMap::new();
//! map.bind_key("jump", KeyCode::Space)
//!    .bind_axis("move_x", KeyCode::KeyD, KeyCode::KeyA)
//!    .bind_axis("move_y", KeyCode::KeyW, KeyCode::KeyS);
//!
//! // each frame, with the live InputState:
//! if map.just_pressed("jump", &input) { /* … */ }
//! let dir = map.vector("move_x", "move_y", &input); // Vec2 in [-1,1]²
//! ```

use std::collections::HashMap;

use voltex_math::Vec2;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use crate::InputState;

/// A single physical input that can fire an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// A keyboard key.
    Key(KeyCode),
    /// A mouse button.
    Mouse(MouseButton),
}

impl Trigger {
    fn is_pressed(&self, input: &InputState) -> bool {
        match *self {
            Trigger::Key(k) => input.is_key_pressed(k),
            Trigger::Mouse(b) => input.is_mouse_button_pressed(b),
        }
    }

    fn is_just_pressed(&self, input: &InputState) -> bool {
        match *self {
            Trigger::Key(k) => input.is_key_just_pressed(k),
            Trigger::Mouse(b) => input.is_mouse_button_just_pressed(b),
        }
    }

    fn is_just_released(&self, input: &InputState) -> bool {
        match *self {
            Trigger::Key(k) => input.is_key_just_released(k),
            // InputState exposes no public mouse just-released query; mouse
            // triggers never contribute a release edge (documented limitation).
            Trigger::Mouse(_) => false,
        }
    }
}

/// A named mapping from semantic actions/axes to physical [`Trigger`]s.
#[derive(Debug, Clone, Default)]
pub struct InputMap {
    /// action name → triggers (held if ANY is held).
    actions: HashMap<String, Vec<Trigger>>,
    /// axis name → (positive triggers, negative triggers).
    axes: HashMap<String, (Vec<Trigger>, Vec<Trigger>)>,
}

impl InputMap {
    /// An empty map with no bindings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a [`Trigger`] to an action. Calling repeatedly with the same action
    /// adds alternatives (the action fires if *any* bound trigger fires).
    pub fn bind(&mut self, action: &str, trigger: Trigger) -> &mut Self {
        self.actions.entry(action.to_string()).or_default().push(trigger);
        self
    }

    /// Convenience: bind a keyboard key to an action.
    pub fn bind_key(&mut self, action: &str, key: KeyCode) -> &mut Self {
        self.bind(action, Trigger::Key(key))
    }

    /// Convenience: bind a mouse button to an action.
    pub fn bind_mouse(&mut self, action: &str, button: MouseButton) -> &mut Self {
        self.bind(action, Trigger::Mouse(button))
    }

    /// Bind a 1-D axis from a `positive` and a `negative` key. The axis reads
    /// `+1` while only positive is held, `-1` while only negative is held, and
    /// `0` when both or neither are held. Repeated calls add alternatives.
    pub fn bind_axis(&mut self, axis: &str, positive: KeyCode, negative: KeyCode) -> &mut Self {
        let entry = self.axes.entry(axis.to_string()).or_default();
        entry.0.push(Trigger::Key(positive));
        entry.1.push(Trigger::Key(negative));
        self
    }

    /// Bind a 1-D axis from arbitrary positive/negative triggers (e.g. mouse
    /// buttons or non-key inputs).
    pub fn bind_axis_triggers(&mut self, axis: &str, positive: Trigger, negative: Trigger) -> &mut Self {
        let entry = self.axes.entry(axis.to_string()).or_default();
        entry.0.push(positive);
        entry.1.push(negative);
        self
    }

    /// True while any trigger bound to `action` is held. Unbound → false.
    pub fn is_pressed(&self, action: &str, input: &InputState) -> bool {
        self.actions
            .get(action)
            .is_some_and(|ts| ts.iter().any(|t| t.is_pressed(input)))
    }

    /// True on the frame any trigger bound to `action` transitions to pressed.
    pub fn just_pressed(&self, action: &str, input: &InputState) -> bool {
        self.actions
            .get(action)
            .is_some_and(|ts| ts.iter().any(|t| t.is_just_pressed(input)))
    }

    /// True on the frame any (keyboard) trigger bound to `action` is released.
    pub fn just_released(&self, action: &str, input: &InputState) -> bool {
        self.actions
            .get(action)
            .is_some_and(|ts| ts.iter().any(|t| t.is_just_released(input)))
    }

    /// The current value of a 1-D axis in `[-1, 1]`. Unbound → 0.
    pub fn axis(&self, axis: &str, input: &InputState) -> f32 {
        let Some((pos, neg)) = self.axes.get(axis) else {
            return 0.0;
        };
        let p = pos.iter().any(|t| t.is_pressed(input)) as i32 as f32;
        let n = neg.iter().any(|t| t.is_pressed(input)) as i32 as f32;
        p - n
    }

    /// Compose two axes into a 2-D vector. Components are each in `[-1, 1]`; the
    /// raw (un-normalized) vector is returned, so a diagonal has length √2 —
    /// call [`Vec2::normalize_or_zero`] if you need a unit direction.
    pub fn vector(&self, x_axis: &str, y_axis: &str, input: &InputState) -> Vec2 {
        Vec2::new(self.axis(x_axis, input), self.axis(y_axis, input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> InputMap {
        let mut m = InputMap::new();
        m.bind_key("jump", KeyCode::Space)
            .bind_key("jump", KeyCode::KeyW) // alternative binding
            .bind_mouse("fire", MouseButton::Left)
            .bind_axis("move_x", KeyCode::KeyD, KeyCode::KeyA)
            .bind_axis("move_y", KeyCode::KeyW, KeyCode::KeyS);
        m
    }

    #[test]
    fn action_pressed_when_any_bound_trigger_held() {
        let m = map();
        let mut input = InputState::new();
        input.process_key(KeyCode::Space, true);
        assert!(m.is_pressed("jump", &input));
    }

    #[test]
    fn action_not_pressed_when_no_trigger_held() {
        let m = map();
        let input = InputState::new();
        assert!(!m.is_pressed("jump", &input));
    }

    #[test]
    fn unbound_action_queries_are_false_and_zero() {
        let m = map();
        let input = InputState::new();
        assert!(!m.is_pressed("nonexistent", &input));
        assert!(!m.just_pressed("nonexistent", &input));
        assert_eq!(m.axis("nonexistent", &input), 0.0);
    }

    #[test]
    fn just_pressed_fires_only_on_the_edge() {
        let m = map();
        let mut input = InputState::new();
        input.process_key(KeyCode::Space, true);
        assert!(m.just_pressed("jump", &input), "press edge");

        // Next frame, key still held (OS key-repeat) → no re-fire.
        input.begin_frame();
        input.process_key(KeyCode::Space, true);
        assert!(!m.just_pressed("jump", &input), "held key must not re-fire");
        assert!(m.is_pressed("jump", &input), "still held");
    }

    #[test]
    fn just_released_fires_on_key_release() {
        let m = map();
        let mut input = InputState::new();
        input.process_key(KeyCode::Space, true);
        input.begin_frame();
        input.process_key(KeyCode::Space, false);
        assert!(m.just_released("jump", &input));
    }

    #[test]
    fn axis_is_positive_negative_or_zero() {
        let m = map();
        let mut input = InputState::new();

        input.process_key(KeyCode::KeyD, true);
        assert_eq!(m.axis("move_x", &input), 1.0, "positive only");

        input.process_key(KeyCode::KeyA, true);
        assert_eq!(m.axis("move_x", &input), 0.0, "both cancel");

        input.process_key(KeyCode::KeyD, false);
        assert_eq!(m.axis("move_x", &input), -1.0, "negative only");
    }

    #[test]
    fn vector_composes_two_axes() {
        let m = map();
        let mut input = InputState::new();
        input.process_key(KeyCode::KeyD, true); // +x
        input.process_key(KeyCode::KeyW, true); // +y
        let v = m.vector("move_x", "move_y", &input);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 1.0);
    }

    #[test]
    fn mouse_button_trigger_drives_action() {
        let m = map();
        let mut input = InputState::new();
        input.process_mouse_button(MouseButton::Left, true);
        assert!(m.is_pressed("fire", &input));
        assert!(m.just_pressed("fire", &input));
    }
}
