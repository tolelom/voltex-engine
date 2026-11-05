//! Lua coroutines over `lua_newthread`: create from a global function name,
//! resume until finished. For multi-frame script logic (cutscenes, staged AI)
//! that yields between engine ticks.

use crate::ffi;
use std::ffi::CString;

/// Status returned by a coroutine resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroutineStatus {
    /// The coroutine yielded and can be resumed again.
    Yielded,
    /// The coroutine ran to completion — do not resume again.
    Finished,
}

/// A Lua coroutine backed by a lua_newthread.
///
/// Borrows the [`LuaState`](crate::state::LuaState) it was created from, so it
/// can't outlive it. The thread is kept alive by a registry reference that is
/// released when the coroutine is dropped.
pub struct LuaCoroutine<'lua> {
    /// The coroutine thread state.
    state: *mut ffi::lua_State,
    /// The state that owns the thread (where the registry reference lives).
    parent: *mut ffi::lua_State,
    /// `luaL_ref` handle that keeps the thread from being garbage-collected.
    thread_ref: std::os::raw::c_int,
    /// Whether the coroutine has finished execution.
    finished: bool,
    _lua: std::marker::PhantomData<&'lua crate::state::LuaState>,
}

impl crate::state::LuaState {
    /// Create a coroutine from a global Lua function name.
    /// The function must already be defined in the Lua state.
    pub fn create_coroutine(&self, func_name: &str) -> crate::Result<LuaCoroutine<'_>> {
        let c_name = CString::new(func_name).map_err(|e| crate::ScriptError::from(e.to_string()))?;
        // SAFETY: `self.raw()` is the live state owned by `self`. `lua_newthread`
        // leaves the new thread on the main stack, which is its only anchor;
        // `luaL_ref` pops it into the registry so the GC keeps it alive until
        // `LuaCoroutine::drop` unrefs it. The `'lua` borrow keeps `self` (and so
        // both pointers) alive for the coroutine's whole life. `c_name` outlives
        // `lua_getglobal`.
        unsafe {
            let parent = self.raw();
            let co_state = ffi::lua_newthread(parent);
            if co_state.is_null() {
                return Err("failed to create Lua thread".into());
            }
            let thread_ref = ffi::luaL_ref(parent, ffi::LUA_REGISTRYINDEX);
            let coroutine = LuaCoroutine {
                state: co_state,
                parent,
                thread_ref,
                finished: false,
                _lua: std::marker::PhantomData,
            };

            // Push the function onto the coroutine stack
            let ty = ffi::lua_getglobal(co_state, c_name.as_ptr());
            if ty != ffi::LUA_TFUNCTION {
                ffi::lua_pop(co_state, 1);
                return Err(format!("'{}' is not a Lua function", func_name).into());
            }
            Ok(coroutine)
        }
    }
}

impl Drop for LuaCoroutine<'_> {
    fn drop(&mut self) {
        // SAFETY: `parent` is alive (the `'lua` borrow) and `thread_ref` was
        // returned by `luaL_ref` on its registry and is released exactly once.
        unsafe { ffi::luaL_unref(self.parent, ffi::LUA_REGISTRYINDEX, self.thread_ref) };
    }
}

impl LuaCoroutine<'_> {
    /// Resume the coroutine. Returns Yielded if the coroutine yielded,
    /// or Finished if it completed.
    pub fn resume(&mut self) -> crate::Result<CoroutineStatus> {
        if self.finished {
            return Err("coroutine already finished".into());
        }
        // SAFETY: `self.state` is a live Lua thread: the registry reference
        // taken in `create_coroutine` keeps it alive. `lua_resume` reports `nresults` values on the
        // thread's stack, which are popped; on error the message on top is
        // copied before it is popped.
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
                    Err(msg.into())
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

    #[test]
    fn coroutine_survives_a_full_garbage_collection() {
        let lua = LuaState::new();
        lua.exec("
            function counter()
                for i = 1, 3 do
                    count = i
                    coroutine.yield()
                end
            end
        ").unwrap();
        let mut co = lua.create_coroutine("counter").unwrap();
        assert_eq!(co.resume().unwrap(), CoroutineStatus::Yielded);
        // Before the fix the thread had no anchor once popped, so a full
        // collection could free it while `co` still pointed at it.
        lua.exec("collectgarbage('collect'); collectgarbage('collect')").unwrap();
        assert_eq!(co.resume().unwrap(), CoroutineStatus::Yielded);
        assert_eq!(lua.get_global_number("count"), Some(2.0));
    }

    #[test]
    fn dropping_a_coroutine_releases_its_registry_reference() {
        let lua = LuaState::new();
        lua.exec("function noop() end").unwrap();
        // Many short-lived coroutines must not grow the registry without bound:
        // luaL_unref puts each slot on the free list and the next ref reuses it.
        let first_ref = lua.create_coroutine("noop").unwrap().thread_ref;
        for _ in 0..100 {
            let co = lua.create_coroutine("noop").unwrap();
            assert_eq!(co.thread_ref, first_ref, "registry slot should be reused after drop");
        }
    }
}

