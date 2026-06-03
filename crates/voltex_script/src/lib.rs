//! # voltex_script
//!
//! Lua 5.4 scripting integration. Bundled Lua C source + thin Rust API.
//!
//! - [`LuaState`] — owned Lua state wrapper. Exec source, register host
//!   functions, push/read tables, run coroutines, hot reload.
//! - [`register_default_bindings`] / [`register_engine_api`] — wire up the
//!   built-in `print`, vector helpers, and engine entity API.
//! - [`ScriptInstance`] / [`ScriptInstancePool`] — per-entity script
//!   instances, indexed by id; survives scene save/load.
//! - [`script_start_all`] / [`script_update_all`] — system functions invoked
//!   from the main loop scheduler.

// Lua C API uses `L` as the conventional name for the lua_State pointer.
// We keep that convention throughout FFI bindings for readability.
#![allow(non_snake_case)]
#![warn(missing_docs)]

pub mod error;
pub mod ffi;
pub mod state;
pub mod bindings;
pub mod engine_api;
pub mod engine_input;
pub mod instance;
pub mod pool;
pub mod system;

pub use error::{ScriptError, Result};
pub use state::LuaState;
pub use bindings::register_default_bindings;
pub use engine_api::register_engine_api;
pub use engine_input::{register_engine_input, set_current_input, ScriptInput};
pub use instance::ScriptInstance;
pub use pool::ScriptInstancePool;
pub use system::{script_start_all, script_update_all};
