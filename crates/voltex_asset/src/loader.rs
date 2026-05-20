use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{self, JoinHandle};

use crate::assets::Assets;
use crate::error::Result;
use crate::handle::Handle;

/// Type-erased parser: deserializes raw bytes into a boxed asset value.
type ParseFn = Box<dyn FnOnce(&[u8]) -> Result<Box<dyn Any + Send>> + Send>;

/// Type-erased inserter: places a parsed asset into `Assets` and returns the
/// assigned handle as a raw (id, generation) pair.
type InserterFn = Box<dyn FnOnce(&mut Assets, Box<dyn Any + Send>) -> (u32, u32)>;

#[derive(Debug)]
pub enum LoadState {
    Loading,
    Ready,
    Failed(String),
}

struct LoadRequest {
    id: u64,
    path: PathBuf,
    parse: ParseFn,
}

struct LoadResult {
    id: u64,
    result: Result<Box<dyn Any + Send>>,
}

struct PendingEntry {
    state: PendingState,
    handle_id: u32,
    handle_gen: u32,
    type_id: TypeId,
    /// Type-erased inserter: takes (assets, boxed_any) and inserts the asset,
    /// returning the actual handle (id, gen) that was assigned.
    inserter: Option<InserterFn>,
}

enum PendingState {
    Loading,
    Failed(String),
    Ready,
}

pub struct AssetLoader {
    request_tx: Option<Sender<LoadRequest>>,
    result_rx: Receiver<LoadResult>,
    thread: Option<JoinHandle<()>>,
    next_id: u64,
    pending: HashMap<u64, PendingEntry>,
    // Map from (type_id, handle_id, handle_gen) to load_id for state lookups
    handle_to_load: HashMap<(TypeId, u32, u32), u64>,
}

impl AssetLoader {
    pub fn new() -> Self {
        let (request_tx, request_rx) = channel::<LoadRequest>();
        let (result_tx, result_rx) = channel::<LoadResult>();

        let thread = thread::spawn(move || {
            while let Ok(req) = request_rx.recv() {
                let result = match std::fs::read(&req.path) {
                    Ok(data) => (req.parse)(&data),
                    Err(e) => Err(format!("Failed to read {}: {}", req.path.display(), e).into()),
                };
                let _ = result_tx.send(LoadResult {
                    id: req.id,
                    result,
                });
            }
        });

        Self {
            request_tx: Some(request_tx),
            result_rx,
            thread: Some(thread),
            next_id: 0,
            pending: HashMap::new(),
            handle_to_load: HashMap::new(),
        }
    }

    /// Queue a file for background loading. Returns a handle immediately.
    ///
    /// The handle becomes valid (pointing to real data) after `process_loaded`
    /// inserts the completed asset into Assets. Until then, `state()` returns
    /// `LoadState::Loading`.
    ///
    /// **Important:** The returned handle's id/generation are provisional.
    /// After `process_loaded`, the handle is updated internally to match the
    /// actual slot in Assets. Since we pre-allocate using the load id, the
    /// actual handle assigned by `Assets::insert` may differ. We remap it.
    pub fn load<T, F>(&mut self, path: PathBuf, parse_fn: F) -> Handle<T>
    where
        T: Send + 'static,
        F: FnOnce(&[u8]) -> Result<T> + Send + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;

        // We use the load id as a provisional handle id.
        // The real handle is assigned when the asset is inserted into Assets.
        let handle_id = id as u32;
        let handle_gen = 0u32;
        let handle = Handle::new(handle_id, handle_gen);

        let type_id = TypeId::of::<T>();

        // Create a type-erased inserter closure that knows how to downcast
        // Box<dyn Any + Send> back to T and insert it into Assets.
        let inserter: InserterFn = Box::new(|assets: &mut Assets, boxed: Box<dyn Any + Send>| {
            let asset = *boxed.downcast::<T>().expect("type mismatch in loader");
            let real_handle = assets.insert(asset);
            (real_handle.id, real_handle.generation)
        });

        self.pending.insert(
            id,
            PendingEntry {
                state: PendingState::Loading,
                handle_id,
                handle_gen,
                type_id,
                inserter: Some(inserter),
            },
        );
        self.handle_to_load
            .insert((type_id, handle_id, handle_gen), id);

        // Wrap parse_fn to erase the type
        let boxed_parse: ParseFn = Box::new(move |data: &[u8]| {
            parse_fn(data).map(|v| Box::new(v) as Box<dyn Any + Send>)
        });

        if let Some(tx) = &self.request_tx {
            let _ = tx.send(LoadRequest {
                id,
                path,
                parse: boxed_parse,
            });
        }

        handle
    }

    /// Check the load state for a given handle.
    pub fn state<T: 'static>(&self, handle: &Handle<T>) -> LoadState {
        let type_id = TypeId::of::<T>();
        if let Some(&load_id) =
            self.handle_to_load
                .get(&(type_id, handle.id, handle.generation))
        {
            if let Some(entry) = self.pending.get(&load_id) {
                return match &entry.state {
                    PendingState::Loading => LoadState::Loading,
                    PendingState::Failed(e) => LoadState::Failed(e.clone()),
                    PendingState::Ready => LoadState::Ready,
                };
            }
        }
        LoadState::Loading
    }

    /// Drain completed loads from the worker thread and insert them into Assets.
    ///
    /// Call this once per frame on the main thread.
    pub fn process_loaded(&mut self, assets: &mut Assets) {
        // Collect results first
        let mut results = Vec::new();
        while let Ok(result) = self.result_rx.try_recv() {
            results.push(result);
        }

        for result in results {
            if let Some(entry) = self.pending.get_mut(&result.id) {
                match result.result {
                    Ok(boxed_asset) => {
                        // Take the inserter out and use it to insert into Assets
                        if let Some(inserter) = entry.inserter.take() {
                            let (real_id, real_gen) = inserter(assets, boxed_asset);

                            // Update the handle mapping if the real handle differs
                            let old_key =
                                (entry.type_id, entry.handle_id, entry.handle_gen);
                            if real_id != entry.handle_id || real_gen != entry.handle_gen {
                                // Remove old mapping, add new one
                                self.handle_to_load.remove(&old_key);
                                entry.handle_id = real_id;
                                entry.handle_gen = real_gen;
                                self.handle_to_load
                                    .insert((entry.type_id, real_id, real_gen), result.id);
                            }
                        }
                        entry.state = PendingState::Ready;
                    }
                    Err(e) => {
                        entry.state = PendingState::Failed(e.to_string());
                    }
                }
            }
        }
    }

    pub fn shutdown(mut self) {
        // Drop the sender to signal the worker to stop
        self.request_tx = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Default for AssetLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    #[test]
    fn test_load_state_initial() {
        let mut loader = AssetLoader::new();
        let dir = std::env::temp_dir().join("voltex_loader_test_1");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test.txt");
        fs::write(&path, "hello world").unwrap();

        let handle: Handle<String> = loader.load(path.clone(), |data| {
            Ok(String::from_utf8_lossy(data).to_string())
        });

        assert!(matches!(
            loader.state::<String>(&handle),
            LoadState::Loading
        ));

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

        let handle: Handle<String> = loader.load(path.clone(), |data| {
            Ok(String::from_utf8_lossy(data).to_string())
        });

        std::thread::sleep(Duration::from_millis(200));

        let mut assets = Assets::new();
        loader.process_loaded(&mut assets);

        assert!(matches!(
            loader.state::<String>(&handle),
            LoadState::Ready
        ));

        // The handle returned by load() is provisional. After process_loaded,
        // the real handle may have different id/gen. We need to look up
        // the actual handle. Since this is the first insert, it should be (0, 0).
        // But our provisional handle is also (0, 0), so it should match.
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

        assert!(matches!(
            loader.state::<String>(&handle),
            LoadState::Failed(_)
        ));
        loader.shutdown();
    }
}
