//! Voltex platform abstraction — winit window + per-frame input + fixed-
//! timestep game loop timer. Owns the (very thin) bridge between the OS
//! event source and the engine's user-facing types.
//!
//! Three core types: [`VoltexWindow`], [`InputState`], [`GameTimer`].
//! Everything else is configuration ([`WindowConfig`]).

#![warn(missing_docs)]

pub mod window;
pub mod input;
pub mod game_loop;

pub use window::{VoltexWindow, WindowConfig};
pub use input::InputState;
pub use game_loop::GameTimer;
