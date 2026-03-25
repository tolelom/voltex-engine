use std::ffi::{CStr, CString};
use crate::ffi;

pub struct LuaState {
    state: *mut ffi::lua_State,
}

unsafe impl Send for LuaState {}

impl LuaState {
    pub fn new() -> Self {
        unsafe {
            let state = ffi::luaL_newstate();
            assert!(!state.is_null(), "Failed to create Lua state");
            ffi::luaL_openlibs(state);
            Self { state }
        }
    }

    /// Execute a Lua code string.
    pub fn exec(&self, code: &str) -> Result<(), String> {
        let c_code = CString::new(code).map_err(|e| e.to_string())?;
        unsafe {
            let result = ffi::luaL_loadstring(self.state, c_code.as_ptr());
            if result != ffi::LUA_OK {
                let err = self.pop_error();
                return Err(err);
            }
            let result = ffi::lua_pcall(self.state, 0, ffi::LUA_MULTRET, 0);
            if result != ffi::LUA_OK {
                let err = self.pop_error();
                return Err(err);
            }
            Ok(())
        }
    }

    /// Execute a Lua file.
    pub fn exec_file(&self, path: &str) -> Result<(), String> {
        let c_path = CString::new(path).map_err(|e| e.to_string())?;
        unsafe {
            let result = ffi::luaL_loadfilex(self.state, c_path.as_ptr(), std::ptr::null());
            if result != ffi::LUA_OK {
                let err = self.pop_error();
                return Err(err);
            }
            let result = ffi::lua_pcall(self.state, 0, ffi::LUA_MULTRET, 0);
            if result != ffi::LUA_OK {
                let err = self.pop_error();
                return Err(err);
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

    pub fn set_global_number(&self, name: &str, value: f64) {
        let c_name = CString::new(name).unwrap();
        unsafe {
            ffi::lua_pushnumber(self.state, value);
            ffi::lua_setglobal(self.state, c_name.as_ptr());
        }
    }

    pub fn set_global_string(&self, name: &str, value: &str) {
        let c_name = CString::new(name).unwrap();
        let c_value = CString::new(value).unwrap();
        unsafe {
            ffi::lua_pushstring(self.state, c_value.as_ptr());
            ffi::lua_setglobal(self.state, c_name.as_ptr());
        }
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
}
