//! Audio clip hot reload: re-decode watched files that changed on disk and
//! swap them into the mixer in place.

use voltex_audio::AudioSystem;

/// Poll `watcher` once and hot-reload every changed file that `watched` maps
/// to a clip index: re-decode via [`voltex_audio::load_clip_file`] and swap
/// the clip in place with [`AudioSystem::replace_clip`] (same index — looping
/// BGM picks the new data up on the next mix block; one-shots on next play).
///
/// Returns one `(path, outcome)` per *watched* changed file so the caller can
/// log / surface results. Changes to unwatched paths are ignored. A decode
/// failure leaves the old clip playing (the swap is skipped).
pub fn poll_audio_hot_reload(
    watcher: &mut voltex_asset::FileWatcher,
    watched: &std::collections::HashMap<std::path::PathBuf, usize>,
    audio: &AudioSystem,
) -> Vec<(std::path::PathBuf, Result<(), String>)> {
    let mut outcomes = Vec::new();
    for changed in watcher.poll_changes() {
        let Some(&clip_index) = watched.get(&changed) else {
            continue;
        };
        let outcome = match voltex_audio::load_clip_file(&changed) {
            Ok(clip) => {
                audio.replace_clip(clip_index, clip);
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        };
        outcomes.push((changed, outcome));
    }
    outcomes
}
