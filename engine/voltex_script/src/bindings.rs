//! Default Lua bindings — `print` and other small host helpers.

use crate::ffi;
use crate::state::LuaState;
use std::ffi::CStr;

/// Register default engine bindings into a LuaState.
pub fn register_default_bindings(lua: &LuaState) {
    lua.register_fn("voltex_print", voltex_print);
}

unsafe extern "C" fn voltex_print(L: *mut ffi::lua_State) -> std::os::raw::c_int {
    let n = ffi::lua_gettop(L);
    for i in 1..=n {
        let ptr = ffi::lua_tostring(L, i);
        if !ptr.is_null() {
            let s = CStr::from_ptr(ptr).to_string_lossy();
            if i > 1 { print!("\t"); }
            print!("{}", s);
        }
    }
    println!();
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voltex_print() {
        let lua = LuaState::new();
        register_default_bindings(&lua);
        // Should not crash
        lua.exec("voltex_print('hello from Lua!')").unwrap();
    }
}
