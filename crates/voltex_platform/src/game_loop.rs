//! Fixed-timestep game loop timer.
//!
//! Glenn Fiedler's classic ["Fix your timestep!"][1] pattern: a
//! wallclock `tick` produces a `frame_time`, which is accumulated and
//! drained in fixed-`dt` chunks via [`should_fixed_update`]. The
//! remainder is exposed as [`alpha`] for interpolation.
//!
//! [1]: https://gafferongames.com/post/fix_your_timestep/

use std::time::{Duration, Instant};

/// Fixed-timestep game-loop timer.
pub struct GameTimer {
    last_frame: Instant,
    accumulator: Duration,
    fixed_dt: Duration,
    frame_time: Duration,
}

impl GameTimer {
    /// Construct with the desired fixed update frequency, e.g.
    /// `GameTimer::new(60)` for 60 Hz.
    pub fn new(fixed_hz: u32) -> Self {
        Self {
            last_frame: Instant::now(),
            accumulator: Duration::ZERO,
            fixed_dt: Duration::from_secs_f64(1.0 / fixed_hz as f64),
            frame_time: Duration::ZERO,
        }
    }

    /// Sample wallclock and accumulate elapsed time into the fixed-
    /// timestep bucket. Clamps frame_time to 250 ms to avoid death
    /// spirals after long stalls (alt-tab, breakpoint).
    pub fn tick(&mut self) {
        let now = Instant::now();
        self.frame_time = now - self.last_frame;
        if self.frame_time > Duration::from_millis(250) {
            self.frame_time = Duration::from_millis(250);
        }
        self.accumulator += self.frame_time;
        self.last_frame = now;
    }

    /// Drain one fixed-`dt` step from the accumulator. Returns `true`
    /// each call until the accumulator falls below `fixed_dt`. Typical
    /// usage: call in a `while` loop each frame.
    pub fn should_fixed_update(&mut self) -> bool {
        if self.accumulator >= self.fixed_dt {
            self.accumulator -= self.fixed_dt;
            true
        } else {
            false
        }
    }

    /// The configured fixed-update interval in seconds.
    pub fn fixed_dt(&self) -> f32 {
        self.fixed_dt.as_secs_f32()
    }

    /// The most recent wallclock frame delta in seconds (clamped).
    pub fn frame_dt(&self) -> f32 {
        self.frame_time.as_secs_f32()
    }

    /// Fraction in `0..1` of the leftover accumulator vs `fixed_dt`.
    /// Pass to the renderer to interpolate physics state at sub-frame
    /// granularity.
    pub fn alpha(&self) -> f32 {
        self.accumulator.as_secs_f32() / self.fixed_dt.as_secs_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_fixed_dt() {
        let timer = GameTimer::new(60);
        let expected = 1.0 / 60.0;
        assert!((timer.fixed_dt() - expected).abs() < 1e-6);
    }

    #[test]
    fn test_should_fixed_update_accumulates() {
        let mut timer = GameTimer::new(60);
        thread::sleep(Duration::from_millis(100));
        timer.tick();
        let mut count = 0;
        while timer.should_fixed_update() {
            count += 1;
        }
        assert!((5..=7).contains(&count), "Expected ~6 fixed updates, got {count}");
    }

    #[test]
    fn test_alpha_range() {
        let mut timer = GameTimer::new(60);
        thread::sleep(Duration::from_millis(10));
        timer.tick();
        while timer.should_fixed_update() {}
        let alpha = timer.alpha();
        assert!((0.0..=1.0).contains(&alpha), "Alpha should be 0..1, got {alpha}");
    }
}
