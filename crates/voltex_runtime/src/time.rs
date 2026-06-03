//! Per-frame time, surfaced to gameplay systems.
//!
//! Systems registered with [`RuntimeApp::add_timed_system`](crate::RuntimeApp::add_timed_system)
//! receive a `&Time` so they can read **accumulated** elapsed seconds and the
//! frame counter — not just the per-frame delta. This is what timers, cooldowns,
//! and animation phase need, and previously every game had to accumulate by hand.
//!
//! `elapsed_secs` accumulates the same `dt` the runtime steps systems with, so
//! it is deterministic under a fixed timestep (`RuntimeConfig::with_fixed_dt`).

/// A snapshot of the runtime clock for the current frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Time {
    /// Seconds elapsed since the app started (sum of every frame's `dt`).
    pub elapsed_secs: f32,
    /// The delta time of the current frame, in seconds.
    pub delta_secs: f32,
    /// Number of frames stepped so far (1 on the first frame after `advance`).
    pub frame_count: u64,
}

impl Time {
    /// A zeroed clock (before the first frame).
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance by one frame of `dt` seconds: bump the frame counter, record the
    /// delta, and add it to the running total.
    pub fn advance(&mut self, dt: f32) {
        self.delta_secs = dt;
        self.elapsed_secs += dt;
        self.frame_count += 1;
    }
}

/// Whether a [`Timer`] stops after firing once or restarts each cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    /// Fire once, then stay finished.
    Once,
    /// Fire every `duration` seconds, wrapping the leftover time.
    Repeating,
}

/// A countdown driven by [`tick`](Timer::tick) — the building block for
/// cooldowns, spawn intervals, ability recharge, and timed state. Advance it
/// each frame with the frame `dt` and check [`just_finished`](Timer::just_finished).
///
/// ```ignore
/// let mut cooldown = Timer::once(1.5);
/// // each frame:
/// if cooldown.tick(dt).just_finished() { /* ability ready */ }
/// ```
#[derive(Debug, Clone)]
pub struct Timer {
    duration: f32,
    elapsed: f32,
    mode: TimerMode,
    finished: bool,
    just_finished: bool,
}

impl Timer {
    /// A one-shot timer of `duration` seconds.
    pub fn once(duration: f32) -> Self {
        Self::new(duration, TimerMode::Once)
    }

    /// A repeating timer that fires every `duration` seconds.
    pub fn repeating(duration: f32) -> Self {
        Self::new(duration, TimerMode::Repeating)
    }

    fn new(duration: f32, mode: TimerMode) -> Self {
        Self {
            duration,
            elapsed: 0.0,
            mode,
            finished: false,
            just_finished: false,
        }
    }

    /// Advance by `dt` seconds. Sets [`just_finished`](Self::just_finished) for
    /// this tick if the timer reached its duration. Returns `&mut self` so you
    /// can chain `timer.tick(dt).just_finished()`.
    pub fn tick(&mut self, dt: f32) -> &mut Self {
        self.just_finished = false;

        // Non-positive duration: fire immediately (and every tick if repeating).
        if self.duration <= 0.0 {
            self.just_finished = !(self.mode == TimerMode::Once && self.finished);
            self.finished = true;
            return self;
        }
        // A one-shot that already fired stays latched and idle.
        if self.mode == TimerMode::Once && self.finished {
            return self;
        }

        self.elapsed += dt;
        if self.elapsed >= self.duration {
            self.just_finished = true;
            self.finished = true;
            match self.mode {
                TimerMode::Once => self.elapsed = self.duration,
                // Wrap leftover time (handles dt larger than one period).
                TimerMode::Repeating => self.elapsed %= self.duration,
            }
        }
        self
    }

    /// True only on the tick the timer reached its duration. For `Repeating`,
    /// true once per completed cycle.
    pub fn just_finished(&self) -> bool {
        self.just_finished
    }

    /// For `Once`: latched true once the timer has completed. For `Repeating`:
    /// equal to [`just_finished`](Self::just_finished) (a repeating timer never
    /// finishes permanently).
    pub fn finished(&self) -> bool {
        match self.mode {
            TimerMode::Once => self.finished,
            TimerMode::Repeating => self.just_finished,
        }
    }

    /// Seconds elapsed in the current cycle.
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    /// The timer's configured duration.
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Progress through the current cycle in `[0, 1]` (1 if duration <= 0).
    pub fn fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.elapsed / self.duration).clamp(0.0, 1.0)
    }

    /// Restart: clear elapsed time and the finished flags.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.finished = false;
        self.just_finished = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_zeroed() {
        let t = Time::new();
        assert_eq!(t.elapsed_secs, 0.0);
        assert_eq!(t.delta_secs, 0.0);
        assert_eq!(t.frame_count, 0);
    }

    #[test]
    fn advance_accumulates_elapsed_and_counts_frames() {
        let mut t = Time::new();
        t.advance(0.1);
        t.advance(0.1);
        t.advance(0.1);
        assert!((t.elapsed_secs - 0.3).abs() < 1e-5, "elapsed = {}", t.elapsed_secs);
        assert_eq!(t.delta_secs, 0.1);
        assert_eq!(t.frame_count, 3);
    }

    #[test]
    fn advance_records_latest_delta() {
        let mut t = Time::new();
        t.advance(0.5);
        t.advance(0.25);
        assert_eq!(t.delta_secs, 0.25);
        assert!((t.elapsed_secs - 0.75).abs() < 1e-5);
    }

    // ── Timer ───────────────────────────────────────────────────────────

    #[test]
    fn once_timer_fires_exactly_once_at_its_duration() {
        let mut timer = Timer::once(1.0);
        assert!(!timer.tick(0.6).just_finished(), "not yet");
        assert!(!timer.finished());
        assert!(timer.tick(0.6).just_finished(), "crossed 1.0 -> fires");
        assert!(timer.finished());
        // Further ticks: latched finished, but no longer "just" finished.
        assert!(!timer.tick(0.6).just_finished());
        assert!(timer.finished());
    }

    #[test]
    fn repeating_timer_fires_each_cycle_and_wraps() {
        let mut timer = Timer::repeating(1.0);
        assert!(!timer.tick(0.5).just_finished());
        assert!(timer.tick(0.6).just_finished(), "cycle 1 at 1.1s");
        // 0.1 carried over; another 0.95 -> 1.05, fires again.
        assert!(!timer.tick(0.5).just_finished());
        assert!(timer.tick(0.45).just_finished(), "cycle 2");
    }

    #[test]
    fn repeating_timer_fires_when_dt_exceeds_period() {
        let mut timer = Timer::repeating(0.5);
        // A 2.0s step is several periods — fires (once) and wraps the remainder.
        assert!(timer.tick(2.0).just_finished());
        assert!(timer.elapsed() < 0.5, "leftover wrapped: {}", timer.elapsed());
    }

    #[test]
    fn fraction_tracks_progress() {
        let mut timer = Timer::once(2.0);
        timer.tick(0.5);
        assert!((timer.fraction() - 0.25).abs() < 1e-5);
        timer.tick(10.0);
        assert_eq!(timer.fraction(), 1.0, "clamped at full");
    }

    #[test]
    fn reset_restarts_the_timer() {
        let mut timer = Timer::once(1.0);
        timer.tick(2.0);
        assert!(timer.finished());
        timer.reset();
        assert!(!timer.finished());
        assert_eq!(timer.elapsed(), 0.0);
        assert!(timer.tick(1.5).just_finished(), "fires again after reset");
    }

    #[test]
    fn zero_duration_timer_fires_immediately() {
        let mut once = Timer::once(0.0);
        assert!(once.tick(0.0).just_finished());
        assert!(once.finished());
        assert!(!once.tick(0.0).just_finished(), "once stays latched");
        assert_eq!(once.fraction(), 1.0);

        let mut rep = Timer::repeating(0.0);
        assert!(rep.tick(0.0).just_finished());
        assert!(rep.tick(0.0).just_finished(), "repeating zero fires every tick");
    }
}
