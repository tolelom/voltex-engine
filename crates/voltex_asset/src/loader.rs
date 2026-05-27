//! Background asset loader. The worker thread reads bytes from disk and the
//! caller's parse closure converts them to `T`; the main thread drains results
//! via [`AssetLoader::process_loaded`] and inserts each asset into [`Assets`].

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

/// Current state of an in-flight load.
#[derive(Debug)]
pub enum LoadState {
    /// Worker has not yet finished reading + parsing.
    Loading,
    /// Asset has been inserted into [`Assets`] and is usable.
    Ready,
    /// Read or parse failed; carries the human-readable error message.
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

/// Background worker thread + per-load tracking. Owns a single worker that
/// reads files sequentially; submit loads with [`AssetLoader::load`] and drain
/// completed ones with [`AssetLoader::process_loaded`].
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
    /// Spawns the worker thread and returns a ready-to-use loader.
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

    /// Queue a file for background loading. Returns a *ticket* handle immediately.
    ///
    /// The returned handle is a **ticket**, not a usable `Assets` handle: its
    /// id/generation come from a global load counter, whereas `Assets::insert`
    /// (run later, in `process_loaded`) assigns a per-type slot that generally
    /// differs. Use the ticket to poll [`state`](Self::state), and once it is
    /// [`LoadState::Ready`] call [`resolved_handle`](Self::resolved_handle) to
    /// get the real `Handle<T>` you pass to `Assets::get`.
    ///
    /// (Only when loading into a fresh, single-type `Assets` do the ticket and
    /// real handle coincide — don't rely on that.)
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

    /// The real [`Handle<T>`] the asset was assigned in [`Assets`], once its
    /// load has reached [`LoadState::Ready`].
    ///
    /// The handle returned by [`load`](Self::load) is only a *ticket*: its
    /// id/generation come from a global load counter and generally do **not**
    /// match the per-type slot `Assets::insert` assigns. Pass the ticket here
    /// after the load is `Ready` to get the handle you actually use with
    /// `Assets::get`. Returns `None` if the ticket is unknown or not yet ready.
    pub fn resolved_handle<T: 'static>(&self, ticket: &Handle<T>) -> Option<Handle<T>> {
        let type_id = TypeId::of::<T>();
        let load_id = *self
            .handle_to_load
            .get(&(type_id, ticket.id, ticket.generation))?;
        let entry = self.pending.get(&load_id)?;
        match entry.state {
            PendingState::Ready => Some(Handle::new(entry.handle_id, entry.handle_gen)),
            _ => None,
        }
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
                        // Take the inserter out and use it to insert into Assets.
                        if let Some(inserter) = entry.inserter.take() {
                            let (real_id, real_gen) = inserter(assets, boxed_asset);
                            // Record the real Assets handle so `resolved_handle`
                            // can return it. Crucially we do NOT touch
                            // `handle_to_load`: it stays keyed by the stable
                            // ticket (the id/gen `load` handed the caller), so
                            // `state` and `resolved_handle` keep working even
                            // when the real per-type slot differs from the
                            // global-counter ticket id.
                            entry.handle_id = real_id;
                            entry.handle_gen = real_gen;
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

    /// Signals the worker to stop, joins the thread. Pending requests still in
    /// the queue are dropped.
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
    fn state_and_handle_correct_when_real_differs_from_ticket() {
        // The ticket from load() uses a global load counter, while Assets
        // assigns a per-type slot. When the two differ (here: a pre-existing
        // asset pushes the loaded one to slot 1, but the ticket is slot 0),
        // state() must still report Ready and resolved_handle() must return the
        // real handle that actually addresses the loaded asset.
        let mut loader = AssetLoader::new();
        let dir = std::env::temp_dir().join("voltex_loader_test_remap");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("data.txt");
        fs::write(&path, "loaded-content").unwrap();

        let mut assets = Assets::new();
        let _pre = assets.insert("pre-existing".to_string()); // occupies slot 0

        let ticket: Handle<String> =
            loader.load(path, |data| Ok(String::from_utf8_lossy(data).to_string()));

        std::thread::sleep(Duration::from_millis(200));
        loader.process_loaded(&mut assets);

        // state() keyed by the stable ticket must report Ready.
        assert!(
            matches!(loader.state::<String>(&ticket), LoadState::Ready),
            "state() must report Ready for the ticket even when the real slot differs"
        );

        // resolved_handle() returns the handle that actually addresses the asset.
        let real = loader
            .resolved_handle::<String>(&ticket)
            .expect("resolved_handle should be Some once Ready");
        assert_eq!(assets.get(real).unwrap(), "loaded-content");

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
