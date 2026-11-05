//! Fire-and-forget background audio file loader. Each `load()` spawns a
//! worker thread that reads the file and flips the entry's [`AudioLoadState`].
//!
//! The read bytes are discarded: this only reports whether the file could be
//! read. It does not decode or register a clip with [`crate::AudioSystem`]
//! (the runtime loads clips synchronously instead).

use std::sync::{Arc, Mutex};
use std::path::PathBuf;

/// State of one async audio load. (Named apart from
/// `voltex_asset::LoadState`, which tracks typed asset loads.)
#[derive(Debug, Clone, PartialEq)]
pub enum AudioLoadState {
    /// Queued but worker thread has not yet started reading.
    Pending,
    /// Worker thread has begun reading bytes from disk.
    Loading,
    /// File was read successfully.
    Loaded,
    /// File read failed; carries the human-readable error message.
    Error(String),
}

/// Old name of [`AudioLoadState`] — clashed with `voltex_asset::LoadState`.
#[deprecated(note = "use `AudioLoadState`")]
pub type LoadState = AudioLoadState;

/// Background audio loader — owns a shared queue of in-flight loads.
pub struct AsyncAudioLoader {
    pending: Arc<Mutex<Vec<(u32, PathBuf, AudioLoadState)>>>,
}

impl Default for AsyncAudioLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncAudioLoader {
    /// Creates an empty loader.
    pub fn new() -> Self {
        AsyncAudioLoader { pending: Arc::new(Mutex::new(Vec::new())) }
    }

    /// Queue a clip for async loading. Returns immediately.
    pub fn load(&self, clip_id: u32, path: PathBuf) {
        let mut pending = self.pending.lock().unwrap();
        pending.push((clip_id, path.clone(), AudioLoadState::Pending));
        let pending_clone = Arc::clone(&self.pending);
        std::thread::spawn(move || {
            // Mark loading
            { let mut p = pending_clone.lock().unwrap();
              if let Some(entry) = p.iter_mut().find(|(id, _, _)| *id == clip_id) {
                  entry.2 = AudioLoadState::Loading;
              }
            }
            // Read only to check the file is loadable — the bytes are dropped.
            match std::fs::read(&path) {
                Ok(_data) => {
                    let mut p = pending_clone.lock().unwrap();
                    if let Some(entry) = p.iter_mut().find(|(id, _, _)| *id == clip_id) {
                        entry.2 = AudioLoadState::Loaded;
                    }
                }
                Err(e) => {
                    let mut p = pending_clone.lock().unwrap();
                    if let Some(entry) = p.iter_mut().find(|(id, _, _)| *id == clip_id) {
                        entry.2 = AudioLoadState::Error(e.to_string());
                    }
                }
            }
        });
    }

    /// Returns the current [`AudioLoadState`] for `clip_id`, or `Error("not found")`
    /// if it was never queued.
    pub fn state(&self, clip_id: u32) -> AudioLoadState {
        let pending = self.pending.lock().unwrap();
        pending.iter().find(|(id, _, _)| *id == clip_id)
            .map(|(_, _, s)| s.clone())
            .unwrap_or(AudioLoadState::Error("not found".to_string()))
    }

    /// Returns the clip ids that have reached [`AudioLoadState::Loaded`].
    pub fn poll_completed(&self) -> Vec<u32> {
        let pending = self.pending.lock().unwrap();
        pending.iter().filter(|(_, _, s)| *s == AudioLoadState::Loaded).map(|(id, _, _)| *id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_new() {
        let loader = AsyncAudioLoader::new();
        assert!(loader.poll_completed().is_empty());
    }
    #[test]
    fn test_load_nonexistent() {
        let loader = AsyncAudioLoader::new();
        loader.load(1, PathBuf::from("/nonexistent/path.wav"));
        std::thread::sleep(std::time::Duration::from_millis(100));
        let state = loader.state(1);
        assert!(matches!(state, AudioLoadState::Error(_)));
    }
    #[test]
    fn test_load_existing() {
        let dir = std::env::temp_dir().join("voltex_async_test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("test.wav"), b"RIFF").unwrap();
        let loader = AsyncAudioLoader::new();
        loader.load(42, dir.join("test.wav"));
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(loader.state(42), AudioLoadState::Loaded);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
