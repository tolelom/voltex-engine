//! # voltex_audio
//!
//! In-house audio stack:
//!
//! - Decoders: [`wav::parse_wav`] (PCM 16/24/32-bit), [`ogg::parse_ogg`] +
//!   [`vorbis::decode_vorbis`] (simplified Vorbis I), bundled into
//!   [`decode_ogg_vorbis_bytes`] for convenience.
//! - Playback model: [`AudioClip`] holds samples, [`AudioSource`] is the per
//!   ECS instance, [`mixing::PlayingSound`] + [`mix_sounds`] do per-frame mix.
//! - Spatialisation: [`Listener`] + [`SpatialParams`] +
//!   [`compute_spatial_gains`], 3D distance/pan/HRTF helpers.
//! - Mixer: [`MixGroup`] / [`MixerState`] + dynamic [`MixGroupManager`] +
//!   [`AudioBus`] routing.
//! - Effects: [`Reverb`] / [`Echo`] / [`DelayLine`] / [`EffectChain`] /
//!   [`LowPassFilter`] occlusion / fade curves.
//! - Backends: [`AudioBackend`] trait with [`NullBackend`] always available;
//!   WASAPI on Windows.

#![warn(missing_docs)]

pub mod error;
pub mod audio_clip;
pub mod wav;
pub mod ogg;
pub mod vorbis;
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
pub use audio_source::{AudioSource, ClipId};
pub use wav::{parse_wav, generate_wav_bytes};
pub use ogg::parse_ogg;
pub use vorbis::decode_vorbis;

/// Decode an in-memory OGG/Vorbis byte slice into an [`AudioClip`].
///
/// Composes [`ogg::parse_ogg`] (container) and [`vorbis::decode_vorbis`]
/// (codec). Returns `Err` on malformed container, missing/invalid Vorbis
/// headers, or unsupported codec features (Vorbis simplified — scalar
/// codebooks + floor1 + residue2 only).
pub fn decode_ogg_vorbis_bytes(data: &[u8]) -> Result<AudioClip> {
    let packets = ogg::parse_ogg(data)?;
    vorbis::decode_vorbis(&packets)
}
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
