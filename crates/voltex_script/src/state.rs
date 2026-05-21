//! Owned Lua state wrapper + values that cross the FFI boundary.

use std::ffi::{CStr, CString};
use crate::ffi;

/// Represents a value that can be passed to/from Lua.
#[derive(Debug, Clone, PartialEq)]
pub enum LuaValue {
    /// Lua `nil`.
    Nil,
    /// Lua boolean.
    Bool(bool),
    /// Lua number (f64).
    Number(f64),
    /// Lua string.
    String(String),
    /// Lua table — key/value pairs, insertion order preserved.
    Table(Vec<(String, LuaValue)>),
}

/// Owns one `lua_State` (created by `luaL_newstate`).
pub struct LuaState {
    state: *mut ffi::lua_State,
}

unsafe impl Send for LuaState {}

impl Default for LuaState {
    fn default() -> Self {
        Self::new()
    }
}

impl LuaState {
    /// Creates a fresh Lua state with stdlibs opened.
    pub fn new() -> Self {
        unsafe {
            let state = ffi::luaL_newstate();
            assert!(!state.is_null(), "Failed to create Lua state");
            ffi::luaL_openlibs(state);
            Self { state }
        }
    }

    /// Execute a Lua code string.
    pub fn exec(&self, code: &str) -> crate::Result<()> {
        let c_code = CString::new(code).map_err(|e| crate::ScriptError::from(e.to_string()))?;
        unsafe {
            let result = ffi::luaL_loadstring(self.state, c_code.as_ptr());
            if result != ffi::LUA_OK {
                let err = self.pop_error();
                return Err(err.into());
            }
            let result = ffi::lua_pcall(self.state, 0, ffi::LUA_MULTRET, 0);
            if result != ffi::LUA_OK {
                let err = self.pop_error();
                return Err(err.into());
            }
            Ok(())
        }
    }

    /// Execute a Lua file.
    pub fn exec_file(&self, path: &str) -> crate::Result<()> {
        let c_path = CString::new(path).map_err(|e| crate::ScriptError::from(e.to_string()))?;
        unsafe {
            let result = ffi::luaL_loadfilex(self.state, c_path.as_ptr(), std::ptr::null());
            if result != ffi::LUA_OK {
                let err = self.pop_error();
                return Err(err.into());
            }
            let result = ffi::lua_pcall(self.state, 0, ffi::LUA_MULTRET, 0);
            if result != ffi::LUA_OK {
                let err = self.pop_error();
                return Err(err.into());
            }
            Ok(())
        }
    }

    /// Register a Rust function as a global Lua function.
    pub fn register_fn(&self, name: &str, f: ffi::lua_CFunction) {
        let c_name = CString::new(name).unwrap();
        unsafe {
            ffi::lua_pushcclosure(self.state, f, 0);
            ffi::lua_setglobal(self.state, c_name.as_ptr());
        }
    }

    /// Reads a global Lua number, or `None` if `name` is unset / non-numeric.
    pub fn get_global_number(&self, name: &str) -> Option<f64> {
        let c_name = CString::new(name).ok()?;
        unsafe {
            let ty = ffi::lua_getglobal(self.state, c_name.as_ptr());
            if ty != ffi::LUA_TNUMBER {
                ffi::lua_pop(self.state, 1);
                return None;
            }
            let mut isnum: std::os::raw::c_int = 0;
            let val = ffi::lua_tonumberx(self.state, -1, &mut isnum);
            ffi::lua_pop(self.state, 1);
            if isnum != 0 { Some(val) } else { None }
        }
    }

    /// Reads a global Lua string, or `None` if `name` is unset / non-string.
    pub fn get_global_string(&self, name: &str) -> Option<String> {
        let c_name = CString::new(name).ok()?;
        unsafe {
            let ty = ffi::lua_getglobal(self.state, c_name.as_ptr());
            if ty != ffi::LUA_TSTRING {
                ffi::lua_pop(self.state, 1);
                return None;
            }
            let ptr = ffi::lua_tostring(self.state, -1);
            let s = if ptr.is_null() { None } else { Some(CStr::from_ptr(ptr).to_string_lossy().into_owned()) };
            ffi::lua_pop(self.state, 1);
            s
        }
    }

    /// Writes a global Lua number.
    pub fn set_global_number(&self, name: &str, value: f64) {
        let c_name = CString::new(name).unwrap();
        unsafe {
            ffi::lua_pushnumber(self.state, value);
            ffi::lua_setglobal(self.state, c_name.as_ptr());
        }
    }

    /// Writes a global Lua string.
    pub fn set_global_string(&self, name: &str, value: &str) {
        let c_name = CString::new(name).unwrap();
        let c_value = CString::new(value).unwrap();
        unsafe {
            ffi::lua_pushstring(self.state, c_value.as_ptr());
            ffi::lua_setglobal(self.state, c_name.as_ptr());
        }
    }

    /// Push a LuaValue onto the Lua stack.
    pub fn push_value(&self, value: &LuaValue) {
        unsafe {
            match value {
                LuaValue::Nil => ffi::lua_pushnil(self.state),
                LuaValue::Bool(b) => ffi::lua_pushboolean(self.state, if *b { 1 } else { 0 }),
                LuaValue::Number(n) => ffi::lua_pushnumber(self.state, *n),
                LuaValue::String(s) => {
                    let cs = CString::new(s.as_str()).unwrap();
                    ffi::lua_pushstring(self.state, cs.as_ptr());
                }
                LuaValue::Table(pairs) => {
                    let borrowed: Vec<(&str, LuaValue)> = pairs.iter()
                        .map(|(k, v)| (k.as_str(), v.clone()))
                        .collect();
                    self.push_table(&borrowed);
                }
            }
        }
    }

    /// Push a Rust slice of key-value pairs as a Lua table onto the stack.
    pub fn push_table(&self, pairs: &[(&str, LuaValue)]) {
        unsafe {
            ffi::lua_createtable(self.state, 0, pairs.len() as i32);
            for (key, val) in pairs {
                self.push_value(val);
                let ckey = CString::new(*key).unwrap();
                ffi::lua_setfield(self.state, -2, ckey.as_ptr());
            }
        }
    }

    /// Read a Lua table at the given stack index into key-value pairs.
    pub fn read_table(&self, index: i32) -> crate::Result<Vec<(String, LuaValue)>> {
        unsafe {
            if ffi::lua_type(self.state, index) != ffi::LUA_TTABLE {
                return Err("expected table".into());
            }
            let abs_idx = if index < 0 {
                ffi::lua_gettop(self.state) + index + 1
            } else {
                index
            };
            let mut result = Vec::new();
            ffi::lua_pushnil(self.state); // first key
            while ffi::lua_next(self.state, abs_idx) != 0 {
                // key at -2, value at -1
                let key = self.read_stack_as_string(-2);
                let val = self.read_stack_value(-1);
                result.push((key, val));
                ffi::lua_pop(self.state, 1); // pop value, keep key for next iteration
            }
            Ok(result)
        }
    }

    /// Push a Vec3 as a Lua table {x=, y=, z=}.
    pub fn push_vec3(&self, v: [f32; 3]) {
        let pairs: Vec<(&str, LuaValue)> = vec![
            ("x", LuaValue::Number(v[0] as f64)),
            ("y", LuaValue::Number(v[1] as f64)),
            ("z", LuaValue::Number(v[2] as f64)),
        ];
        self.push_table(&pairs);
    }

    /// Read a Vec3 table from the stack. Supports `{x=, y=, z=}` named fields
    /// or `{[1]=, [2]=, [3]=}` array-style fields (escape the brackets to keep
    /// rustdoc happy).
    pub fn read_vec3(&self, index: i32) -> crate::Result<[f32; 3]> {
        unsafe {
            if ffi::lua_type(self.state, index) != ffi::LUA_TTABLE {
                return Err("expected table for vec3".into());
            }
            let abs_idx = if index < 0 {
                ffi::lua_gettop(self.state) + index + 1
            } else {
                index
            };

            // Try named fields first
            let cx = CString::new("x").unwrap();
            let cy = CString::new("y").unwrap();
            let cz = CString::new("z").unwrap();

            let tx = ffi::lua_getfield(self.state, abs_idx, cx.as_ptr());
            if tx == ffi::LUA_TNUMBER {
                let mut isnum = 0;
                let x = ffi::lua_tonumberx(self.state, -1, &mut isnum) as f32;
                ffi::lua_pop(self.state, 1);

                ffi::lua_getfield(self.state, abs_idx, cy.as_ptr());
                let y = ffi::lua_tonumberx(self.state, -1, &mut isnum) as f32;
                ffi::lua_pop(self.state, 1);

                ffi::lua_getfield(self.state, abs_idx, cz.as_ptr());
                let z = ffi::lua_tonumberx(self.state, -1, &mut isnum) as f32;
                ffi::lua_pop(self.state, 1);

                return Ok([x, y, z]);
            }
            ffi::lua_pop(self.state, 1);

            // Try array-style {v[1], v[2], v[3]}
            let mut vals = [0.0f32; 3];
            for (i, slot) in vals.iter_mut().enumerate() {
                ffi::lua_rawgeti(self.state, abs_idx, (i + 1) as ffi::lua_Integer);
                let mut isnum = 0;
                *slot = ffi::lua_tonumberx(self.state, -1, &mut isnum) as f32;
                ffi::lua_pop(self.state, 1);
                if isnum == 0 {
                    return Err("vec3 array element is not a number".into());
                }
            }
            Ok(vals)
        }
    }

    /// Read a value from the Lua stack at the given index.
    pub fn read_stack_value(&self, index: i32) -> LuaValue {
        unsafe {
            match ffi::lua_type(self.state, index) {
                ffi::LUA_TNIL => LuaValue::Nil,
                ffi::LUA_TBOOLEAN => {
                    LuaValue::Bool(ffi::lua_toboolean(self.state, index) != 0)
                }
                ffi::LUA_TNUMBER => {
                    let mut isnum = 0;
                    let n = ffi::lua_tonumberx(self.state, index, &mut isnum);
                    LuaValue::Number(n)
                }
                ffi::LUA_TSTRING => {
                    let ptr = ffi::lua_tostring(self.state, index);
                    if ptr.is_null() {
                        LuaValue::Nil
                    } else {
                        LuaValue::String(CStr::from_ptr(ptr).to_string_lossy().into_owned())
                    }
                }
                ffi::LUA_TTABLE => {
                    // Read recursively
                    self.read_table(index).map(LuaValue::Table).unwrap_or(LuaValue::Nil)
                }
                _ => LuaValue::Nil,
            }
        }
    }

    /// Read the stack value at index as a string key (for table iteration).
    fn read_stack_as_string(&self, index: i32) -> String {
        unsafe {
            match ffi::lua_type(self.state, index) {
                ffi::LUA_TSTRING => {
                    let ptr = ffi::lua_tostring(self.state, index);
                    if ptr.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(ptr).to_string_lossy().into_owned()
                    }
                }
                ffi::LUA_TNUMBER => {
                    let mut isnum = 0;
                    let n = ffi::lua_tonumberx(self.state, index, &mut isnum);
                    // Format integer keys without decimal
                    if n == (n as i64) as f64 {
                        format!("{}", n as i64)
                    } else {
                        format!("{}", n)
                    }
                }
                _ => String::new(),
            }
        }
    }

    /// Re-execute a Lua script file on the existing state (hot reload).
    pub fn reload_file(&mut self, path: &str) -> crate::Result<()> {
        self.exec_file(path)
    }

    /// Raw Lua state pointer (for advanced use).
    pub fn raw(&self) -> *mut ffi::lua_State {
        self.state
    }

    unsafe fn pop_error(&self) -> String {
        let ptr = ffi::lua_tostring(self.state, -1);
        let msg = if ptr.is_null() {
            "unknown Lua error".to_string()
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        };
        ffi::lua_pop(self.state, 1);
        msg
    }
}

impl Drop for LuaState {
    fn drop(&mut self) {
        unsafe { ffi::lua_close(self.state); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_drop() {
        let _lua = LuaState::new();
    }

    #[test]
    fn test_exec_simple() {
        let lua = LuaState::new();
        lua.exec("x = 1 + 2").unwrap();
        assert_eq!(lua.get_global_number("x"), Some(3.0));
    }

    #[test]
    fn test_exec_error() {
        let lua = LuaState::new();
        assert!(lua.exec("this is not valid lua!!!").is_err());
    }

    #[test]
    fn test_global_number_roundtrip() {
        let lua = LuaState::new();
        lua.set_global_number("myval", 42.5);
        assert_eq!(lua.get_global_number("myval"), Some(42.5));
    }

    #[test]
    fn test_global_string() {
        let lua = LuaState::new();
        lua.exec("greeting = 'hello from lua'").unwrap();
        assert_eq!(lua.get_global_string("greeting"), Some("hello from lua".to_string()));
    }

    #[test]
    fn test_global_string_roundtrip() {
        let lua = LuaState::new();
        lua.set_global_string("name", "voltex");
        assert_eq!(lua.get_global_string("name"), Some("voltex".to_string()));
    }

    #[test]
    fn test_missing_global() {
        let lua = LuaState::new();
        assert_eq!(lua.get_global_number("nonexistent"), None);
    }

    #[test]
    fn test_register_fn() {
        unsafe extern "C" fn lua_add_ten(L: *mut crate::ffi::lua_State) -> std::os::raw::c_int {
            let mut isnum = 0;
            let val = crate::ffi::lua_tonumberx(L, 1, &mut isnum);
            crate::ffi::lua_pushnumber(L, val + 10.0);
            1
        }
        let lua = LuaState::new();
        lua.register_fn("add_ten", lua_add_ten);
        lua.exec("result = add_ten(5)").unwrap();
        assert_eq!(lua.get_global_number("result"), Some(15.0));
    }

    #[test]
    fn test_push_read_table_roundtrip() {
        let lua = LuaState::new();
        let pairs: Vec<(&str, LuaValue)> = vec![
            ("name", LuaValue::String("test".into())),
            ("value", LuaValue::Number(42.0)),
            ("active", LuaValue::Bool(true)),
        ];
        lua.push_table(&pairs);
        unsafe { crate::ffi::lua_setglobal(lua.state, c"tbl".as_ptr()); }
        lua.exec("result_name = tbl.name; result_val = tbl.value").unwrap();
        assert_eq!(lua.get_global_string("result_name"), Some("test".to_string()));
        assert_eq!(lua.get_global_number("result_val"), Some(42.0));
    }

    #[test]
    fn test_read_table_from_lua() {
        let lua = LuaState::new();
        lua.exec("test_table = { greeting = 'hello', count = 7 }").unwrap();
        unsafe {
            let name = std::ffi::CString::new("test_table").unwrap();
            crate::ffi::lua_getglobal(lua.state, name.as_ptr());
            let table = lua.read_table(-1).unwrap();
            crate::ffi::lua_pop(lua.state, 1);

            let has_greeting = table.iter().any(|(k, v)| {
                k == "greeting" && *v == LuaValue::String("hello".into())
            });
            let has_count = table.iter().any(|(k, v)| {
                k == "count" && *v == LuaValue::Number(7.0)
            });
            assert!(has_greeting, "table should contain greeting='hello'");
            assert!(has_count, "table should contain count=7");
        }
    }

    #[test]
    fn test_nested_table() {
        let lua = LuaState::new();
        lua.exec("nested = { inner = { val = 99 } }").unwrap();
        unsafe {
            let name = std::ffi::CString::new("nested").unwrap();
            crate::ffi::lua_getglobal(lua.state, name.as_ptr());
            let table = lua.read_table(-1).unwrap();
            crate::ffi::lua_pop(lua.state, 1);

            let inner = table.iter().find(|(k, _)| k == "inner");
            assert!(inner.is_some());
            if let Some((_, LuaValue::Table(inner_pairs))) = inner {
                let has_val = inner_pairs.iter().any(|(k, v)| {
                    k == "val" && *v == LuaValue::Number(99.0)
                });
                assert!(has_val);
            } else {
                panic!("inner should be a table");
            }
        }
    }

    #[test]
    fn test_vec3_named_roundtrip() {
        let lua = LuaState::new();
        lua.push_vec3([1.0, 2.5, 3.0]);
        unsafe { crate::ffi::lua_setglobal(lua.state, c"v".as_ptr()); }

        // Read it back
        unsafe {
            crate::ffi::lua_getglobal(lua.state, c"v".as_ptr());
            let v = lua.read_vec3(-1).unwrap();
            crate::ffi::lua_pop(lua.state, 1);
            assert_eq!(v, [1.0, 2.5, 3.0]);
        }
    }

    #[test]
    fn test_vec3_from_lua_named() {
        let lua = LuaState::new();
        lua.exec("v = { x = 10, y = 20, z = 30 }").unwrap();
        unsafe {
            crate::ffi::lua_getglobal(lua.state, c"v".as_ptr());
            let v = lua.read_vec3(-1).unwrap();
            crate::ffi::lua_pop(lua.state, 1);
            assert_eq!(v, [10.0, 20.0, 30.0]);
        }
    }

    #[test]
    fn test_vec3_from_lua_array() {
        let lua = LuaState::new();
        lua.exec("v = { 4, 5, 6 }").unwrap();
        unsafe {
            crate::ffi::lua_getglobal(lua.state, c"v".as_ptr());
            let v = lua.read_vec3(-1).unwrap();
            crate::ffi::lua_pop(lua.state, 1);
            assert_eq!(v, [4.0, 5.0, 6.0]);
        }
    }

    #[test]
    fn test_hot_reload() {
        let dir = std::env::temp_dir().join("voltex_script_reload_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("reload_test.lua");

        // Initial script
        std::fs::write(&path, "counter = 1").unwrap();
        let mut lua = LuaState::new();
        lua.exec_file(path.to_str().unwrap()).unwrap();
        assert_eq!(lua.get_global_number("counter"), Some(1.0));

        // Modified script
        std::fs::write(&path, "counter = counter + 10").unwrap();
        lua.reload_file(path.to_str().unwrap()).unwrap();
        assert_eq!(lua.get_global_number("counter"), Some(11.0));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
