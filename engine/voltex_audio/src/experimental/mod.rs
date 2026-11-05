//! Experimental audio pieces.
//!
//! Implemented and unit-tested, but not used by the engine runtime yet.
//! APIs here may change or be removed.
//!
//! [`AudioSystem`](crate::AudioSystem)'s mixer does not run any of these: it
//! mixes through [`mixing`](crate::mixing), [`MixGroup`](crate::MixGroup) and
//! [`spatial`](crate::spatial) only. The old top-level paths
//! (`voltex_audio::Reverb`, `voltex_audio::hrtf`, …) still resolve but are
//! deprecated.

// Loading
pub mod async_loader;
// Mixing
pub mod audio_bus;
pub mod dynamic_groups;
// Effects
pub mod effect_chain;
pub mod fade_curves;
pub mod reverb;
// 3D audio
pub mod hrtf;
pub mod occlusion;
