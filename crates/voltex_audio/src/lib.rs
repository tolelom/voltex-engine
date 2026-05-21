pub mod error;
pub mod audio_clip;
pub mod wav;
// `ogg` + `vorbis` are intentionally NOT registered here in v1.0.x.
// Their source exists under src/ but is not in the compilation graph —
// v1.0.8 surfaced this as an unintentional deferral from Phase 6-1.
// Wiring (+ clippy/audio-path integration) is scheduled for v1.0.9.
pub mod mixing;
#[cfg(target_os = "windows")]
pub mod wasapi;
pub mod backend;
pub mod audio_system;
pub mod spatial;
pub mod hrtf;
pub mod mix_group;
pub mod audio_source;
pub mod reverb;
pub mod occlusion;

pub use error::{AudioError, Result};
pub use audio_clip::AudioClip;
pub use audio_source::AudioSource;
pub use wav::{parse_wav, generate_wav_bytes};
pub use mixing::{PlayingSound, mix_sounds};
pub use audio_system::AudioSystem;
pub use spatial::{Listener, SpatialParams, distance_attenuation, stereo_pan, compute_spatial_gains};
pub use mix_group::{MixGroup, MixerState};
pub use reverb::{Reverb, Echo, DelayLine};
pub use occlusion::{OcclusionResult, LowPassFilter, calculate_occlusion};
pub mod fade_curves;
pub use fade_curves::{FadeCurve, apply_fade};
pub mod dynamic_groups;
pub use dynamic_groups::MixGroupManager;
pub mod audio_bus;
pub use audio_bus::AudioBus;
pub mod async_loader;
pub use async_loader::AsyncAudioLoader;
pub mod effect_chain;
pub use effect_chain::{AudioEffect, EffectChain};
pub use backend::{AudioBackend, BackendError, NullBackend, make_backend};
#[cfg(target_os = "windows")]
pub use backend::WasapiBackend;
#[cfg(target_os = "linux")]
pub use backend::AlsaBackend;
