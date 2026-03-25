# Async Loading + Hot Reload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add background asset loading via worker thread and file-change-based hot reload to voltex_asset.

**Architecture:** AssetLoader spawns one worker thread, communicates via channels. FileWatcher polls `std::fs::metadata` for mtime changes. Both are independent modules.

**Tech Stack:** Pure Rust std library (threads, channels, fs). No external crates.

---

### Task 1: FileWatcher (mtime polling)

**Files:**
- Create: `crates/voltex_asset/src/watcher.rs`
- Modify: `crates/voltex_asset/src/lib.rs`

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_watch_and_poll_no_changes() {
        let mut watcher = FileWatcher::new(Duration::from_millis(0));
        let dir = std::env::temp_dir().join("voltex_watcher_test_1");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test.txt");
        fs::write(&path, "hello").unwrap();

        watcher.watch(path.clone());
        // First poll — should not report as changed (just registered)
        let changes = watcher.poll_changes();
        assert!(changes.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_file_change() {
        let dir = std::env::temp_dir().join("voltex_watcher_test_2");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test2.txt");
        fs::write(&path, "v1").unwrap();

        let mut watcher = FileWatcher::new(Duration::from_millis(0));
        watcher.watch(path.clone());
        let _ = watcher.poll_changes(); // register initial mtime

        // Modify file
        std::thread::sleep(Duration::from_millis(50));
        fs::write(&path, "v2 with more data").unwrap();

        let changes = watcher.poll_changes();
        assert!(changes.contains(&path));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_unwatch() {
        let mut watcher = FileWatcher::new(Duration::from_millis(0));
        let path = PathBuf::from("/nonexistent/test.txt");
        watcher.watch(path.clone());
        watcher.unwatch(&path);
        assert!(watcher.poll_changes().is_empty());
    }
}
```

- [ ] **Step 2: Implement FileWatcher**

```rust
// crates/voltex_asset/src/watcher.rs
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

pub struct FileWatcher {
    watched: HashMap<PathBuf, Option<SystemTime>>,
    poll_interval: Duration,
    last_poll: Instant,
}

impl FileWatcher {
    pub fn new(poll_interval: Duration) -> Self {
        Self {
            watched: HashMap::new(),
            poll_interval,
            last_poll: Instant::now(),
        }
    }

    pub fn watch(&mut self, path: PathBuf) {
        let mtime = std::fs::metadata(&path).ok()
            .and_then(|m| m.modified().ok());
        self.watched.insert(path, mtime);
    }

    pub fn unwatch(&mut self, path: &Path) {
        self.watched.remove(path);
    }

    pub fn poll_changes(&mut self) -> Vec<PathBuf> {
        let now = Instant::now();
        if now.duration_since(self.last_poll) < self.poll_interval {
            return Vec::new();
        }
        self.last_poll = now;

        let mut changed = Vec::new();
        for (path, last_mtime) in &mut self.watched {
            let current = std::fs::metadata(path).ok()
                .and_then(|m| m.modified().ok());
            if current != *last_mtime && last_mtime.is_some() {
                changed.push(path.clone());
            }
            *last_mtime = current;
        }
        changed
    }

    pub fn watched_count(&self) -> usize {
        self.watched.len()
    }
}
```

- [ ] **Step 3: Run tests, commit**

```bash
cargo test --package voltex_asset -- watcher::tests -v
git add crates/voltex_asset/src/watcher.rs crates/voltex_asset/src/lib.rs
git commit -m "feat(asset): add FileWatcher with mtime-based change detection"
```

---

### Task 2: AssetLoader (background thread)

**Files:**
- Create: `crates/voltex_asset/src/loader.rs`
- Modify: `crates/voltex_asset/src/lib.rs`

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_state_initial() {
        let mut loader = AssetLoader::new();
        let dir = std::env::temp_dir().join("voltex_loader_test_1");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test.txt");
        fs::write(&path, "hello world").unwrap();

        let handle: Handle<String> = loader.load(
            path.clone(),
            |data| Ok(String::from_utf8_lossy(data).to_string()),
        );

        // Initially loading
        assert!(matches!(loader.state::<String>(&handle), LoadState::Loading));

        let _ = fs::remove_dir_all(&dir);
        loader.shutdown();
    }

    #[test]
    fn test_load_and_process() {
        let mut loader = AssetLoader::new();
        let dir = std::env::temp_dir().join("voltex_loader_test_2");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("data.txt");
        fs::write(&path, "content123").unwrap();

        let handle: Handle<String> = loader.load(
            path.clone(),
            |data| Ok(String::from_utf8_lossy(data).to_string()),
        );

        // Wait for worker
        std::thread::sleep(Duration::from_millis(200));

        let mut assets = Assets::new();
        loader.process_loaded(&mut assets);

        assert!(matches!(loader.state::<String>(&handle), LoadState::Ready));
        let val = assets.get(handle).unwrap();
        assert_eq!(val, "content123");

        let _ = fs::remove_dir_all(&dir);
        loader.shutdown();
    }

    #[test]
    fn test_load_nonexistent_fails() {
        let mut loader = AssetLoader::new();
        let handle: Handle<String> = loader.load(
            PathBuf::from("/nonexistent/file.txt"),
            |data| Ok(String::from_utf8_lossy(data).to_string()),
        );

        std::thread::sleep(Duration::from_millis(200));

        let mut assets = Assets::new();
        loader.process_loaded(&mut assets);

        assert!(matches!(loader.state::<String>(&handle), LoadState::Failed(_)));
        loader.shutdown();
    }
}
```

- [ ] **Step 2: Implement AssetLoader**

Key design:
- Worker thread reads files from disk via channel
- `LoadRequest` contains path + parse function (boxed)
- `LoadResult` contains handle id + parsed asset (boxed Any) or error
- `process_loaded` drains results channel and inserts into Assets
- Handle is pre-allocated with a placeholder in a pending map
- `state()` checks pending map for Loading/Failed, or assets for Ready

```rust
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use crate::handle::Handle;
use crate::assets::Assets;

pub enum LoadState {
    Loading,
    Ready,
    Failed(String),
}

struct LoadRequest {
    id: u64,
    path: PathBuf,
    parse: Box<dyn FnOnce(&[u8]) -> Result<Box<dyn Any + Send>, String> + Send>,
}

struct LoadResult {
    id: u64,
    result: Result<Box<dyn Any + Send>, String>,
}

pub struct AssetLoader {
    sender: Sender<LoadRequest>,
    receiver: Receiver<LoadResult>,
    thread: Option<JoinHandle<()>>,
    next_id: u64,
    pending: HashMap<u64, PendingEntry>,
}

struct PendingEntry {
    state: LoadState,
    handle_id: u32,
    handle_gen: u32,
    type_id: std::any::TypeId,
}
```

- [ ] **Step 3: Run tests, commit**

```bash
cargo test --package voltex_asset -- loader::tests -v
git add crates/voltex_asset/src/loader.rs crates/voltex_asset/src/lib.rs
git commit -m "feat(asset): add AssetLoader with background thread loading"
```

---

### Task 3: Storage replace_in_place for Hot Reload

**Files:**
- Modify: `crates/voltex_asset/src/storage.rs`

- [ ] **Step 1: Write test**

```rust
#[test]
fn replace_in_place() {
    let mut storage: AssetStorage<Mesh> = AssetStorage::new();
    let h = storage.insert(Mesh { verts: 3 });
    storage.replace_in_place(h, Mesh { verts: 99 });
    assert_eq!(storage.get(h).unwrap().verts, 99);
    // Same handle still works — generation unchanged
}
```

- [ ] **Step 2: Implement replace_in_place**

```rust
/// Replace the asset data without changing generation or ref_count.
/// Used for hot reload — existing handles remain valid.
pub fn replace_in_place(&mut self, handle: Handle<T>, new_asset: T) -> bool {
    if let Some(Some(entry)) = self.entries.get_mut(handle.id as usize) {
        if entry.generation == handle.generation {
            entry.asset = new_asset;
            return true;
        }
    }
    false
}
```

- [ ] **Step 3: Run tests, commit**

```bash
cargo test --package voltex_asset -- storage::tests -v
git add crates/voltex_asset/src/storage.rs
git commit -m "feat(asset): add replace_in_place for hot reload support"
```

---

### Task 4: Exports and Full Verification

- [ ] **Step 1: Update lib.rs**

```rust
pub mod watcher;
pub mod loader;
pub use watcher::FileWatcher;
pub use loader::{AssetLoader, LoadState};
```

- [ ] **Step 2: Run full tests**

```bash
cargo test --package voltex_asset -v
cargo build --workspace
cargo test --workspace
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(asset): complete async loading and hot reload support"
```
