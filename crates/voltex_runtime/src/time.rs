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
}
