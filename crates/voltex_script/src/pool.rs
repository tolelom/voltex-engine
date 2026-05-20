//! `ScriptInstancePool` — runtime store for the LuaState behind every
//! `ScriptComponent::script_path` value seen at runtime.
//!
//! One LuaState per *path* (not per entity). Multiple entities pointing at
//! the same `.lua` share the interpreter and engine_api bindings. Each entry
//! is assigned a stable `u32` id which `ScriptComponent::instance_id` caches
//! so subsequent updates skip the path lookup.

use std::collections::HashMap;
use std::path::PathBuf;
use voltex_ecs::World;

use crate::bindings::register_default_bindings;
use crate::engine_api::register_engine_api;
use crate::instance::ScriptInstance;
use crate::state::LuaState;

pub struct ScriptInstancePool {
    instances: HashMap<u32, ScriptInstance>,
    path_to_id: HashMap<String, u32>,
    next_id: u32,
}

impl ScriptInstancePool {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            path_to_id: HashMap::new(),
            next_id: 0,
        }
    }

    /// Return the instance id for `path`. On first encounter, create a new
    /// LuaState, register the engine API + default bindings against `world`,
    /// and execute the file. Subsequent calls return the cached id without
    /// re-loading.
    ///
    /// # Safety
    /// `world` must remain valid for the lifetime of every script call made
    /// against this LuaState. In practice, the pool and the World are owned
    /// by the same outer struct (EditorSession, RuntimeApp) and dropped in
    /// pool-before-world order.
    pub fn get_or_create(&mut self, path: &str, world: *mut World) -> crate::Result<u32> {
        if let Some(&id) = self.path_to_id.get(path) {
            return Ok(id);
        }
        let lua = LuaState::new();
        register_default_bindings(&lua);
        register_engine_api(&lua, world);
        lua.exec_file(path)?;
        let id = self.next_id;
        self.next_id += 1;
        self.instances.insert(
            id,
            ScriptInstance {
                lua,
                path: path.to_string(),
                started: false,
            },
        );
        self.path_to_id.insert(path.to_string(), id);
        Ok(id)
    }

    pub fn get(&self, id: u32) -> Option<&ScriptInstance> {
        self.instances.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut ScriptInstance> {
        self.instances.get_mut(&id)
    }

    pub fn find_by_path(&self, path: &str) -> Option<u32> {
        self.path_to_id.get(path).copied()
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Re-exec the file on the existing LuaState. Marks `started = false` so
    /// the next `script_start_all` re-runs `on_start`. Returns `Ok(true)` if
    /// the instance existed, `Ok(false)` if the path was unknown to the pool.
    pub fn reload(&mut self, path: &str) -> crate::Result<bool> {
        let Some(&id) = self.path_to_id.get(path) else {
            return Ok(false);
        };
        let Some(inst) = self.instances.get_mut(&id) else {
            return Ok(false);
        };
        inst.lua.reload_file(path)?;
        inst.started = false;
        Ok(true)
    }

    /// Drop every instance. Used by the editor's PlayStop lifecycle and by
    /// `RuntimeApp` shutdown.
    pub fn clear(&mut self) {
        self.instances.clear();
        self.path_to_id.clear();
        self.next_id = 0;
    }

    /// Poll a `FileWatcher` and reload any tracked script whose mtime changed.
    /// Silent on per-path errors — the watcher entry is consumed regardless,
    /// matching the watcher's "report once per change" semantics. Returns the
    /// list of paths that were successfully reloaded.
    pub fn poll_reload(&mut self, watcher: &mut voltex_asset::FileWatcher) -> Vec<String> {
        let mut reloaded = Vec::new();
        for path in watcher.poll_changes() {
            let p = path.to_string_lossy().to_string();
            if matches!(self.reload(&p), Ok(true)) {
                reloaded.push(p);
            }
        }
        reloaded
    }

    /// Register a path with the given watcher. Convenience pass-through so
    /// callers don't need to construct `PathBuf` themselves.
    pub fn watch(path: &str, watcher: &mut voltex_asset::FileWatcher) {
        watcher.watch(PathBuf::from(path));
    }
}

impl Default for ScriptInstancePool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp_lua(name: &str, body: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("voltex_script_pool_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn get_or_create_same_path_returns_same_id() {
        let mut world = World::new();
        let world_ptr = &mut world as *mut World;
        let path = write_tmp_lua("same.lua", "x = 1");

        let mut pool = ScriptInstancePool::new();
        let id1 = pool.get_or_create(path.to_str().unwrap(), world_ptr).unwrap();
        let id2 = pool.get_or_create(path.to_str().unwrap(), world_ptr).unwrap();
        assert_eq!(id1, id2);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn get_or_create_loads_lua_file() {
        let mut world = World::new();
        let world_ptr = &mut world as *mut World;
        let path = write_tmp_lua("loads.lua", "loaded_marker = 7");

        let mut pool = ScriptInstancePool::new();
        let id = pool.get_or_create(path.to_str().unwrap(), world_ptr).unwrap();
        let inst = pool.get(id).expect("instance present");
        assert_eq!(inst.lua.get_global_number("loaded_marker"), Some(7.0));
        assert!(!inst.started);
    }

    #[test]
    fn reload_resets_started_flag() {
        let mut world = World::new();
        let world_ptr = &mut world as *mut World;
        let path = write_tmp_lua("reload.lua", "v = 1");

        let mut pool = ScriptInstancePool::new();
        let id = pool.get_or_create(path.to_str().unwrap(), world_ptr).unwrap();
        pool.get_mut(id).unwrap().started = true;

        std::fs::write(&path, "v = 2").unwrap();
        let reloaded = pool.reload(path.to_str().unwrap()).unwrap();
        assert!(reloaded);
        let inst = pool.get(id).unwrap();
        assert!(!inst.started);
        assert_eq!(inst.lua.get_global_number("v"), Some(2.0));
    }

    #[test]
    fn reload_unknown_path_is_false() {
        let mut pool = ScriptInstancePool::new();
        let result = pool.reload("/no/such/path.lua").unwrap();
        assert!(!result);
    }

    #[test]
    fn clear_empties_pool() {
        let mut world = World::new();
        let world_ptr = &mut world as *mut World;
        let path = write_tmp_lua("clear.lua", "noop = 0");

        let mut pool = ScriptInstancePool::new();
        let _ = pool.get_or_create(path.to_str().unwrap(), world_ptr).unwrap();
        assert_eq!(pool.len(), 1);
        pool.clear();
        assert!(pool.is_empty());
        assert!(pool.find_by_path(path.to_str().unwrap()).is_none());
    }
}
