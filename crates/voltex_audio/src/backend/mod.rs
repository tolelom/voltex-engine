//! Audio output backend abstraction.
//!
//! The audio thread does not care whether samples end up on WASAPI, ALSA, or
//! `/dev/null` — it just calls `write()`. `make_backend()` picks the best
//! candidate for the current OS at runtime, with a hard fallback to
//! `NullBackend` whenever device init fails.

use std::fmt;

#[derive(Debug)]
pub enum BackendError {
    InitFailed(String),
    WriteFailed(String),
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
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u16;
    fn buffer_frames(&self) -> u32;
    fn write(&mut self, samples: &[f32]) -> Result<usize, BackendError>;
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
