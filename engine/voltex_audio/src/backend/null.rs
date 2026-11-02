//! No-op backend. Used as a hard fallback when device init fails and as the
//! "headless / CI" mode for tests.

use super::{AudioBackend, BackendError};

/// Silent backend that advertises a sane sample rate but discards samples.
pub struct NullBackend {
    sample_rate: u32,
    channels: u16,
    buffer_frames: u32,
}

impl Default for NullBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl NullBackend {
    /// Creates a Null backend advertising 48 kHz stereo, 1024-frame buffers.
    pub fn new() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 2,
            buffer_frames: 1024,
        }
    }
}

impl AudioBackend for NullBackend {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn buffer_frames(&self) -> u32 {
        self.buffer_frames
    }
    fn write(&mut self, samples: &[f32]) -> Result<usize, BackendError> {
        // Pretend we consumed the whole buffer. Each frame is `channels` samples.
        Ok(samples.len() / self.channels as usize)
    }
    fn shutdown(&mut self) {
        // nothing to release
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::AudioBackend;

    #[test]
    fn null_backend_reports_sane_defaults() {
        let b = NullBackend::new();
        assert_eq!(b.sample_rate(), 48_000);
        assert_eq!(b.channels(), 2);
        assert_eq!(b.buffer_frames(), 1024);
    }

    #[test]
    fn null_backend_write_succeeds_and_returns_frames() {
        let mut b = NullBackend::new();
        let samples = vec![0.0f32; 200]; // 100 stereo frames
        let frames = b.write(&samples).unwrap();
        assert_eq!(frames, 100);
    }

    #[test]
    fn shutdown_is_idempotent() {
        let mut b = NullBackend::new();
        b.shutdown();
        b.shutdown(); // calling again must not panic
    }
}
