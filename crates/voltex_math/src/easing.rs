//! Easing curves + linear interpolation — the building blocks for tweens, UI
//! motion, camera moves, and "juice". Pair with
//! [`Timer::fraction`](../../voltex_runtime/time/struct.Timer.html) to animate a
//! value over time:
//!
//! ```ignore
//! let t = timer.fraction();             // 0..1 progress
//! let y = lerp(start, end, ease_out_cubic(t));
//! ```
//!
//! Every easing function takes a progress `t` (clamped to `[0, 1]`) and returns
//! the eased progress; `f(0) == 0` and `f(1) == 1` for all of them (the `back`
//! variant overshoots *between* the endpoints, not at them).

/// Linear blend from `a` to `b` by `t` (t is **not** clamped — pass eased
/// progress in `[0, 1]`, or extrapolate deliberately).
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Identity easing (constant velocity).
pub fn linear(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

/// Accelerating from zero (t²).
pub fn ease_in_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

/// Decelerating to zero.
pub fn ease_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Accelerate then decelerate (quadratic).
pub fn ease_in_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

/// Accelerating from zero (t³).
pub fn ease_in_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

/// Decelerating to zero (cubic).
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Accelerate then decelerate (cubic).
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Hermite smoothstep `3t² - 2t³` — smooth (C¹) ease in/out, the classic
/// fade/blend curve.
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Decelerate with a slight overshoot past the target before settling (popular
/// for pop-in UI / juice). Ends exactly at 1.
pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    let p = t - 1.0;
    1.0 + C3 * p * p * p + C1 * p * p
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every easing curve must pin its endpoints.
    fn assert_endpoints(f: fn(f32) -> f32, name: &str) {
        assert!(f(0.0).abs() < 1e-5, "{name}(0) != 0: {}", f(0.0));
        assert!((f(1.0) - 1.0).abs() < 1e-5, "{name}(1) != 1: {}", f(1.0));
    }

    #[test]
    fn all_curves_pin_their_endpoints() {
        assert_endpoints(linear, "linear");
        assert_endpoints(ease_in_quad, "ease_in_quad");
        assert_endpoints(ease_out_quad, "ease_out_quad");
        assert_endpoints(ease_in_out_quad, "ease_in_out_quad");
        assert_endpoints(ease_in_cubic, "ease_in_cubic");
        assert_endpoints(ease_out_cubic, "ease_out_cubic");
        assert_endpoints(ease_in_out_cubic, "ease_in_out_cubic");
        assert_endpoints(smoothstep, "smoothstep");
        assert_endpoints(ease_out_back, "ease_out_back");
    }

    #[test]
    fn curves_clamp_out_of_range_input() {
        assert_eq!(ease_in_quad(-1.0), 0.0);
        assert_eq!(ease_out_cubic(2.0), 1.0);
        assert_eq!(smoothstep(5.0), 1.0);
    }

    #[test]
    fn ease_in_is_below_linear_and_ease_out_is_above() {
        // At the midpoint, ease-in lags (slow start) and ease-out leads.
        assert!(ease_in_quad(0.5) < 0.5, "ease_in should lag: {}", ease_in_quad(0.5));
        assert!(ease_out_quad(0.5) > 0.5, "ease_out should lead: {}", ease_out_quad(0.5));
    }

    #[test]
    fn smoothstep_is_symmetric_about_the_midpoint() {
        assert!((smoothstep(0.5) - 0.5).abs() < 1e-5);
        // f(t) + f(1-t) == 1 for a symmetric ease-in-out.
        for &t in &[0.1f32, 0.25, 0.4] {
            assert!((smoothstep(t) + smoothstep(1.0 - t) - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn ease_out_back_overshoots_before_settling() {
        // Somewhere in the back half it exceeds 1.0, then returns to exactly 1.
        let peak = (0..=100)
            .map(|i| ease_out_back(i as f32 / 100.0))
            .fold(f32::MIN, f32::max);
        assert!(peak > 1.0, "ease_out_back should overshoot, peak={peak}");
    }

    #[test]
    fn lerp_interpolates_and_extrapolates() {
        assert_eq!(lerp(2.0, 10.0, 0.0), 2.0);
        assert_eq!(lerp(2.0, 10.0, 1.0), 10.0);
        assert_eq!(lerp(2.0, 10.0, 0.5), 6.0);
    }

    #[test]
    fn timer_fraction_through_easing_is_a_smooth_value_ramp() {
        // The intended usage: ease a progress fraction, lerp a value.
        let v = lerp(0.0, 100.0, ease_out_cubic(0.0));
        assert_eq!(v, 0.0);
        let v_end = lerp(0.0, 100.0, ease_out_cubic(1.0));
        assert_eq!(v_end, 100.0);
        // Ease-out is past the linear midpoint at t=0.5.
        assert!(lerp(0.0, 100.0, ease_out_cubic(0.5)) > 50.0);
    }
}
