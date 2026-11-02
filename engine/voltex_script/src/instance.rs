//! A single Lua interpreter bound to one `.lua` file.
//!
//! Multiple entities sharing the same `ScriptComponent::script_path` use the
//! same `ScriptInstance` — globals defined in the script are shared. This is
//! intentional: per-entity state belongs in component data, not in the script.

use crate::state::LuaState;

/// One Lua interpreter bound to a single script file path.
pub struct ScriptInstance {
    /// Owned Lua state for this script.
    pub lua: LuaState,
    /// VFS-relative path of the loaded script.
    pub path: String,
    /// `false` until the first `script_start_all` pass invokes `on_start`,
    /// then `true`. Hot reload resets this back to `false`.
    pub started: bool,
}
