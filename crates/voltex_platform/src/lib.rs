//! Voltex platform abstraction — winit window + per-frame input + fixed-
//! timestep game loop timer. Owns the (very thin) bridge between the OS
//! event source and the engine's user-facing types.
//!
//! Three core types: [`VoltexWindow`], [`InputState`], [`GameTimer`].
//! Everything else is configuration ([`WindowConfig`]).

#![warn(missing_docs)]

pub mod window;
pub mod input;
pub mod input_map;
pub mod game_loop;

pub use window::{VoltexWindow, WindowConfig};
pub use input::InputState;
pub use input_map::{InputMap, Trigger};
pub use game_loop::GameTimer;

/// winit key/button enums, re-exported so game code can bind inputs without a
/// direct `winit` dependency (`use voltex_platform::KeyCode`).
pub use winit::event::MouseButton;
pub use winit::keyboard::KeyCode;
