//! Generic audio bus — sums several gain-weighted inputs into a single output.

/// Audio bus: mixes multiple input signals.
pub struct AudioBus {
    /// Input list (id + per-input gain).
    pub inputs: Vec<BusInput>,
    /// Final gain applied to the summed inputs.
    pub output_gain: f32,
}

/// Per-input entry on an [`AudioBus`].
#[derive(Debug, Clone)]
pub struct BusInput {
    /// Identifier of the source signal (matched against the sample list).
    pub source_id: u32,
    /// Linear gain applied to this input before summing.
    pub gain: f32,
}

impl Default for AudioBus {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBus {
    /// Empty bus with unity output gain.
    pub fn new() -> Self {
        AudioBus { inputs: Vec::new(), output_gain: 1.0 }
    }

    /// Adds a new input with the given gain.
    pub fn add_input(&mut self, source_id: u32, gain: f32) {
        self.inputs.push(BusInput { source_id, gain });
    }

    /// Removes all inputs matching `source_id`.
    pub fn remove_input(&mut self, source_id: u32) {
        self.inputs.retain(|i| i.source_id != source_id);
    }

    /// Mix samples from all inputs. Each input provides a sample value.
    pub fn mix(&self, samples: &[(u32, f32)]) -> f32 {
        let mut sum = 0.0;
        for input in &self.inputs {
            if let Some((_, sample)) = samples.iter().find(|(id, _)| *id == input.source_id) {
                sum += sample * input.gain;
            }
        }
        sum * self.output_gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_bus() {
        let bus = AudioBus::new();
        assert!((bus.mix(&[]) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_single_input() {
        let mut bus = AudioBus::new();
        bus.add_input(1, 0.5);
        let out = bus.mix(&[(1, 1.0)]);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_multiple_inputs() {
        let mut bus = AudioBus::new();
        bus.add_input(1, 1.0);
        bus.add_input(2, 1.0);
        let out = bus.mix(&[(1, 0.3), (2, 0.5)]);
        assert!((out - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_remove_input() {
        let mut bus = AudioBus::new();
        bus.add_input(1, 1.0);
        bus.add_input(2, 1.0);
        bus.remove_input(1);
        assert_eq!(bus.inputs.len(), 1);
    }

    #[test]
    fn test_output_gain() {
        let mut bus = AudioBus::new();
        bus.output_gain = 0.5;
        bus.add_input(1, 1.0);
        let out = bus.mix(&[(1, 1.0)]);
        assert!((out - 0.5).abs() < 1e-6);
    }
}
