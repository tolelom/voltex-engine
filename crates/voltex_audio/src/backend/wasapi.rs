//! `WasapiBackend` — `AudioBackend` impl on top of the existing
//! `crate::wasapi::WasapiDevice` FFI wrapper.
//!
//! This is a thin adapter; the FFI heavy lifting stays in `src/wasapi.rs`.

use super::{AudioBackend, BackendError};
use crate::wasapi::WasapiDevice;

pub struct WasapiBackend {
    device: WasapiDevice,
}

impl WasapiBackend {
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
