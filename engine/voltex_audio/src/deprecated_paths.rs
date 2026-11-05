//! Old public paths of the modules and items moved to [`crate::experimental`].
//!
//! Deprecated aliases kept so existing code still compiles; nothing in the
//! engine uses them. A module alias (`voltex_audio::reverb`, …) warns when the module
//! itself is imported with `use`; type and function aliases warn at every use.

// ── Modules ──────────────────────────────────────────────────────────────

/// Moved to [`experimental::async_loader`](crate::experimental::async_loader).
#[deprecated(note = "moved to `voltex_audio::experimental::async_loader`; not used by the engine")]
pub mod async_loader {
    pub use crate::experimental::async_loader::*;
}

/// Moved to [`experimental::audio_bus`](crate::experimental::audio_bus).
#[deprecated(note = "moved to `voltex_audio::experimental::audio_bus`; not used by the engine")]
pub mod audio_bus {
    pub use crate::experimental::audio_bus::*;
}

/// Moved to [`experimental::dynamic_groups`](crate::experimental::dynamic_groups).
#[deprecated(note = "moved to `voltex_audio::experimental::dynamic_groups`; not used by the engine")]
pub mod dynamic_groups {
    pub use crate::experimental::dynamic_groups::*;
}

/// Moved to [`experimental::effect_chain`](crate::experimental::effect_chain).
#[deprecated(note = "moved to `voltex_audio::experimental::effect_chain`; not used by the engine")]
pub mod effect_chain {
    pub use crate::experimental::effect_chain::*;
}

/// Moved to [`experimental::fade_curves`](crate::experimental::fade_curves).
#[deprecated(note = "moved to `voltex_audio::experimental::fade_curves`; not used by the engine")]
pub mod fade_curves {
    pub use crate::experimental::fade_curves::*;
}

/// Moved to [`experimental::hrtf`](crate::experimental::hrtf).
#[deprecated(note = "moved to `voltex_audio::experimental::hrtf`; not used by the engine")]
pub mod hrtf {
    pub use crate::experimental::hrtf::*;
}

/// Moved to [`experimental::occlusion`](crate::experimental::occlusion).
#[deprecated(note = "moved to `voltex_audio::experimental::occlusion`; not used by the engine")]
pub mod occlusion {
    pub use crate::experimental::occlusion::*;
}

/// Moved to [`experimental::reverb`](crate::experimental::reverb).
#[deprecated(note = "moved to `voltex_audio::experimental::reverb`; not used by the engine")]
pub mod reverb {
    pub use crate::experimental::reverb::*;
}

// ── Types that were re-exported at the crate root ────────────────────────

/// Moved to [`experimental::async_loader::AsyncAudioLoader`](crate::experimental::async_loader::AsyncAudioLoader).
#[deprecated(note = "moved to `voltex_audio::experimental::async_loader::AsyncAudioLoader`; not used by the engine")]
pub type AsyncAudioLoader = crate::experimental::async_loader::AsyncAudioLoader;

/// Moved to [`experimental::async_loader::AudioLoadState`](crate::experimental::async_loader::AudioLoadState).
#[deprecated(note = "moved to `voltex_audio::experimental::async_loader::AudioLoadState`; not used by the engine")]
pub type AudioLoadState = crate::experimental::async_loader::AudioLoadState;

/// Moved to [`experimental::audio_bus::AudioBus`](crate::experimental::audio_bus::AudioBus).
#[deprecated(note = "moved to `voltex_audio::experimental::audio_bus::AudioBus`; not used by the engine")]
pub type AudioBus = crate::experimental::audio_bus::AudioBus;

/// Moved to [`experimental::dynamic_groups::MixGroupManager`](crate::experimental::dynamic_groups::MixGroupManager).
#[deprecated(note = "moved to `voltex_audio::experimental::dynamic_groups::MixGroupManager`; not used by the engine")]
pub type MixGroupManager = crate::experimental::dynamic_groups::MixGroupManager;

/// Moved to [`experimental::effect_chain::EffectChain`](crate::experimental::effect_chain::EffectChain).
#[deprecated(note = "moved to `voltex_audio::experimental::effect_chain::EffectChain`; not used by the engine")]
pub type EffectChain = crate::experimental::effect_chain::EffectChain;

/// Moved to [`experimental::fade_curves::FadeCurve`](crate::experimental::fade_curves::FadeCurve).
#[deprecated(note = "moved to `voltex_audio::experimental::fade_curves::FadeCurve`; not used by the engine")]
pub type FadeCurve = crate::experimental::fade_curves::FadeCurve;

/// Moved to [`experimental::occlusion::LowPassFilter`](crate::experimental::occlusion::LowPassFilter).
#[deprecated(note = "moved to `voltex_audio::experimental::occlusion::LowPassFilter`; not used by the engine")]
pub type LowPassFilter = crate::experimental::occlusion::LowPassFilter;

/// Moved to [`experimental::occlusion::OcclusionResult`](crate::experimental::occlusion::OcclusionResult).
#[deprecated(note = "moved to `voltex_audio::experimental::occlusion::OcclusionResult`; not used by the engine")]
pub type OcclusionResult = crate::experimental::occlusion::OcclusionResult;

/// Moved to [`experimental::reverb::DelayLine`](crate::experimental::reverb::DelayLine).
#[deprecated(note = "moved to `voltex_audio::experimental::reverb::DelayLine`; not used by the engine")]
pub type DelayLine = crate::experimental::reverb::DelayLine;

/// Moved to [`experimental::reverb::Echo`](crate::experimental::reverb::Echo).
#[deprecated(note = "moved to `voltex_audio::experimental::reverb::Echo`; not used by the engine")]
pub type Echo = crate::experimental::reverb::Echo;

/// Moved to [`experimental::reverb::Reverb`](crate::experimental::reverb::Reverb).
#[deprecated(note = "moved to `voltex_audio::experimental::reverb::Reverb`; not used by the engine")]
pub type Reverb = crate::experimental::reverb::Reverb;

/// Old name of [`AudioLoadState`](crate::experimental::async_loader::AudioLoadState)
/// (itself deprecated: it clashed with `voltex_asset::LoadState`).
#[allow(deprecated)]
pub use crate::experimental::async_loader::LoadState;

// ── Traits that were re-exported at the crate root ───────────────────────
// A trait cannot be aliased, so the re-export carries `#[deprecated]`. rustc
// does not warn on deprecated re-exports; the attribute only marks the docs.

#[deprecated(note = "moved to `voltex_audio::experimental::effect_chain::AudioEffect`; not used by the engine")]
pub use crate::experimental::effect_chain::AudioEffect;

// ── Functions that were re-exported at the crate root ────────────────────

/// Moved to [`experimental::fade_curves::apply_fade`](crate::experimental::fade_curves::apply_fade).
#[deprecated(note = "moved to `voltex_audio::experimental::fade_curves::apply_fade`; not used by the engine")]
#[inline]
pub fn apply_fade(t: f32, curve: crate::experimental::fade_curves::FadeCurve) -> f32 {
    crate::experimental::fade_curves::apply_fade(t, curve)
}

/// Moved to [`experimental::occlusion::calculate_occlusion`](crate::experimental::occlusion::calculate_occlusion).
#[deprecated(note = "moved to `voltex_audio::experimental::occlusion::calculate_occlusion`; not used by the engine")]
#[inline]
pub fn calculate_occlusion(occlusion_factor: f32) -> crate::experimental::occlusion::OcclusionResult {
    crate::experimental::occlusion::calculate_occlusion(occlusion_factor)
}
