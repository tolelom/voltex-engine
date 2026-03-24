use std::time::{Duration, Instant};

pub struct GameTimer {
    last_frame: Instant,
    accumulator: Duration,
    fixed_dt: Duration,
    frame_time: Duration,
}

impl GameTimer {
    pub fn new(fixed_hz: u32) -> Self {
        Self {
            last_frame: Instant::now(),
            accumulator: Duration::ZERO,
            fixed_dt: Duration::from_secs_f64(1.0 / fixed_hz as f64),
            frame_time: Duration::ZERO,
        }
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        self.frame_time = now - self.last_frame;
        if self.frame_time > Duration::from_millis(250) {
            self.frame_time = Duration::from_millis(250);
        }
        self.accumulator += self.frame_time;
        self.last_frame = now;
    }

    pub fn should_fixed_update(&mut self) -> bool {
        if self.accumulator >= self.fixed_dt {
            self.accumulator -= self.fixed_dt;
            true
        } else {
            false
        }
    }

    pub fn fixed_dt(&self) -> f32 {
        self.fixed_dt.as_secs_f32()
    }

    pub fn frame_dt(&self) -> f32 {
        self.frame_time.as_secs_f32()
    }

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
        assert!(count >= 5 && count <= 7, "Expected ~6 fixed updates, got {count}");
    }

    #[test]
    fn test_alpha_range() {
        let mut timer = GameTimer::new(60);
        thread::sleep(Duration::from_millis(10));
        timer.tick();
        while timer.should_fixed_update() {}
        let alpha = timer.alpha();
        assert!(alpha >= 0.0 && alpha <= 1.0, "Alpha should be 0..1, got {alpha}");
    }
}
