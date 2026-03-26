/// Audio occlusion parameters.
#[derive(Debug, Clone, Copy)]
pub struct OcclusionResult {
    pub volume_multiplier: f32,    // 0.0-1.0, reduced by occlusion
    pub lowpass_cutoff: f32,       // Hz, lower = more muffled
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

/// Simple low-pass filter (one-pole IIR).
pub struct LowPassFilter {
    prev_output: f32,
    alpha: f32,
}

impl LowPassFilter {
    pub fn new(cutoff_hz: f32, sample_rate: u32) -> Self {
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate as f32;
        let alpha = dt / (rc + dt);
        LowPassFilter { prev_output: 0.0, alpha }
    }

    pub fn set_cutoff(&mut self, cutoff_hz: f32, sample_rate: u32) {
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate as f32;
        self.alpha = dt / (rc + dt);
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.prev_output = self.prev_output + self.alpha * (input - self.prev_output);
        self.prev_output
    }

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
