//! Per-sample audio effect chain. Users implement [`AudioEffect`] and stack
//! instances on an [`EffectChain`] which runs them in order.

/// Trait for audio effects.
pub trait AudioEffect {
    /// Processes one input sample and returns the processed output.
    fn process(&mut self, sample: f32) -> f32;
    /// Human-readable effect name (used for debugging/inspector display).
    fn name(&self) -> &str;
}

/// Chain of audio effects processed in order.
pub struct EffectChain {
    effects: Vec<Box<dyn AudioEffect>>,
    /// When `true`, [`process`](Self::process) returns the input unchanged.
    pub bypass: bool,
}

impl Default for EffectChain {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectChain {
    /// Creates an empty chain (no effects, not bypassed).
    pub fn new() -> Self { EffectChain { effects: Vec::new(), bypass: false } }
    /// Appends an effect to the end of the chain.
    pub fn add(&mut self, effect: Box<dyn AudioEffect>) { self.effects.push(effect); }
    /// Removes the effect at `index`. No-op if out of range.
    pub fn remove(&mut self, index: usize) { if index < self.effects.len() { self.effects.remove(index); } }
    /// Returns the number of effects in the chain.
    pub fn len(&self) -> usize { self.effects.len() }
    /// `true` if the chain holds no effects.
    pub fn is_empty(&self) -> bool { self.effects.is_empty() }

    /// Runs the sample through every effect in order. Returns the unchanged
    /// sample if [`bypass`](Self::bypass) is set.
    pub fn process(&mut self, sample: f32) -> f32 {
        if self.bypass { return sample; }
        let mut s = sample;
        for effect in &mut self.effects {
            s = effect.process(s);
        }
        s
    }
}

/// Simple gain effect — multiplies the input by a fixed scalar.
pub struct GainEffect {
    /// Linear gain scalar.
    pub gain: f32,
}
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
