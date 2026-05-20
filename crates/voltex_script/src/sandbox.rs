use crate::state::LuaState;

/// List of dangerous globals to remove for sandboxing.
const BLOCKED_GLOBALS: &[&str] = &[
    "os",
    "io",
    "loadfile",
    "dofile",
    "require",
    "load",       // can load arbitrary bytecode
    "rawget",     // bypass metatables
    "rawset",     // bypass metatables
    "rawequal",
    "rawlen",
    "collectgarbage", // can manipulate GC
    "debug",      // full debug access
];

/// Apply sandboxing to a LuaState by removing dangerous globals.
/// Call this after `LuaState::new()` and before executing any user scripts.
///
/// Allowed: math, string, table, pairs, ipairs, print, type, tostring, tonumber,
/// pcall, xpcall, error, select, unpack, next, coroutine, assert
pub fn create_sandbox(state: &LuaState) -> crate::Result<()> {
    let mut code = String::new();
    for global in BLOCKED_GLOBALS {
        code.push_str(&format!("{} = nil\n", global));
    }
    state.exec(&code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::LuaState;

    #[test]
    fn test_os_execute_blocked() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        let result = lua.exec("os.execute('echo hello')");
        assert!(result.is_err(), "os.execute should be blocked");
    }

    #[test]
    fn test_io_blocked() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        let result = lua.exec("io.open('/etc/passwd', 'r')");
        assert!(result.is_err(), "io.open should be blocked");
    }

    #[test]
    fn test_loadfile_blocked() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        let result = lua.exec("loadfile('something.lua')");
        assert!(result.is_err(), "loadfile should be blocked");
    }

    #[test]
    fn test_dofile_blocked() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        let result = lua.exec("dofile('something.lua')");
        assert!(result.is_err(), "dofile should be blocked");
    }

    #[test]
    fn test_require_blocked() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        let result = lua.exec("require('os')");
        assert!(result.is_err(), "require should be blocked");
    }

    #[test]
    fn test_debug_blocked() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        let result = lua.exec("debug.getinfo(1)");
        assert!(result.is_err(), "debug should be blocked");
    }

    #[test]
    fn test_math_allowed() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        lua.exec("result = math.sin(0)").unwrap();
        assert_eq!(lua.get_global_number("result"), Some(0.0));
    }

    #[test]
    fn test_string_allowed() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        lua.exec("result = string.len('hello')").unwrap();
        assert_eq!(lua.get_global_number("result"), Some(5.0));
    }

    #[test]
    fn test_table_functions_allowed() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        lua.exec("
            t = {3, 1, 2}
            table.sort(t)
            result = t[1]
        ").unwrap();
        assert_eq!(lua.get_global_number("result"), Some(1.0));
    }

    #[test]
    fn test_pairs_ipairs_allowed() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        lua.exec("
            sum = 0
            for _, v in ipairs({1, 2, 3}) do sum = sum + v end
        ").unwrap();
        assert_eq!(lua.get_global_number("sum"), Some(6.0));
    }

    #[test]
    fn test_type_tostring_tonumber_allowed() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        lua.exec("
            t = type(42)
            n = tonumber('10')
            s = tostring(42)
        ").unwrap();
        assert_eq!(lua.get_global_string("t"), Some("number".to_string()));
        assert_eq!(lua.get_global_number("n"), Some(10.0));
        assert_eq!(lua.get_global_string("s"), Some("42".to_string()));
    }

    #[test]
    fn test_coroutine_allowed_after_sandbox() {
        let lua = LuaState::new();
        create_sandbox(&lua).unwrap();
        lua.exec("
            function coro() coroutine.yield() end
            co = coroutine.create(coro)
            coroutine.resume(co)
            status = coroutine.status(co)
        ").unwrap();
        assert_eq!(lua.get_global_string("status"), Some("suspended".to_string()));
    }
}
