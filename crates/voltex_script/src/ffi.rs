#![allow(non_camel_case_types, non_snake_case)]
use std::os::raw::{c_char, c_int, c_void};

pub enum lua_State {}
pub type lua_Number = f64;
pub type lua_Integer = i64;
pub type lua_CFunction = unsafe extern "C" fn(*mut lua_State) -> c_int;

// Constants
pub const LUA_OK: c_int = 0;
pub const LUA_TNUMBER: c_int = 3;
pub const LUA_TSTRING: c_int = 4;
pub const LUA_MULTRET: c_int = -1;

extern "C" {
    pub fn luaL_newstate() -> *mut lua_State;
    pub fn luaL_openlibs(L: *mut lua_State);
    pub fn lua_close(L: *mut lua_State);

    // Execute
    pub fn luaL_loadstring(L: *mut lua_State, s: *const c_char) -> c_int;
    pub fn luaL_loadfilex(L: *mut lua_State, filename: *const c_char, mode: *const c_char) -> c_int;
    pub fn lua_pcallk(L: *mut lua_State, nargs: c_int, nresults: c_int, msgh: c_int, ctx: isize, k: *const c_void) -> c_int;

    // Stack operations
    pub fn lua_gettop(L: *mut lua_State) -> c_int;
    pub fn lua_settop(L: *mut lua_State, idx: c_int);
    pub fn lua_type(L: *mut lua_State, idx: c_int) -> c_int;

    // Push
    pub fn lua_pushnumber(L: *mut lua_State, n: lua_Number);
    pub fn lua_pushstring(L: *mut lua_State, s: *const c_char) -> *const c_char;
    pub fn lua_pushcclosure(L: *mut lua_State, f: lua_CFunction, n: c_int);
    pub fn lua_pushlightuserdata(L: *mut lua_State, p: *mut c_void);

    // Get
    pub fn lua_tonumberx(L: *mut lua_State, idx: c_int, isnum: *mut c_int) -> lua_Number;
    pub fn lua_tolstring(L: *mut lua_State, idx: c_int, len: *mut usize) -> *const c_char;
    pub fn lua_touserdata(L: *mut lua_State, idx: c_int) -> *mut c_void;

    // Globals
    pub fn lua_getglobal(L: *mut lua_State, name: *const c_char) -> c_int;
    pub fn lua_setglobal(L: *mut lua_State, name: *const c_char);
}

// Helper: lua_pcall macro equivalent
pub unsafe fn lua_pcall(L: *mut lua_State, nargs: c_int, nresults: c_int, msgh: c_int) -> c_int {
    lua_pcallk(L, nargs, nresults, msgh, 0, std::ptr::null())
}

// Helper: lua_pop
pub unsafe fn lua_pop(L: *mut lua_State, n: c_int) {
    lua_settop(L, -n - 1);
}

// Helper: lua_tostring (without length)
pub unsafe fn lua_tostring(L: *mut lua_State, idx: c_int) -> *const c_char {
    lua_tolstring(L, idx, std::ptr::null_mut())
}
