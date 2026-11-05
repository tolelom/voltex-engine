//! Audio output backend abstraction.
//!
//! A push-style `write()` interface over WASAPI / ALSA / CoreAudio /
//! `NullBackend`. `make_backend()` picks the best candidate for the current OS
//! at runtime, with a hard fallback to `NullBackend` whenever device init
//! fails.
//!
//! **Not wired into [`AudioSystem`](crate::AudioSystem) yet**: its audio thread
//! drives `crate::wasapi_ffi::WasapiDevice` directly on Windows and runs
//! silent (commands only, no output) on every other OS. Nothing in the engine
//! calls `make_backend()` today.

use std::fmt;

/// Errors a backend may return from init / write.
#[derive(Debug)]
pub enum BackendError {
    /// Device handle could not be acquired; carries a human-readable reason.
    InitFailed(String),
    /// `write()` failed mid-stream; carries the OS-level error string.
    WriteFailed(String),
    /// Backend isn't supported on the current OS or feature build.
    NotSupported,
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::InitFailed(s) => write!(f, "audio backend init failed: {}", s),
            BackendError::WriteFailed(s) => write!(f, "audio backend write failed: {}", s),
            BackendError::NotSupported => write!(f, "audio backend not supported on this platform"),
        }
    }
}

impl std::error::Error for BackendError {}

/// Per-platform PCM output. Implementations are owned by the audio worker
/// thread, so `Send` is required but `Sync` is not.
pub trait AudioBackend: Send {
    /// Output sample rate in Hz that the backend is currently feeding.
    fn sample_rate(&self) -> u32;
    /// Output channel count (1 = mono, 2 = stereo).
    fn channels(&self) -> u16;
    /// Per-call buffer size in frames (samples per channel).
    fn buffer_frames(&self) -> u32;
    /// Pushes interleaved PCM samples to the device; returns the number written.
    fn write(&mut self, samples: &[f32]) -> Result<usize, BackendError>;
    /// Releases the device handle. Safe to call multiple times.
    fn shutdown(&mut self);
}

pub mod null;
pub use null::NullBackend;

#[cfg(target_os = "windows")]
pub mod wasapi;
#[cfg(target_os = "windows")]
pub use wasapi::WasapiBackend;

#[cfg(target_os = "linux")]
pub mod alsa;
#[cfg(target_os = "linux")]
pub use alsa::AlsaBackend;

#[cfg(target_os = "macos")]
pub mod coreaudio;
#[cfg(target_os = "macos")]
pub use coreaudio::CoreAudioBackend;

/// Pick the best backend the OS offers, or fall back to `NullBackend` so the
/// audio thread can keep running silently in headless or device-less envs.
pub fn make_backend() -> Box<dyn AudioBackend> {
    #[cfg(target_os = "windows")]
    {
        match WasapiBackend::new() {
            Ok(b) => return Box::new(b),
            Err(e) => eprintln!("[voltex_audio] WASAPI init failed: {} — falling back to null", e),
        }
    }
    #[cfg(target_os = "linux")]
    {
        match AlsaBackend::new() {
            Ok(b) => return Box::new(b),
            Err(e) => eprintln!("[voltex_audio] ALSA init failed: {} — falling back to null", e),
        }
    }
    #[cfg(target_os = "macos")]
    {
        match CoreAudioBackend::new() {
            Ok(b) => return Box::new(b),
            Err(e) => eprintln!("[voltex_audio] CoreAudio init failed: {} — falling back to null", e),
        }
    }
    Box::new(NullBackend::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_error_display_includes_message() {
        let e = BackendError::InitFailed("oh no".to_string());
        let s = format!("{}", e);
        assert!(s.contains("oh no"));
    }

    #[test]
    fn make_backend_returns_some_backend() {
        // Always returns at least the null fallback — must not panic, and the
        // returned trait object must report consistent metadata.
        let b = make_backend();
        let _ = b.sample_rate();
        let _ = b.channels();
        let _ = b.buffer_frames();
    }
}
