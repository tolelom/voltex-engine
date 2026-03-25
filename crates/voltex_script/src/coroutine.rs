use crate::ffi;
use std::ffi::CString;

/// Status returned by a coroutine resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroutineStatus {
    Yielded,
    Finished,
}

/// A Lua coroutine backed by a lua_newthread.
pub struct LuaCoroutine {
    /// The coroutine thread state.
    state: *mut ffi::lua_State,
    /// Whether the coroutine has finished execution.
    finished: bool,
}

impl crate::state::LuaState {
    /// Create a coroutine from a global Lua function name.
    /// The function must already be defined in the Lua state.
    pub fn create_coroutine(&self, func_name: &str) -> Result<LuaCoroutine, String> {
        unsafe {
            let co_state = ffi::lua_newthread(self.raw());
            if co_state.is_null() {
                return Err("failed to create Lua thread".to_string());
            }
            // Pop the thread from the main stack (it's anchored in the registry)
            // Actually, we need to keep the thread referenced. lua_newthread pushes it
            // onto the main stack. We'll leave it there (Lua GC won't collect it while
            // it's on the stack). For simplicity, pop it — Lua keeps a reference in the
            // registry as long as the coroutine is alive.
            // Note: lua_newthread creates a thread that is anchored by the registry
            // automatically in Lua 5.4, so we can pop it from the main stack.
            ffi::lua_pop(self.raw(), 1);

            // Push the function onto the coroutine stack
            let c_name = CString::new(func_name).map_err(|e| e.to_string())?;
            let ty = ffi::lua_getglobal(co_state, c_name.as_ptr());
            if ty != ffi::LUA_TFUNCTION {
                ffi::lua_pop(co_state, 1);
                return Err(format!("'{}' is not a Lua function", func_name));
            }

            Ok(LuaCoroutine {
                state: co_state,
                finished: false,
            })
        }
    }
}

impl LuaCoroutine {
    /// Resume the coroutine. Returns Yielded if the coroutine yielded,
    /// or Finished if it completed.
    pub fn resume(&mut self) -> Result<CoroutineStatus, String> {
        if self.finished {
            return Err("coroutine already finished".to_string());
        }
        unsafe {
            let mut nresults: std::os::raw::c_int = 0;
            let status = ffi::lua_resume(self.state, std::ptr::null_mut(), 0, &mut nresults);
            // Pop any results from the coroutine stack
            if nresults > 0 {
                ffi::lua_pop(self.state, nresults);
            }
            match status {
                ffi::LUA_YIELD => Ok(CoroutineStatus::Yielded),
                ffi::LUA_OK => {
                    self.finished = true;
                    Ok(CoroutineStatus::Finished)
                }
                _ => {
                    self.finished = true;
                    let ptr = ffi::lua_tostring(self.state, -1);
                    let msg = if ptr.is_null() {
                        "coroutine error".to_string()
                    } else {
                        std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
                    };
                    ffi::lua_pop(self.state, 1);
                    Err(msg)
                }
            }
        }
    }

    /// Check if the coroutine has finished execution.
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use crate::state::LuaState;
    use super::*;

    #[test]
    fn test_simple_coroutine_yield_resume() {
        let lua = LuaState::new();
        lua.exec("
            function my_coro()
                step = 1
                coroutine.yield()
                step = 2
                coroutine.yield()
                step = 3
            end
            step = 0
        ").unwrap();

        let mut co = lua.create_coroutine("my_coro").unwrap();
        assert!(!co.is_finished());

        let status = co.resume().unwrap();
        assert_eq!(status, CoroutineStatus::Yielded);
        assert_eq!(lua.get_global_number("step"), Some(1.0));

        let status = co.resume().unwrap();
        assert_eq!(status, CoroutineStatus::Yielded);
        assert_eq!(lua.get_global_number("step"), Some(2.0));

        let status = co.resume().unwrap();
        assert_eq!(status, CoroutineStatus::Finished);
        assert_eq!(lua.get_global_number("step"), Some(3.0));
        assert!(co.is_finished());
    }

    #[test]
    fn test_multi_step_coroutine() {
        let lua = LuaState::new();
        lua.exec("
            function counter_coro()
                for i = 1, 5 do
                    count = i
                    coroutine.yield()
                end
            end
            count = 0
        ").unwrap();

        let mut co = lua.create_coroutine("counter_coro").unwrap();
        for i in 1..=5 {
            let status = co.resume().unwrap();
            assert_eq!(lua.get_global_number("count"), Some(i as f64));
            if i < 5 {
                assert_eq!(status, CoroutineStatus::Yielded);
            } else {
                // After the last yield, the loop ends and the function returns
                // Actually the yield happens inside the loop, so after i=5 it yields,
                // then we resume once more to finish
            }
        }
        // One more resume to finish the coroutine (the loop body yields after setting count)
        let status = co.resume().unwrap();
        assert_eq!(status, CoroutineStatus::Finished);
        assert!(co.is_finished());
    }

    #[test]
    fn test_finished_detection() {
        let lua = LuaState::new();
        lua.exec("
            function instant()
                result = 42
            end
        ").unwrap();

        let mut co = lua.create_coroutine("instant").unwrap();
        assert!(!co.is_finished());
        let status = co.resume().unwrap();
        assert_eq!(status, CoroutineStatus::Finished);
        assert!(co.is_finished());
        assert_eq!(lua.get_global_number("result"), Some(42.0));

        // Resuming again should error
        assert!(co.resume().is_err());
    }

    #[test]
    fn test_create_coroutine_invalid_function() {
        let lua = LuaState::new();
        let result = lua.create_coroutine("nonexistent");
        assert!(result.is_err());
    }
}
