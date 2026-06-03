//! A tiny deterministic pseudo-random generator — no external crates (the
//! engine keeps its dependency surface minimal). Based on **SplitMix64**, which
//! is fast, has a long period, and passes the usual statistical smoke tests for
//! game use (spawn jitter, loot rolls, procedural placement).
//!
//! Determinism is the point: the same seed always yields the same sequence, so
//! gameplay can be replayed / networked / unit-tested. Store one in a
//! [`World`](../../voltex_ecs/world/struct.World.html) resource for a
//! per-run generator every system can reach.
//!
//! ```ignore
//! let mut rng = Rng::seeded(1234);
//! let angle = rng.range_f32(0.0, std::f32::consts::TAU);
//! let dir = rng.unit_vec3();
//! if rng.chance(0.1) { /* 10% drop */ }
//! ```

use crate::{Vec2, Vec3};

/// A seedable SplitMix64 generator.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Create a generator from `seed`. Any seed (including 0) is valid.
    pub fn seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next raw 64-bit value (SplitMix64 step).
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The next 32-bit value (high bits of [`next_u64`](Self::next_u64), which
    /// are the best-mixed).
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// A float uniformly in `[0.0, 1.0)` (24-bit mantissa precision).
    pub fn next_f32(&mut self) -> f32 {
        // Top 24 bits → [0, 2^24) → divide by 2^24.
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// A float uniformly in `[min, max)`. If `max <= min`, returns `min`.
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        if max <= min {
            return min;
        }
        min + self.next_f32() * (max - min)
    }

    /// An integer uniformly in `[min, max)`. If `max <= min`, returns `min`.
    pub fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        let span = (max - min) as u32;
        min + (self.next_u32() % span) as i32
    }

    /// True with probability `p` (clamped to `[0, 1]`). `p <= 0` is always
    /// false; `p >= 1` is always true.
    pub fn chance(&mut self, p: f32) -> bool {
        if p <= 0.0 {
            return false;
        }
        if p >= 1.0 {
            return true;
        }
        self.next_f32() < p
    }

    /// A uniformly random unit vector in the XY plane.
    pub fn unit_vec2(&mut self) -> Vec2 {
        let a = self.range_f32(0.0, std::f32::consts::TAU);
        Vec2::new(a.cos(), a.sin())
    }

    /// A uniformly random unit vector on the sphere (direction). Uses the
    /// standard z + azimuth method so the distribution is area-uniform.
    pub fn unit_vec3(&mut self) -> Vec3 {
        let z = self.range_f32(-1.0, 1.0);
        let a = self.range_f32(0.0, std::f32::consts::TAU);
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(r * a.cos(), r * a.sin(), z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_sequence() {
        let mut a = Rng::seeded(42);
        let mut b = Rng::seeded(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::seeded(1);
        let mut b = Rng::seeded(2);
        // Extremely unlikely to match on the first draw for a good mixer.
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn next_f32_is_in_unit_interval() {
        let mut rng = Rng::seeded(7);
        for _ in 0..10_000 {
            let f = rng.next_f32();
            assert!((0.0..1.0).contains(&f), "f32 out of [0,1): {f}");
        }
    }

    #[test]
    fn range_f32_stays_within_bounds() {
        let mut rng = Rng::seeded(99);
        for _ in 0..10_000 {
            let v = rng.range_f32(-3.0, 5.0);
            assert!((-3.0..5.0).contains(&v), "out of range: {v}");
        }
        // Degenerate range returns min.
        assert_eq!(rng.range_f32(2.0, 2.0), 2.0);
        assert_eq!(rng.range_f32(5.0, 1.0), 5.0);
    }

    #[test]
    fn range_i32_stays_within_bounds_and_covers() {
        let mut rng = Rng::seeded(123);
        let mut seen_min = false;
        let mut seen_max_minus_1 = false;
        for _ in 0..10_000 {
            let v = rng.range_i32(0, 6); // a d6 in [0,6)
            assert!((0..6).contains(&v), "out of range: {v}");
            if v == 0 {
                seen_min = true;
            }
            if v == 5 {
                seen_max_minus_1 = true;
            }
        }
        assert!(seen_min && seen_max_minus_1, "range should cover both ends");
        assert_eq!(rng.range_i32(4, 4), 4, "degenerate range returns min");
    }

    #[test]
    fn chance_extremes_are_deterministic() {
        let mut rng = Rng::seeded(5);
        for _ in 0..1000 {
            assert!(!rng.chance(0.0));
            assert!(rng.chance(1.0));
        }
    }

    #[test]
    fn chance_is_roughly_calibrated() {
        let mut rng = Rng::seeded(2024);
        let n = 100_000;
        let hits = (0..n).filter(|_| rng.chance(0.25)).count();
        let ratio = hits as f32 / n as f32;
        assert!((ratio - 0.25).abs() < 0.02, "p=0.25 gave {ratio}");
    }

    #[test]
    fn unit_vectors_are_normalized() {
        let mut rng = Rng::seeded(88);
        for _ in 0..1000 {
            assert!((rng.unit_vec2().length() - 1.0).abs() < 1e-4);
            assert!((rng.unit_vec3().length() - 1.0).abs() < 1e-4);
        }
    }
}
