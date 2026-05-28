#![allow(non_camel_case_types, non_snake_case)]
#![allow(missing_docs)] // 1:1 Lua C API bindings — lua.org's reference is canonical.

//! Raw Lua C API FFI bindings (`lua_*`/`luaL_*`/constants). See
//! <https://www.lua.org/manual/5.4/manual.html> for the upstream reference.

use std::os::raw::{c_char, c_int, c_void};

pub enum lua_State {}
pub type lua_Number = f64;
pub type lua_Integer = i64;
pub type lua_CFunction = unsafe extern "C" fn(*mut lua_State) -> c_int;

// Constants
pub(crate) const LUA_OK: c_int = 0;
pub(crate) const LUA_YIELD: c_int = 1;
pub(crate) const LUA_TNIL: c_int = 0;
pub(crate) const LUA_TBOOLEAN: c_int = 1;
pub(crate) const LUA_TNUMBER: c_int = 3;
pub(crate) const LUA_TSTRING: c_int = 4;
pub(crate) const LUA_TTABLE: c_int = 5;
pub(crate) const LUA_TFUNCTION: c_int = 6;
pub(crate) const LUA_MULTRET: c_int = -1;

extern "C" {
    pub(crate) fn luaL_newstate() -> *mut lua_State;
    pub(crate) fn luaL_openlibs(L: *mut lua_State);
    pub(crate) fn lua_close(L: *mut lua_State);

    // Execute
    pub(crate) fn luaL_loadstring(L: *mut lua_State, s: *const c_char) -> c_int;
    pub(crate) fn luaL_loadfilex(L: *mut lua_State, filename: *const c_char, mode: *const c_char) -> c_int;
    pub(crate) fn lua_pcallk(L: *mut lua_State, nargs: c_int, nresults: c_int, msgh: c_int, ctx: isize, k: *const c_void) -> c_int;

    // Stack operations
    pub(crate) fn lua_gettop(L: *mut lua_State) -> c_int;
    pub(crate) fn lua_settop(L: *mut lua_State, idx: c_int);
    pub(crate) fn lua_type(L: *mut lua_State, idx: c_int) -> c_int;
    pub(crate) fn lua_pushvalue(L: *mut lua_State, idx: c_int);

    // Push
    pub(crate) fn lua_pushnumber(L: *mut lua_State, n: lua_Number);
    pub(crate) fn lua_pushinteger(L: *mut lua_State, n: lua_Integer);
    pub(crate) fn lua_pushstring(L: *mut lua_State, s: *const c_char) -> *const c_char;
    pub(crate) fn lua_pushcclosure(L: *mut lua_State, f: lua_CFunction, n: c_int);
    pub(crate) fn lua_pushlightuserdata(L: *mut lua_State, p: *mut c_void);
    pub(crate) fn lua_pushboolean(L: *mut lua_State, b: c_int);
    pub(crate) fn lua_pushnil(L: *mut lua_State);

    // Get
    pub(crate) fn lua_tonumberx(L: *mut lua_State, idx: c_int, isnum: *mut c_int) -> lua_Number;
    pub(crate) fn lua_tolstring(L: *mut lua_State, idx: c_int, len: *mut usize) -> *const c_char;
    pub(crate) fn lua_touserdata(L: *mut lua_State, idx: c_int) -> *mut c_void;
    pub(crate) fn lua_toboolean(L: *mut lua_State, idx: c_int) -> c_int;
    pub(crate) fn lua_tointegerx(L: *mut lua_State, idx: c_int, isnum: *mut c_int) -> lua_Integer;

    // Table
    pub(crate) fn lua_createtable(L: *mut lua_State, narr: c_int, nrec: c_int);
    pub(crate) fn lua_setfield(L: *mut lua_State, idx: c_int, k: *const c_char);
    pub(crate) fn lua_getfield(L: *mut lua_State, idx: c_int, k: *const c_char) -> c_int;
    pub(crate) fn lua_next(L: *mut lua_State, idx: c_int) -> c_int;
    pub(crate) fn lua_rawgeti(L: *mut lua_State, idx: c_int, n: lua_Integer) -> c_int;
    pub(crate) fn lua_rawseti(L: *mut lua_State, idx: c_int, n: lua_Integer);

    // Globals
    pub(crate) fn lua_getglobal(L: *mut lua_State, name: *const c_char) -> c_int;
    pub(crate) fn lua_setglobal(L: *mut lua_State, name: *const c_char);

    // Coroutine
    pub(crate) fn lua_newthread(L: *mut lua_State) -> *mut lua_State;
    pub(crate) fn lua_resume(L: *mut lua_State, from: *mut lua_State, nargs: c_int, nresults: *mut c_int) -> c_int;
    pub(crate) fn lua_xmove(from: *mut lua_State, to: *mut lua_State, n: c_int);
    pub(crate) fn lua_status(L: *mut lua_State) -> c_int;
}

/// Helper: lua_pcall macro equivalent.
///
/// # Safety
/// `L` must be a valid `lua_State` pointer. See Lua reference for `lua_pcallk`.
pub(crate) unsafe fn lua_pcall(L: *mut lua_State, nargs: c_int, nresults: c_int, msgh: c_int) -> c_int {
    lua_pcallk(L, nargs, nresults, msgh, 0, std::ptr::null())
}

/// Helper: lua_pop.
///
/// # Safety
/// `L` must be a valid `lua_State` pointer and the stack must have at least `n` values.
pub(crate) unsafe fn lua_pop(L: *mut lua_State, n: c_int) {
    lua_settop(L, -n - 1);
}

/// Helper: lua_tostring (without length).
///
/// # Safety
/// `L` must be a valid `lua_State` pointer and the returned pointer is valid only
/// while the corresponding stack value remains untouched.
pub(crate) unsafe fn lua_tostring(L: *mut lua_State, idx: c_int) -> *const c_char {
    lua_tolstring(L, idx, std::ptr::null_mut())
}
