//! `AudioSource` — per-entity playback descriptor.

/// Typed identifier for an [`AudioClip`](crate::AudioClip) in the caller's clip
/// table. A newtype (rather than a bare `u32`) so a clip id cannot be silently
/// confused with any other index. Construct from a `u32` via `ClipId(n)` or
/// `n.into()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClipId(pub u32);

impl From<u32> for ClipId {
    fn from(v: u32) -> Self {
        ClipId(v)
    }
}

/// Per-entity playback descriptor. `voltex_audio` has no ECS dependency, so
/// this is a plain struct; the integration layer (e.g. game code or
/// `voltex_runtime`) registers it as an ECS component and an `audio_sync_system`
/// drives the mixer from it.
#[derive(Debug, Clone)]
pub struct AudioSource {
    /// Clip identifier (key into the caller's clip table holding the
    /// [`AudioClip`](crate::AudioClip)).
    pub clip_id: ClipId,
    /// Linear volume multiplier (0.0 = silent, 1.0 = unattenuated).
    pub volume: f32,
    /// If `true`, the mixer applies 3D spatialisation using the entity transform.
    pub spatial: bool,
    /// If `true`, the clip restarts from the beginning when it ends.
    pub looping: bool,
    /// Desired playback flag — set to `true` to start, `false` to stop.
    pub playing: bool,
    /// Internal flag: set by the mixer once the clip has finished once
    /// (relevant for non-looping one-shot detection).
    pub played_once: bool,
    /// Edge-tracking flag owned by the `audio_sync_system`: `true` once a Play
    /// command has been issued for the current `playing == true` state. The
    /// sync system issues Play on the `playing` false→true edge and Stop on the
    /// true→false edge by comparing `playing` against this. Not set by users.
    pub active: bool,
}

impl AudioSource {
    /// Creates a stopped, 100%-volume, non-spatial, non-looping source.
    pub fn new(clip_id: impl Into<ClipId>) -> Self {
        AudioSource {
            clip_id: clip_id.into(),
            volume: 1.0,
            spatial: false,
            looping: false,
            playing: false,
            played_once: false,
            active: false,
        }
    }

    /// Builder helper: marks this source as spatialised.
    pub fn spatial(mut self) -> Self { self.spatial = true; self }
    /// Builder helper: marks this source as looping.
    pub fn looping(mut self) -> Self { self.looping = true; self }

    /// Starts playback (resets the `played_once` latch).
    pub fn play(&mut self) {
        self.playing = true;
        self.played_once = false;
    }

    /// Stops playback (does not reset `played_once`).
    pub fn stop(&mut self) {
        self.playing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let src = AudioSource::new(42);
        assert_eq!(src.clip_id, ClipId(42));
        assert!((src.volume - 1.0).abs() < 1e-6);
        assert!(!src.spatial);
        assert!(!src.looping);
        assert!(!src.playing);
        assert!(!src.played_once);
        assert!(!src.active);
    }

    #[test]
    fn test_builder_pattern() {
        let src = AudioSource::new(1).spatial().looping();
        assert!(src.spatial);
        assert!(src.looping);
    }

    #[test]
    fn test_play_stop() {
        let mut src = AudioSource::new(1);
        src.play();
        assert!(src.playing);
        assert!(!src.played_once);
        src.played_once = true;
        src.play(); // re-play should reset played_once
        assert!(!src.played_once);
        src.stop();
        assert!(!src.playing);
    }

    #[test]
    fn test_volume() {
        let mut src = AudioSource::new(1);
        src.volume = 0.5;
        assert!((src.volume - 0.5).abs() < 1e-6);
    }
}
