// Lua C API uses `L` as the conventional name for the lua_State pointer.
// We keep that convention throughout FFI bindings for readability.
#![allow(non_snake_case)]

pub mod error;
pub mod ffi;
pub mod state;
pub mod bindings;
pub mod engine_api;
pub mod instance;
pub mod pool;
pub mod system;

pub use error::{ScriptError, Result};
pub use state::LuaState;
pub use bindings::register_default_bindings;
pub use engine_api::register_engine_api;
pub use instance::ScriptInstance;
pub use pool::ScriptInstancePool;
pub use system::{script_start_all, script_update_all};
