//! `WasapiBackend` — `AudioBackend` impl on top of the existing
//! `crate::wasapi::WasapiDevice` FFI wrapper.
//!
//! This is a thin adapter; the FFI heavy lifting stays in `src/wasapi.rs`.

use super::{AudioBackend, BackendError};
use crate::wasapi::WasapiDevice;

/// `AudioBackend` adapter over the WASAPI FFI wrapper.
pub struct WasapiBackend {
    device: WasapiDevice,
}

impl WasapiBackend {
    /// Initialises a WASAPI device; returns `BackendError::InitFailed` if the
    /// underlying device handle could not be obtained.
    pub fn new() -> Result<Self, BackendError> {
        WasapiDevice::new()
            .map(|device| WasapiBackend { device })
            .map_err(|e| BackendError::InitFailed(e.to_string()))
    }
}

impl AudioBackend for WasapiBackend {
    fn sample_rate(&self) -> u32 {
        self.device.sample_rate
    }
    fn channels(&self) -> u16 {
        self.device.channels
    }
    fn buffer_frames(&self) -> u32 {
        self.device.buffer_frames()
    }
    fn write(&mut self, samples: &[f32]) -> Result<usize, BackendError> {
        self.device
            .write_samples(samples)
            .map_err(|e| BackendError::WriteFailed(e.to_string()))
    }
    fn shutdown(&mut self) {
        // WasapiDevice's Drop releases COM resources on its own.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real-hardware smoke test (F6). Opens the default WASAPI output and
    /// pushes 64 frames of silence. Skipped by default (`#[ignore]`) because
    /// CI Windows runners may have no audio device; gracefully returns on
    /// init failure.
    ///
    /// Run on a real Windows box with:
    ///   cargo test -p voltex_audio --lib backend::wasapi -- --ignored --nocapture
    #[test]
    #[ignore]
    fn wasapi_real_device_smoke() {
        let mut backend = match WasapiBackend::new() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skipping: no WASAPI device: {e}");
                return;
            }
        };
        let frames = backend.buffer_frames() as usize;
        let channels = backend.channels() as usize;
        let silence = vec![0.0_f32; frames * channels];
        let written = backend
            .write(&silence)
            .expect("write to default WASAPI device");
        assert!(written > 0, "WASAPI backend wrote 0 samples");
        backend.shutdown();
    }
}
