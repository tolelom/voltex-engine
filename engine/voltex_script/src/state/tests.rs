//! Unit tests for [`super::LuaState`].

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
fn test_vec3_named_missing_field_is_error() {
    // x present but y/z missing → malformed vec3. The named path must not
    // silently coerce the missing fields to 0.0 (the array path already
    // rejects non-numeric elements; the named path was inconsistent).
    let lua = LuaState::new();
    lua.exec("v = { x = 1.0 }").unwrap();
    unsafe {
        crate::ffi::lua_getglobal(lua.state, c"v".as_ptr());
        let result = lua.read_vec3(-1);
        crate::ffi::lua_pop(lua.state, 1);
        assert!(result.is_err(), "vec3 missing y/z must error, got {result:?}");
    }
}

#[test]
fn test_vec3_named_non_numeric_field_is_error() {
    let lua = LuaState::new();
    lua.exec("v = { x = 1.0, y = 'foo', z = 3.0 }").unwrap();
    unsafe {
        crate::ffi::lua_getglobal(lua.state, c"v".as_ptr());
        let result = lua.read_vec3(-1);
        crate::ffi::lua_pop(lua.state, 1);
        assert!(result.is_err(), "vec3 with non-numeric y must error, got {result:?}");
    }
}

#[test]
fn test_vec3_named_error_leaves_stack_balanced() {
    // A malformed-vec3 read must not leak Lua stack slots — repeated reads
    // would otherwise overflow the stack over time.
    let lua = LuaState::new();
    lua.exec("v = { x = 1.0 }").unwrap();
    let before = unsafe { crate::ffi::lua_gettop(lua.state) };
    for _ in 0..50 {
        unsafe {
            crate::ffi::lua_getglobal(lua.state, c"v".as_ptr());
            let _ = lua.read_vec3(-1);
            crate::ffi::lua_pop(lua.state, 1);
        }
    }
    let after = unsafe { crate::ffi::lua_gettop(lua.state) };
    assert_eq!(before, after, "read_vec3 error path leaked stack slots");
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
