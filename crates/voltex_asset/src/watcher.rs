//! mtime-polling file watcher used to trigger hot reload.
//!
//! No OS notifications — we read each watched file's `modified()` time at
//! `poll_interval` and report any whose timestamp moved. First poll establishes
//! baseline (no false-positive change on first call).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Polls a set of file paths for `mtime` changes.
pub struct FileWatcher {
    watched: HashMap<PathBuf, Option<SystemTime>>,
    poll_interval: Duration,
    last_poll: Instant,
}

impl FileWatcher {
    /// Creates a watcher that re-checks at most every `poll_interval`.
    pub fn new(poll_interval: Duration) -> Self {
        Self {
            watched: HashMap::new(),
            poll_interval,
            last_poll: Instant::now() - poll_interval, // allow immediate first poll
        }
    }

    /// Starts watching `path`. First poll after this call will not flag it
    /// as changed — it only records the baseline mtime.
    pub fn watch(&mut self, path: PathBuf) {
        // Store None initially — first poll will record the mtime without reporting change
        self.watched.insert(path, None);
    }

    /// Stops watching `path`. No-op if not watched.
    pub fn unwatch(&mut self, path: &Path) {
        self.watched.remove(path);
    }

    /// Returns the paths whose mtime changed since the previous poll. Returns
    /// an empty vec if the poll interval has not elapsed yet.
    pub fn poll_changes(&mut self) -> Vec<PathBuf> {
        let now = Instant::now();
        if now.duration_since(self.last_poll) < self.poll_interval {
            return Vec::new();
        }
        self.last_poll = now;

        let mut changed = Vec::new();
        for (path, last_mtime) in &mut self.watched {
            let current = std::fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok());
            if let Some(prev) = last_mtime {
                // We have a previous mtime — compare
                if current != Some(*prev) {
                    changed.push(path.clone());
                }
            }
            // else: first poll, just record mtime, don't report
            *last_mtime = current;
        }
        changed
    }

    /// Number of paths currently being watched.
    pub fn watched_count(&self) -> usize {
        self.watched.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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

        // Second poll without modification — still no changes
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
        assert_eq!(watcher.watched_count(), 1);
        watcher.unwatch(&path);
        assert_eq!(watcher.watched_count(), 0);
        assert!(watcher.poll_changes().is_empty());
    }
}
