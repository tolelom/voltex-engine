/// Audio bus: mixes multiple input signals.
pub struct AudioBus {
    pub inputs: Vec<BusInput>,
    pub output_gain: f32,
}

#[derive(Debug, Clone)]
pub struct BusInput {
    pub source_id: u32,
    pub gain: f32,
}

impl AudioBus {
    pub fn new() -> Self {
        AudioBus { inputs: Vec::new(), output_gain: 1.0 }
    }

    pub fn add_input(&mut self, source_id: u32, gain: f32) {
        self.inputs.push(BusInput { source_id, gain });
    }

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
