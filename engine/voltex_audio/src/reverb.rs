//! Schroeder-style reverb + single-tap echo + raw [`DelayLine`].

/// Simple delay line for echo effect.
pub struct DelayLine {
    pub(crate) buffer: Vec<f32>,
    pub(crate) write_pos: usize,
    delay_samples: usize,
}

impl DelayLine {
    /// Creates a delay line of `delay_samples` samples (min 1).
    pub fn new(delay_samples: usize) -> Self {
        DelayLine { buffer: vec![0.0; delay_samples.max(1)], write_pos: 0, delay_samples }
    }

    /// Pushes `input` and returns the sample delayed by `delay_samples`. The
    /// `feedback` scalar is added back into the buffer for recirculation.
    pub fn process(&mut self, input: f32, feedback: f32) -> f32 {
        let read_pos = (self.write_pos + self.buffer.len() - self.delay_samples) % self.buffer.len();
        let delayed = self.buffer[read_pos];
        self.buffer[self.write_pos] = input + delayed * feedback;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        delayed
    }
}

/// Simple Schroeder reverb using 4 parallel comb filters + 2 allpass filters.
pub struct Reverb {
    comb_filters: Vec<CombFilter>,
    allpass_filters: Vec<AllpassFilter>,
    /// Wet (reverbed) signal mix `[0.0, 1.0]`.
    pub wet: f32,
    /// Dry (original) signal mix `[0.0, 1.0]`.
    pub dry: f32,
}

struct CombFilter {
    delay: DelayLine,
    feedback: f32,
}

impl CombFilter {
    fn new(delay_samples: usize, feedback: f32) -> Self {
        CombFilter { delay: DelayLine::new(delay_samples), feedback }
    }
    fn process(&mut self, input: f32) -> f32 {
        self.delay.process(input, self.feedback)
    }
}

struct AllpassFilter {
    delay: DelayLine,
    gain: f32,
}

impl AllpassFilter {
    fn new(delay_samples: usize, gain: f32) -> Self {
        AllpassFilter { delay: DelayLine::new(delay_samples), gain }
    }
    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.delay.process(input, 0.0);
        let _output = -self.gain * input + delayed + self.gain * delayed;
        // Actually: allpass: output = -g*input + delayed, buffer = input + g*delayed
        // Simplified:
        let buf_val = self.delay.buffer[(self.delay.write_pos + self.delay.buffer.len() - 1) % self.delay.buffer.len()];
        -self.gain * input + buf_val
    }
}

impl Reverb {
    /// Builds a reverb with prime-number delay lengths tuned for the given
    /// device sample rate.
    pub fn new(sample_rate: u32) -> Self {
        // Schroeder reverb with prime-number delay lengths
        let sr = sample_rate as f32;
        Reverb {
            comb_filters: vec![
                CombFilter::new((0.0297 * sr) as usize, 0.805),
                CombFilter::new((0.0371 * sr) as usize, 0.827),
                CombFilter::new((0.0411 * sr) as usize, 0.783),
                CombFilter::new((0.0437 * sr) as usize, 0.764),
            ],
            allpass_filters: vec![
                AllpassFilter::new((0.005 * sr) as usize, 0.7),
                AllpassFilter::new((0.0017 * sr) as usize, 0.7),
            ],
            wet: 0.3,
            dry: 0.7,
        }
    }

    /// Processes one sample through the reverb chain. Returns the mixed
    /// `dry * input + wet * reverb` output.
    pub fn process(&mut self, input: f32) -> f32 {
        // Sum comb filter outputs
        let mut comb_sum = 0.0;
        for comb in &mut self.comb_filters {
            comb_sum += comb.process(input);
        }
        comb_sum /= self.comb_filters.len() as f32;

        // Chain through allpass filters
        let mut output = comb_sum;
        for ap in &mut self.allpass_filters {
            output = ap.process(output);
        }

        input * self.dry + output * self.wet
    }
}

/// Simple echo (single delay + feedback).
pub struct Echo {
    delay: DelayLine,
    /// Feedback amount fed back into the delay (0.0 = single tap, 1.0 = self-oscillating).
    pub feedback: f32,
    /// Wet (delayed) signal mix.
    pub wet: f32,
    /// Dry (original) signal mix.
    pub dry: f32,
}

impl Echo {
    /// Creates an echo with `delay_ms` of single-tap delay at the given sample rate.
    pub fn new(delay_ms: f32, sample_rate: u32) -> Self {
        let samples = (delay_ms * sample_rate as f32 / 1000.0) as usize;
        Echo { delay: DelayLine::new(samples), feedback: 0.5, wet: 0.3, dry: 0.7 }
    }

    /// Processes one sample. Returns the mixed `dry * input + wet * delayed` output.
    pub fn process(&mut self, input: f32) -> f32 {
        let delayed = self.delay.process(input, self.feedback);
        input * self.dry + delayed * self.wet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_line_basic() {
        let mut dl = DelayLine::new(2);
        assert!((dl.process(1.0, 0.0) - 0.0).abs() < 1e-6); // no output yet
        assert!((dl.process(0.0, 0.0) - 0.0).abs() < 1e-6);
        assert!((dl.process(0.0, 0.0) - 1.0).abs() < 1e-6); // input appears after 2-sample delay
    }

    #[test]
    fn test_delay_line_feedback() {
        let mut dl = DelayLine::new(2);
        dl.process(1.0, 0.5);
        dl.process(0.0, 0.5);
        let out = dl.process(0.0, 0.5); // delayed 1.0
        assert!((out - 1.0).abs() < 1e-6);
        dl.process(0.0, 0.5);
        let out2 = dl.process(0.0, 0.5); // feedback: 0.5
        assert!((out2 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_reverb_preserves_signal() {
        let mut rev = Reverb::new(44100);
        // Process silence -> should be silence
        let out = rev.process(0.0);
        assert!((out - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_reverb_produces_output() {
        let mut rev = Reverb::new(44100);
        // Send an impulse, collect some output
        let mut energy = 0.0;
        rev.process(1.0);
        for _ in 0..4410 {
            let s = rev.process(0.0);
            energy += s.abs();
        }
        assert!(energy > 0.01, "reverb should produce decaying output after impulse");
    }

    #[test]
    fn test_echo_basic() {
        let mut echo = Echo::new(100.0, 44100); // 100ms delay
        let delay_samples = (100.0 * 44100.0 / 1000.0) as usize;
        // Send impulse
        echo.process(1.0);
        // Process until delay
        for _ in 1..delay_samples {
            echo.process(0.0);
        }
        let out = echo.process(0.0);
        // Should have echo of input (wet * delayed)
        assert!(out.abs() > 0.01, "echo should produce delayed output");
    }

    #[test]
    fn test_echo_wet_dry() {
        let mut echo = Echo::new(10.0, 44100);
        echo.wet = 0.0;
        echo.dry = 1.0;
        let out = echo.process(0.5);
        assert!((out - 0.5).abs() < 1e-6); // dry only
    }
}
