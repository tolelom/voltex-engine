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

/// Decode in-memory audio bytes into an [`AudioClip`] by `path`'s extension:
/// `.wav` → [`parse_wav`], `.ogg` → [`decode_ogg_vorbis_bytes`]
/// (case-insensitive). The path is only consulted for its extension and error
/// messages — the bytes may come from anywhere (disk, `.vpkg` archive, network).
pub fn decode_clip_bytes(path: &std::path::Path, bytes: &[u8]) -> Result<AudioClip> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("wav") => parse_wav(bytes),
        Some("ogg") => decode_ogg_vorbis_bytes(bytes),
        _ => Err(AudioError::from(format!(
            "decode_clip_bytes: unsupported audio extension: {}",
            path.display()
        ))),
    }
}

/// Read an audio file from disk and decode it into an [`AudioClip`] —
/// `fs::read` + [`decode_clip_bytes`]. Errors on read failure, decode
/// failure, or an unsupported extension. The one-call path for editors /
/// hot-reload loops.
pub fn load_clip_file(path: &std::path::Path) -> Result<AudioClip> {
    let bytes = std::fs::read(path)?;
    decode_clip_bytes(path, &bytes)
}
pub use mixing::{PlayingSound, VoiceId, mix_sounds};
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

#[cfg(test)]
mod lib_tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("{}_{}", std::process::id(), name))
    }

    #[test]
    fn load_clip_file_decodes_wav_by_extension() {
        let path = temp_file("voltex_load_clip.wav");
        let wav = generate_wav_bytes(&[0.0, 0.5, -0.5, 0.25], 22050);
        std::fs::write(&path, wav).unwrap();

        let clip = load_clip_file(&path).expect("wav decodes");
        assert_eq!(clip.sample_rate, 22050);
        assert_eq!(clip.channels, 1);
        assert_eq!(clip.samples.len(), 4);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_clip_file_rejects_unknown_extension_and_missing_file() {
        let txt = temp_file("voltex_load_clip.txt");
        std::fs::write(&txt, b"not audio").unwrap();
        assert!(load_clip_file(&txt).is_err(), "unsupported extension is Err");
        let _ = std::fs::remove_file(&txt);

        let missing = temp_file("voltex_load_clip_missing.wav");
        assert!(load_clip_file(&missing).is_err(), "missing file is Err");
    }

    #[test]
    fn load_clip_file_propagates_decode_errors() {
        let path = temp_file("voltex_load_clip_corrupt.wav");
        std::fs::write(&path, b"RIFFnope").unwrap();
        assert!(load_clip_file(&path).is_err(), "corrupt wav is Err");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn decode_clip_bytes_decodes_wav_without_touching_disk() {
        // Path supplies only the extension — no file exists at it.
        let phantom = std::path::Path::new("vpkg://sounds/beep.WAV");
        let wav = generate_wav_bytes(&[0.25, -0.25], 44100);
        let clip = decode_clip_bytes(phantom, &wav).expect("wav bytes decode");
        assert_eq!(clip.sample_rate, 44100);
        assert_eq!(clip.samples.len(), 2);
    }

    #[test]
    fn decode_clip_bytes_rejects_unsupported_extension() {
        let err = decode_clip_bytes(std::path::Path::new("a.mp3"), &[0u8; 4]);
        assert!(err.is_err(), "mp3 is unsupported");
    }
}
