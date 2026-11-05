//! Read-only input API exposed to Lua scripts as a global `input` table.
//!
//! ## Why a snapshot instead of a pointer to `InputState`
//!
//! `voltex_script` deliberately does **not** depend on `voltex_platform`/winit.
//! The runtime translates its platform-specific `InputState` into a plain
//! [`ScriptInput`] snapshot each frame (keys identified by string name) and
//! publishes a pointer to it via [`set_current_input`]. The Lua C functions
//! read that thread-local pointer.
//!
//! ## Lifetime / safety
//!
//! The pointer published by [`set_current_input`] must be valid for the
//! duration of the synchronous script execution that follows. The runtime
//! re-publishes it every frame (re-deriving the address) *immediately before*
//! running scripts, so a moved `RuntimeApp` never leaves a stale pointer live
//! during a call — mirroring the `__voltex_world` discipline. Scripts run on
//! the main thread, so a `thread_local` cell is sufficient and shares nothing
//! across threads.
//!
//! ```lua
//! function on_update(id, gen, dt)
//!     if input.is_key_pressed("W") then move_forward(id, gen, dt) end
//!     local mx, my = input.mouse_position()
//! end
//! ```

use crate::ffi;
use crate::state::LuaState;
use std::cell::Cell;
use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::os::raw::c_int;

/// Per-frame input snapshot handed to Lua scripts. Keys are identified by
/// string name (e.g. `"W"`, `"Space"`); the runtime owns the name↔platform
/// keycode mapping when it fills this from its `InputState`.
#[derive(Debug, Default, Clone)]
pub struct ScriptInput {
    pressed: HashSet<String>,
    just_pressed: HashSet<String>,
    mouse_x: f64,
    mouse_y: f64,
    mouse_dx: f64,
    mouse_dy: f64,
    mouse_left: bool,
    mouse_right: bool,
    mouse_middle: bool,
    scroll: f32,
}

impl ScriptInput {
    /// A fresh empty snapshot (no keys/buttons, mouse at origin).
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset every field — call once per frame before re-filling.
    pub fn clear(&mut self) {
        self.pressed.clear();
        self.just_pressed.clear();
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.scroll = 0.0;
        // mouse position / button state are overwritten wholesale by the
        // setters below, so they don't need explicit clearing.
    }

    /// Mark `name` as currently held this frame.
    pub fn set_key_pressed(&mut self, name: &str) {
        self.pressed.insert(name.to_string());
    }

    /// Mark `name` as having a press edge this frame.
    pub fn set_key_just_pressed(&mut self, name: &str) {
        self.just_pressed.insert(name.to_string());
    }

    /// Set mouse position (pixels) and per-frame delta.
    pub fn set_mouse(&mut self, x: f64, y: f64, dx: f64, dy: f64) {
        self.mouse_x = x;
        self.mouse_y = y;
        self.mouse_dx = dx;
        self.mouse_dy = dy;
    }

    /// Set held state of the three standard mouse buttons.
    pub fn set_buttons(&mut self, left: bool, right: bool, middle: bool) {
        self.mouse_left = left;
        self.mouse_right = right;
        self.mouse_middle = middle;
    }

    /// Set the accumulated scroll-wheel delta for this frame.
    pub fn set_scroll(&mut self, scroll: f32) {
        self.scroll = scroll;
    }

    /// `true` while the named key is held.
    pub fn key_pressed(&self, name: &str) -> bool {
        self.pressed.contains(name)
    }

    /// `true` for one frame on the named key's press edge.
    pub fn key_just_pressed(&self, name: &str) -> bool {
        self.just_pressed.contains(name)
    }

    /// `true` while the named mouse button (`"left"`/`"right"`/`"middle"`) is held.
    pub fn button_pressed(&self, name: &str) -> bool {
        match name {
            "left" => self.mouse_left,
            "right" => self.mouse_right,
            "middle" => self.mouse_middle,
            _ => false,
        }
    }
}

thread_local! {
    /// Pointer to the snapshot the current frame's scripts read. Null until the
    /// runtime publishes one.
    static SCRIPT_INPUT: Cell<*const ScriptInput> = const { Cell::new(std::ptr::null()) };
}

/// Publish `ptr` as the snapshot read by `input.*` Lua calls on this thread.
/// Pass `std::ptr::null()` to clear. The pointer must stay valid until the next
/// call to this function (the runtime re-publishes every frame).
pub fn set_current_input(ptr: *const ScriptInput) {
    SCRIPT_INPUT.with(|c| c.set(ptr));
}

/// Run `f` with the currently published snapshot, or return `default` if none.
fn with_input<R>(default: R, f: impl FnOnce(&ScriptInput) -> R) -> R {
    SCRIPT_INPUT.with(|c| {
        let ptr = c.get();
        if ptr.is_null() {
            default
        } else {
            // SAFETY: non-null pointer published by `set_current_input`; the
            // runtime guarantees validity for the synchronous call window.
            f(unsafe { &*ptr })
        }
    })
}

/// Read a string argument at stack slot `idx`, or `None` if absent/non-string.
unsafe fn read_str_arg(L: *mut ffi::lua_State, idx: c_int) -> Option<String> {
    let cstr = ffi::lua_tostring(L, idx);
    if cstr.is_null() {
        None
    } else {
        Some(CStr::from_ptr(cstr).to_string_lossy().into_owned())
    }
}

/// Register the global `input` table into `lua`. Always succeeds; the table's
/// functions return defaults (`false` / `0`) until a snapshot is published.
pub fn register_engine_input(lua: &LuaState) {
    // SAFETY: `lua.raw()` is the live state owned by `lua`; the table pushed
    // here is consumed by lua_setglobal, leaving the stack balanced.
    unsafe {
        let L = lua.raw();
        ffi::lua_createtable(L, 0, 6);
        set_table_fn(L, "is_key_pressed", lua_is_key_pressed);
        set_table_fn(L, "is_key_just_pressed", lua_is_key_just_pressed);
        set_table_fn(L, "mouse_position", lua_mouse_position);
        set_table_fn(L, "mouse_delta", lua_mouse_delta);
        set_table_fn(L, "is_mouse_button_pressed", lua_is_mouse_button_pressed);
        set_table_fn(L, "scroll", lua_scroll);
        let name = CString::new("input").unwrap();
        ffi::lua_setglobal(L, name.as_ptr());
    }
}

/// Set `table[name] = f` where the table is on top of the stack (-1). Pops the
/// pushed function, leaving the table on top.
unsafe fn set_table_fn(L: *mut ffi::lua_State, name: &str, f: ffi::lua_CFunction) {
    ffi::lua_pushcclosure(L, f, 0);
    let key = CString::new(name).unwrap();
    ffi::lua_setfield(L, -2, key.as_ptr());
}

/// `input.is_key_pressed(name) -> bool`
unsafe extern "C" fn lua_is_key_pressed(L: *mut ffi::lua_State) -> c_int {
    let pressed = match read_str_arg(L, 1) {
        Some(name) => with_input(false, |i| i.key_pressed(&name)),
        None => false,
    };
    ffi::lua_pushboolean(L, pressed as c_int);
    1
}

/// `input.is_key_just_pressed(name) -> bool`
unsafe extern "C" fn lua_is_key_just_pressed(L: *mut ffi::lua_State) -> c_int {
    let pressed = match read_str_arg(L, 1) {
        Some(name) => with_input(false, |i| i.key_just_pressed(&name)),
        None => false,
    };
    ffi::lua_pushboolean(L, pressed as c_int);
    1
}

/// `input.mouse_position() -> x, y`
unsafe extern "C" fn lua_mouse_position(L: *mut ffi::lua_State) -> c_int {
    let (x, y) = with_input((0.0, 0.0), |i| (i.mouse_x, i.mouse_y));
    ffi::lua_pushnumber(L, x);
    ffi::lua_pushnumber(L, y);
    2
}

/// `input.mouse_delta() -> dx, dy`
unsafe extern "C" fn lua_mouse_delta(L: *mut ffi::lua_State) -> c_int {
    let (dx, dy) = with_input((0.0, 0.0), |i| (i.mouse_dx, i.mouse_dy));
    ffi::lua_pushnumber(L, dx);
    ffi::lua_pushnumber(L, dy);
    2
}

/// `input.is_mouse_button_pressed(button) -> bool` (button: "left"/"right"/"middle")
unsafe extern "C" fn lua_is_mouse_button_pressed(L: *mut ffi::lua_State) -> c_int {
    let pressed = match read_str_arg(L, 1) {
        Some(name) => with_input(false, |i| i.button_pressed(&name)),
        None => false,
    };
    ffi::lua_pushboolean(L, pressed as c_int);
    1
}

/// `input.scroll() -> number`
unsafe extern "C" fn lua_scroll(L: *mut ffi::lua_State) -> c_int {
    let scroll = with_input(0.0_f32, |i| i.scroll);
    ffi::lua_pushnumber(L, scroll as f64);
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guard so the thread-local pointer never leaks a dangling address between
    /// tests on the same thread.
    struct InputGuard;
    impl Drop for InputGuard {
        fn drop(&mut self) {
            set_current_input(std::ptr::null());
        }
    }

    #[test]
    fn reads_key_pressed_state() {
        let _g = InputGuard;
        let lua = LuaState::new();
        register_engine_input(&lua);

        let mut si = ScriptInput::new();
        si.set_key_pressed("W");
        set_current_input(&si as *const _);

        lua.exec(
            "result = 0
             if input.is_key_pressed('W') then result = result + 1 end
             if input.is_key_pressed('A') then result = result + 10 end",
        )
        .unwrap();
        assert_eq!(lua.get_global_number("result"), Some(1.0));
    }

    #[test]
    fn reads_mouse_position_and_buttons() {
        let _g = InputGuard;
        let lua = LuaState::new();
        register_engine_input(&lua);

        let mut si = ScriptInput::new();
        si.set_mouse(12.0, 34.0, 1.0, -2.0);
        si.set_buttons(true, false, false);
        si.set_scroll(3.5);
        set_current_input(&si as *const _);

        lua.exec(
            "mx, my = input.mouse_position()
             dx, dy = input.mouse_delta()
             left = input.is_mouse_button_pressed('left') and 1 or 0
             right = input.is_mouse_button_pressed('right') and 1 or 0
             sc = input.scroll()",
        )
        .unwrap();
        assert_eq!(lua.get_global_number("mx"), Some(12.0));
        assert_eq!(lua.get_global_number("my"), Some(34.0));
        assert_eq!(lua.get_global_number("dx"), Some(1.0));
        assert_eq!(lua.get_global_number("dy"), Some(-2.0));
        assert_eq!(lua.get_global_number("left"), Some(1.0));
        assert_eq!(lua.get_global_number("right"), Some(0.0));
        assert_eq!(lua.get_global_number("sc"), Some(3.5));
    }

    #[test]
    fn unknown_key_name_is_false_not_error() {
        let _g = InputGuard;
        let lua = LuaState::new();
        register_engine_input(&lua);
        let si = ScriptInput::new();
        set_current_input(&si as *const _);
        lua.exec("v = input.is_key_pressed('NotAKey') and 1 or 0").unwrap();
        assert_eq!(lua.get_global_number("v"), Some(0.0));
    }

    #[test]
    fn no_published_snapshot_returns_defaults() {
        let _g = InputGuard;
        set_current_input(std::ptr::null());
        let lua = LuaState::new();
        register_engine_input(&lua);
        // Must not panic / crash when nothing is published.
        lua.exec(
            "k = input.is_key_pressed('W') and 1 or 0
             mx, my = input.mouse_position()",
        )
        .unwrap();
        assert_eq!(lua.get_global_number("k"), Some(0.0));
        assert_eq!(lua.get_global_number("mx"), Some(0.0));
    }

    #[test]
    fn just_pressed_is_separate_from_held() {
        let _g = InputGuard;
        let lua = LuaState::new();
        register_engine_input(&lua);
        let mut si = ScriptInput::new();
        si.set_key_pressed("Space");
        si.set_key_just_pressed("Space");
        si.set_key_pressed("W"); // held but not a fresh edge
        set_current_input(&si as *const _);
        lua.exec(
            "space_just = input.is_key_just_pressed('Space') and 1 or 0
             w_just = input.is_key_just_pressed('W') and 1 or 0",
        )
        .unwrap();
        assert_eq!(lua.get_global_number("space_just"), Some(1.0));
        assert_eq!(lua.get_global_number("w_just"), Some(0.0));
    }
}
