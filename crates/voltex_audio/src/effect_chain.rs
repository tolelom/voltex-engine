/// Trait for audio effects.
pub trait AudioEffect {
    fn process(&mut self, sample: f32) -> f32;
    fn name(&self) -> &str;
}

/// Chain of audio effects processed in order.
pub struct EffectChain {
    effects: Vec<Box<dyn AudioEffect>>,
    pub bypass: bool,
}

impl Default for EffectChain {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectChain {
    pub fn new() -> Self { EffectChain { effects: Vec::new(), bypass: false } }
    pub fn add(&mut self, effect: Box<dyn AudioEffect>) { self.effects.push(effect); }
    pub fn remove(&mut self, index: usize) { if index < self.effects.len() { self.effects.remove(index); } }
    pub fn len(&self) -> usize { self.effects.len() }
    pub fn is_empty(&self) -> bool { self.effects.is_empty() }

    pub fn process(&mut self, sample: f32) -> f32 {
        if self.bypass { return sample; }
        let mut s = sample;
        for effect in &mut self.effects {
            s = effect.process(s);
        }
        s
    }
}

// Simple gain effect for testing
pub struct GainEffect { pub gain: f32 }
impl AudioEffect for GainEffect {
    fn process(&mut self, sample: f32) -> f32 { sample * self.gain }
    fn name(&self) -> &str { "Gain" }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_empty_chain() { let mut c = EffectChain::new(); assert!((c.process(1.0) - 1.0).abs() < 1e-6); }
    #[test]
    fn test_single_effect() {
        let mut c = EffectChain::new();
        c.add(Box::new(GainEffect { gain: 0.5 }));
        assert!((c.process(1.0) - 0.5).abs() < 1e-6);
    }
    #[test]
    fn test_chain_order() {
        let mut c = EffectChain::new();
        c.add(Box::new(GainEffect { gain: 0.5 }));
        c.add(Box::new(GainEffect { gain: 0.5 }));
        assert!((c.process(1.0) - 0.25).abs() < 1e-6);
    }
    #[test]
    fn test_bypass() {
        let mut c = EffectChain::new();
        c.add(Box::new(GainEffect { gain: 0.0 }));
        c.bypass = true;
        assert!((c.process(1.0) - 1.0).abs() < 1e-6);
    }
}
