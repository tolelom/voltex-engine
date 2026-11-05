//! Audio occlusion model — derives a volume scaler + low-pass cutoff from an
//! `occlusion_factor` in `[0.0, 1.0]`.

/// Audio occlusion parameters.
#[derive(Debug, Clone, Copy)]
pub struct OcclusionResult {
    /// Volume scaler in `[0.3, 1.0]` (capped at 70% reduction).
    pub volume_multiplier: f32,
    /// Low-pass cutoff frequency in Hz — lower = more muffled.
    pub lowpass_cutoff: f32,
}

/// Simple ray-based occlusion check.
/// `occlusion_factor`: 0.0 = no occlusion (direct line of sight), 1.0 = fully occluded.
pub fn calculate_occlusion(occlusion_factor: f32) -> OcclusionResult {
    let factor = occlusion_factor.clamp(0.0, 1.0);
    OcclusionResult {
        volume_multiplier: 1.0 - factor * 0.7,           // Max 70% volume reduction
        lowpass_cutoff: 20000.0 * (1.0 - factor * 0.8),  // 20kHz → 4kHz when fully occluded
    }
}

/// Smoothing factor of a one-pole RC low-pass at `cutoff_hz`, sampled at
/// `sample_rate`: `alpha = dt / (RC + dt)` with `RC = 1 / (2π·cutoff)`.
fn one_pole_alpha(cutoff_hz: f32, sample_rate: u32) -> f32 {
    let rc_time_constant = 1.0 / (2.0 * std::f32::consts::PI * cutoff_hz);
    let dt = 1.0 / sample_rate as f32;
    dt / (rc_time_constant + dt)
}

/// Simple low-pass filter (one-pole IIR).
pub struct LowPassFilter {
    prev_output: f32,
    alpha: f32,
}

impl LowPassFilter {
    /// Creates a one-pole low-pass filter with the given cutoff frequency.
    pub fn new(cutoff_hz: f32, sample_rate: u32) -> Self {
        LowPassFilter { prev_output: 0.0, alpha: one_pole_alpha(cutoff_hz, sample_rate) }
    }

    /// Recomputes the filter coefficient for a new cutoff frequency.
    pub fn set_cutoff(&mut self, cutoff_hz: f32, sample_rate: u32) {
        self.alpha = one_pole_alpha(cutoff_hz, sample_rate);
    }

    /// Pushes one sample through the filter; returns the smoothed output.
    pub fn process(&mut self, input: f32) -> f32 {
        self.prev_output = self.prev_output + self.alpha * (input - self.prev_output);
        self.prev_output
    }

    /// Clears the filter's internal state (returns it to silence).
    pub fn reset(&mut self) {
        self.prev_output = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_occlusion() {
        let r = calculate_occlusion(0.0);
        assert!((r.volume_multiplier - 1.0).abs() < 1e-6);
        assert!((r.lowpass_cutoff - 20000.0).abs() < 1.0);
    }

    #[test]
    fn test_full_occlusion() {
        let r = calculate_occlusion(1.0);
        assert!((r.volume_multiplier - 0.3).abs() < 1e-6);
        assert!((r.lowpass_cutoff - 4000.0).abs() < 1.0);
    }

    #[test]
    fn test_partial_occlusion() {
        let r = calculate_occlusion(0.5);
        assert!(r.volume_multiplier > 0.3 && r.volume_multiplier < 1.0);
        assert!(r.lowpass_cutoff > 4000.0 && r.lowpass_cutoff < 20000.0);
    }

    #[test]
    fn test_lowpass_attenuates_high_freq() {
        let mut lpf = LowPassFilter::new(100.0, 44100); // very low cutoff
        // High frequency signal (alternating +1/-1) should be attenuated
        let mut max_output = 0.0_f32;
        for i in 0..100 {
            let input = if i % 2 == 0 { 1.0 } else { -1.0 };
            let output = lpf.process(input);
            max_output = max_output.max(output.abs());
        }
        assert!(max_output < 0.5, "high freq should be attenuated, got {}", max_output);
    }

    #[test]
    fn test_lowpass_passes_dc() {
        let mut lpf = LowPassFilter::new(5000.0, 44100);
        // DC signal (constant 1.0) should pass through
        for _ in 0..1000 {
            lpf.process(1.0);
        }
        let output = lpf.process(1.0);
        assert!((output - 1.0).abs() < 0.01, "DC should pass, got {}", output);
    }
}
