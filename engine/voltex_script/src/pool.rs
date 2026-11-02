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
use crate::engine_input::register_engine_input;
use crate::instance::ScriptInstance;
use crate::state::LuaState;

/// Runtime store of one [`ScriptInstance`] per `.lua` path. Each instance is
/// identified by a stable `u32` cached on the entity's `ScriptComponent`.
pub struct ScriptInstancePool {
    instances: HashMap<u32, ScriptInstance>,
    path_to_id: HashMap<String, u32>,
    /// Vfs 등에서 시드된 스크립트 소스 (키: forward-slash 정규화 경로).
    /// [`Self::get_or_create`]가 파일시스템보다 먼저 조회 — `.vpkg`에 담긴
    /// 스크립트가 디스크에 루즈 파일 없이 동작하는 경로.
    sources: HashMap<String, String>,
    /// 로드에 실패한 경로 — 프레임마다 재시도/재로그하지 않기 위한 캐시.
    failed: std::collections::HashSet<String>,
    next_id: u32,
}

/// 소스/경로 맵 키 정규화 (vpkg 키 규약과 동일: forward slash).
fn norm_key(path: &str) -> String {
    path.replace('\\', "/")
}

impl ScriptInstancePool {
    /// Creates an empty pool.
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            path_to_id: HashMap::new(),
            sources: HashMap::new(),
            failed: std::collections::HashSet::new(),
            next_id: 0,
        }
    }

    /// Pre-bind `path` to an in-memory script source. `get_or_create` will
    /// execute this instead of reading the filesystem — the runtime seeds
    /// every `.lua` entry of its Vfs archive through here so `.vpkg`-shipped
    /// games run without loose script files (audit fix: scripts were the only
    /// asset class that bypassed the Vfs).
    pub fn seed_source(&mut self, path: impl Into<String>, source: impl Into<String>) {
        self.sources.insert(norm_key(&path.into()), source.into());
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
        let key = norm_key(path);
        if let Some(&id) = self.path_to_id.get(&key) {
            return Ok(id);
        }
        if self.failed.contains(&key) {
            return Err(format!("script previously failed to load: {path}").into());
        }
        let lua = LuaState::new();
        register_default_bindings(&lua);
        register_engine_api(&lua, world);
        // The global `input` table reads a thread-local snapshot published by
        // the runtime each frame (`set_current_input`); harmless if unset.
        register_engine_input(&lua);
        // 시드된 소스(Vfs) 우선, 없으면 파일시스템.
        let loaded = match self.sources.get(&key) {
            Some(src) => lua.exec(src),
            None => lua.exec_file(path),
        };
        if let Err(e) = loaded {
            // 1회 로깅 + 실패 캐시 — 이전엔 호출부(system.rs)가 조용히
            // continue 해서 스크립트 부재가 무진단 무동작이었다.
            eprintln!("voltex_script: failed to load `{path}`: {e}");
            self.failed.insert(key);
            return Err(e);
        }
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
        self.path_to_id.insert(key, id);
        Ok(id)
    }

    /// Borrows the instance at `id`, if any.
    pub fn get(&self, id: u32) -> Option<&ScriptInstance> {
        self.instances.get(&id)
    }

    /// Mutable equivalent of [`get`](Self::get).
    pub fn get_mut(&mut self, id: u32) -> Option<&mut ScriptInstance> {
        self.instances.get_mut(&id)
    }

    /// Looks up the id assigned to `path`, if any.
    pub fn find_by_path(&self, path: &str) -> Option<u32> {
        self.path_to_id.get(&norm_key(path)).copied()
    }

    /// Number of loaded script instances.
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// `true` if no scripts are loaded.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Re-exec the file on the existing LuaState. Marks `started = false` so
    /// the next `script_start_all` re-runs `on_start`. Returns `Ok(true)` if
    /// the instance existed, `Ok(false)` if the path was unknown to the pool.
    pub fn reload(&mut self, path: &str) -> crate::Result<bool> {
        let Some(&id) = self.path_to_id.get(&norm_key(path)) else {
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
    fn seeded_source_loads_without_a_file_on_disk() {
        let mut world = World::new();
        let world_ptr = &mut world as *mut World;
        let mut pool = ScriptInstancePool::new();
        // 디스크에 존재하지 않는 경로 — .vpkg 시나리오.
        pool.seed_source("scripts/ghost.lua", "x = 42");
        let id = pool.get_or_create("scripts/ghost.lua", world_ptr).unwrap();
        assert!(pool.get(id).is_some());
        // 백슬래시 표기도 같은 시드에 히트 (키 정규화).
        let id2 = pool.get_or_create("scripts\\ghost.lua", world_ptr).unwrap();
        assert_eq!(id, id2);
    }

    #[test]
    fn failed_load_is_cached_and_does_not_retry() {
        let mut world = World::new();
        let world_ptr = &mut world as *mut World;
        let mut pool = ScriptInstancePool::new();
        assert!(pool.get_or_create("scripts/nope.lua", world_ptr).is_err());
        // 두 번째는 실패 캐시에서 즉시 반환 (프레임당 fs 재시도/재로그 없음).
        assert!(pool.get_or_create("scripts/nope.lua", world_ptr).is_err());
        assert_eq!(pool.len(), 0);
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
    fn poll_reload_picks_up_file_changes() {
        use std::time::Duration;
        let mut world = World::new();
        let world_ptr = &mut world as *mut World;
        // Unique filename so other test runs don't trigger spurious watcher changes.
        let path = write_tmp_lua("watched_unique_abc.lua", "value = 10");

        let mut pool = ScriptInstancePool::new();
        let id = pool.get_or_create(path.to_str().unwrap(), world_ptr).unwrap();
        pool.get_mut(id).unwrap().started = true;

        let mut watcher = voltex_asset::FileWatcher::new(Duration::from_millis(0));
        ScriptInstancePool::watch(path.to_str().unwrap(), &mut watcher);
        // First poll registers the initial mtime — must not report a change.
        let _ = pool.poll_reload(&mut watcher);
        assert!(pool.get(id).unwrap().started);

        // Touch the file with new contents so the watcher detects a change.
        std::thread::sleep(Duration::from_millis(50));
        std::fs::write(&path, "value = 99").unwrap();

        let reloaded = pool.poll_reload(&mut watcher);
        assert_eq!(reloaded.len(), 1);
        let inst = pool.get(id).unwrap();
        // Reload re-executes the file on the same LuaState and clears `started`.
        assert!(!inst.started);
        assert_eq!(inst.lua.get_global_number("value"), Some(99.0));
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
