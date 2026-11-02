//! Minimal HUD primitive — rolling FPS counter + user-controlled title segment,
//! both surfaced through the OS window title.
//!
//! Why title-bar and not in-window: shipping a full text-overlay pipeline
//! (font atlas, screen-space quads, alpha pass) inside `voltex_runtime` would
//! require either a new dep on `voltex_editor` (which pulls IMGUI, dock tree,
//! TTF — disproportionate for a HUD) or a duplicated bitmap-font renderer.
//! The title bar gives every game an out-of-box FPS readout and a one-line
//! "scoreboard" channel with zero rendering work. v1.1+ will add in-window
//! overlay rendering once a shared text primitive exists.
//!
//! Usage:
//!
//! ```ignore
//! // Each frame, in a user system:
//! app.hud.set_user_segment(Some(format!("Score: {}", score)));
//! ```
//!
//! The runtime calls [`Hud::tick`] once per frame; when the update interval
//! elapses it returns the new title string, which `RuntimeApp` writes back to
//! the window.

use std::collections::VecDeque;

/// Rolling FPS averaging + window-title formatting.
pub struct Hud {
    /// Window title supplied by [`RuntimeConfig::with_window_title`](crate::RuntimeConfig::with_window_title).
    /// Used as the base when [`fps_in_title`](Self::fps_in_title) reformats
    /// the title each interval.
    base_title: String,
    /// Optional user-controlled segment placed between the base title and the
    /// FPS suffix. Updated by game code via [`set_user_segment`](Self::set_user_segment).
    user_segment: Option<String>,
    /// Rolling window of recent frame deltas in seconds.
    samples: VecDeque<f32>,
    /// Maximum samples retained for FPS averaging (default 60 ≈ 1s @ 60Hz).
    sample_window: usize,
    /// How often the window title is rebuilt, in seconds (default 0.25).
    update_interval_s: f32,
    /// Accumulator since the last title update.
    since_last_update_s: f32,
    /// When `true` (default), [`tick`](Self::tick) periodically returns a new
    /// formatted title string. Set to `false` to leave the title alone and
    /// access the FPS via [`average_fps`](Self::average_fps) directly.
    pub fps_in_title: bool,
}

impl Hud {
    /// New HUD with the given base window title; FPS-in-title enabled by default.
    pub fn new(base_title: impl Into<String>) -> Self {
        let sample_window = 60;
        Self {
            base_title: base_title.into(),
            user_segment: None,
            samples: VecDeque::with_capacity(sample_window),
            sample_window,
            update_interval_s: 0.25,
            since_last_update_s: 0.0,
            fps_in_title: true,
        }
    }

    /// Replaces the user-controlled segment shown between the base title and
    /// the FPS suffix. Pass `None` to drop it.
    pub fn set_user_segment(&mut self, segment: Option<String>) {
        self.user_segment = segment;
    }

    /// Sets how many recent frames participate in the FPS average. Clamps to
    /// at least 1.
    pub fn set_sample_window(&mut self, window: usize) {
        self.sample_window = window.max(1);
        while self.samples.len() > self.sample_window {
            self.samples.pop_front();
        }
    }

    /// Sets the title update cadence in seconds. Clamps to at least 1 ms so
    /// the title isn't rebuilt every frame.
    pub fn set_update_interval(&mut self, seconds: f32) {
        self.update_interval_s = seconds.max(0.001);
    }

    /// Average FPS over the recent sample window. Returns `0.0` until at
    /// least one frame has been recorded.
    pub fn average_fps(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.samples.iter().copied().sum();
        if sum <= 0.0 {
            return 0.0;
        }
        self.samples.len() as f32 / sum
    }

    /// FPS computed from the most recent frame only — noisier than
    /// [`average_fps`](Self::average_fps) but reacts immediately to spikes.
    pub fn instantaneous_fps(&self) -> f32 {
        match self.samples.back() {
            Some(&dt) if dt > 0.0 => 1.0 / dt,
            _ => 0.0,
        }
    }

    /// Records one frame's `frame_dt` and, if the update interval elapsed and
    /// [`fps_in_title`](Self::fps_in_title) is enabled, returns a freshly
    /// formatted title string for the caller to apply to the window. The
    /// caller (typically `RuntimeApp`) is responsible for the actual
    /// window-title write so this module stays free of `winit` types.
    pub fn tick(&mut self, frame_dt_s: f32) -> Option<String> {
        if frame_dt_s > 0.0 {
            self.samples.push_back(frame_dt_s);
            while self.samples.len() > self.sample_window {
                self.samples.pop_front();
            }
        }

        if !self.fps_in_title {
            return None;
        }
        self.since_last_update_s += frame_dt_s;
        if self.since_last_update_s < self.update_interval_s {
            return None;
        }
        self.since_last_update_s = 0.0;
        Some(self.format_title())
    }

    /// Builds the window title string from the current state, without
    /// affecting timers. Useful for tests and one-shot redraw triggers.
    pub fn format_title(&self) -> String {
        let fps = self.average_fps().round() as u32;
        match (&self.user_segment, self.fps_in_title) {
            (Some(seg), true) => format!("{} | {} | {} fps", self.base_title, seg, fps),
            (Some(seg), false) => format!("{} | {}", self.base_title, seg),
            (None, true) => format!("{} | {} fps", self.base_title, fps),
            (None, false) => self.base_title.clone(),
        }
    }

    /// The original base title — never reformatted.
    pub fn base_title(&self) -> &str {
        &self.base_title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_fps_before_any_sample_is_zero() {
        let hud = Hud::new("test");
        assert_eq!(hud.average_fps(), 0.0);
        assert_eq!(hud.instantaneous_fps(), 0.0);
    }

    #[test]
    fn average_fps_over_a_window() {
        let mut hud = Hud::new("test");
        // 60 frames at 16.67ms each → 60 fps.
        for _ in 0..60 {
            hud.tick(1.0 / 60.0);
        }
        let avg = hud.average_fps();
        assert!((avg - 60.0).abs() < 0.5, "expected ~60 fps, got {avg}");
    }

    #[test]
    fn tick_emits_title_after_interval_elapses() {
        let mut hud = Hud::new("My Game");
        hud.set_update_interval(0.1);

        // Below threshold — no emission.
        assert!(hud.tick(0.05).is_none());
        // Crossing threshold — emit.
        let title = hud.tick(0.05).expect("title emitted after interval");
        assert!(title.starts_with("My Game"), "got {title}");
        assert!(title.contains("fps"));
    }

    #[test]
    fn tick_with_disabled_fps_returns_none() {
        let mut hud = Hud::new("My Game");
        hud.fps_in_title = false;
        for _ in 0..10 {
            assert!(hud.tick(0.1).is_none());
        }
    }

    #[test]
    fn user_segment_appears_in_title() {
        let mut hud = Hud::new("Base");
        hud.set_user_segment(Some("Score: 42".into()));
        let t = hud.format_title();
        assert!(t.contains("Base | Score: 42 | "), "{t}");
        assert!(t.ends_with("fps"));
        hud.set_user_segment(None);
        let t = hud.format_title();
        assert!(!t.contains("Score"));
    }

    #[test]
    fn user_segment_without_fps_omits_fps_suffix() {
        let mut hud = Hud::new("Base");
        hud.fps_in_title = false;
        hud.set_user_segment(Some("Score: 7".into()));
        assert_eq!(hud.format_title(), "Base | Score: 7");
    }

    #[test]
    fn sample_window_truncates_old_samples() {
        let mut hud = Hud::new("test");
        hud.set_sample_window(3);
        // 5 samples but only 3 retained.
        for _ in 0..5 {
            hud.tick(0.5);
        }
        assert_eq!(hud.samples.len(), 3);
    }

    #[test]
    fn set_update_interval_clamps_to_minimum() {
        let mut hud = Hud::new("test");
        hud.set_update_interval(0.0);
        assert!(hud.update_interval_s >= 0.001);
        hud.set_update_interval(-1.0);
        assert!(hud.update_interval_s >= 0.001);
    }

    #[test]
    fn instantaneous_fps_uses_last_sample() {
        let mut hud = Hud::new("test");
        hud.tick(1.0 / 30.0);
        hud.tick(1.0 / 120.0);
        // Last sample = 1/120 → 120 fps.
        assert!((hud.instantaneous_fps() - 120.0).abs() < 0.1);
    }

    #[test]
    fn zero_or_negative_dt_is_ignored() {
        let mut hud = Hud::new("test");
        hud.tick(0.0);
        hud.tick(-0.5);
        assert!(hud.samples.is_empty());
    }
}
