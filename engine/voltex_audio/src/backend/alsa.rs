//! `AlsaBackend` — Linux PCM output via libasound's "default" plug device.
//!
//! Format is locked to f32 little-endian stereo @48 kHz with ALSA's built-in
//! resampler enabled, so any modern Linux machine should be able to
//! initialise it. Requires `libasound2` (Debian/Ubuntu) or `alsa-lib` (Arch)
//! at runtime.
//!
//! NOTE: this module is compiled only when `cfg(target_os = "linux")`. The
//! initial version was written on Windows and has not yet been runtime-
//! verified against a real Linux machine.

#![cfg(target_os = "linux")]
#![allow(non_camel_case_types, non_snake_case)]

use std::ffi::{c_int, c_long, c_uint, c_ulong, c_void, CString};

use super::{AudioBackend, BackendError};

// ── libasound FFI (sufficient subset) ────────────────────────────────────

type snd_pcm_t = c_void;

const SND_PCM_STREAM_PLAYBACK: c_int = 0;
const SND_PCM_FORMAT_FLOAT_LE: c_int = 14;
const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;

const EPIPE: c_int = 32;
const ESTRPIPE: c_int = 86;

#[link(name = "asound")]
unsafe extern "C" {
    fn snd_pcm_open(
        handle: *mut *mut snd_pcm_t,
        name: *const u8,
        stream: c_int,
        mode: c_int,
    ) -> c_int;
    fn snd_pcm_set_params(
        pcm: *mut snd_pcm_t,
        format: c_int,
        access: c_int,
        channels: c_uint,
        rate: c_uint,
        soft_resample: c_int,
        latency_us: c_uint,
    ) -> c_int;
    // `size` is `snd_pcm_uframes_t` (unsigned long) and the result is
    // `snd_pcm_sframes_t` (long) in <alsa/pcm.h>.
    fn snd_pcm_writei(pcm: *mut snd_pcm_t, buffer: *const f32, size: c_ulong) -> c_long;
    fn snd_pcm_prepare(pcm: *mut snd_pcm_t) -> c_int;
    fn snd_pcm_drop(pcm: *mut snd_pcm_t) -> c_int;
    fn snd_pcm_close(pcm: *mut snd_pcm_t) -> c_int;
    fn snd_strerror(err: c_int) -> *const u8;
}

/// Human-readable text for an ALSA error code.
///
/// # Safety
/// Only calls `snd_strerror`, which accepts any code; marked `unsafe` because
/// it trusts the returned pointer to be NUL-terminated static text.
unsafe fn alsa_strerror(err: c_int) -> String {
    // SAFETY: snd_strerror takes any int and returns static text or NULL.
    let ptr = unsafe { snd_strerror(err) };
    if ptr.is_null() {
        return format!("snd error {}", err);
    }
    // Walk to NUL ourselves so we don't pull in libc::strlen.
    let mut len = 0usize;
    // SAFETY: ptr is non-null and NUL-terminated (snd_strerror contract), so
    // every byte up to and including the NUL is readable.
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: the `len` bytes before the NUL were just read above.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(slice).into_owned()
}

// ── Backend impl ─────────────────────────────────────────────────────────

/// `AudioBackend` over an ALSA PCM handle on the "default" device.
pub struct AlsaBackend {
    pcm: *mut snd_pcm_t,
    sample_rate: u32,
    channels: u16,
    buffer_frames: u32,
}

// SAFETY: the PCM handle is owned exclusively by this struct (never copied out)
// and every FFI call goes through `&mut self` or `Drop`, so only one thread
// touches it at a time. ALSA PCM handles may be used from any thread as long
// as access is not concurrent.
unsafe impl Send for AlsaBackend {}

impl AlsaBackend {
    /// Opens the "default" PCM device as f32 stereo @48 kHz; returns
    /// `BackendError::InitFailed` if ALSA cannot open or configure it.
    pub fn new() -> Result<Self, BackendError> {
        let sample_rate = 48_000u32;
        let channels = 2u16;
        let buffer_frames = 1024u32;
        let latency_us = 100_000u32; // 100 ms

        let device = CString::new("default").unwrap();
        let mut pcm: *mut snd_pcm_t = std::ptr::null_mut();
        // SAFETY: `device` is a valid C string that outlives the call; `pcm`
        // is only passed on to set_params/close after snd_pcm_open succeeded.
        unsafe {
            let rc = snd_pcm_open(
                &mut pcm,
                device.as_ptr().cast::<u8>(),
                SND_PCM_STREAM_PLAYBACK,
                0,
            );
            if rc < 0 {
                return Err(BackendError::InitFailed(format!(
                    "snd_pcm_open: {}",
                    alsa_strerror(rc)
                )));
            }
            let rc = snd_pcm_set_params(
                pcm,
                SND_PCM_FORMAT_FLOAT_LE,
                SND_PCM_ACCESS_RW_INTERLEAVED,
                channels as c_uint,
                sample_rate as c_uint,
                1, // soft_resample = enable
                latency_us as c_uint,
            );
            if rc < 0 {
                let msg = alsa_strerror(rc);
                snd_pcm_close(pcm);
                return Err(BackendError::InitFailed(format!(
                    "snd_pcm_set_params: {}",
                    msg
                )));
            }
        }
        Ok(Self {
            pcm,
            sample_rate,
            channels,
            buffer_frames,
        })
    }
}

impl AudioBackend for AlsaBackend {
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
        let frames = (samples.len() / self.channels as usize) as c_ulong;
        if frames == 0 {
            return Ok(0);
        }
        if self.pcm.is_null() {
            return Err(BackendError::WriteFailed("ALSA backend is shut down".into()));
        }
        // SAFETY: `self.pcm` is non-null (checked above), so it is the handle
        // opened in `new`; only `shutdown` closes it, and it nulls the pointer.
        // `samples` holds at least `frames * channels` interleaved f32 values.
        unsafe {
            let written = snd_pcm_writei(self.pcm, samples.as_ptr(), frames);
            if written >= 0 {
                return Ok(written as usize);
            }
            // Recover from underrun (EPIPE) or suspend (ESTRPIPE).
            let err = -(written as c_int);
            if err == EPIPE || err == ESTRPIPE {
                let rc = snd_pcm_prepare(self.pcm);
                if rc < 0 {
                    return Err(BackendError::WriteFailed(format!(
                        "snd_pcm_prepare after underrun: {}",
                        alsa_strerror(rc)
                    )));
                }
                // Retry once.
                let retry = snd_pcm_writei(self.pcm, samples.as_ptr(), frames);
                if retry >= 0 {
                    return Ok(retry as usize);
                }
                return Err(BackendError::WriteFailed(format!(
                    "snd_pcm_writei retry: {}",
                    alsa_strerror(-(retry as c_int))
                )));
            }
            Err(BackendError::WriteFailed(format!(
                "snd_pcm_writei: {}",
                alsa_strerror(err)
            )))
        }
    }
    fn shutdown(&mut self) {
        if !self.pcm.is_null() {
            // SAFETY: non-null handle from snd_pcm_open, closed exactly once
            // (nulled right after).
            unsafe {
                snd_pcm_drop(self.pcm);
                snd_pcm_close(self.pcm);
            }
            self.pcm = std::ptr::null_mut();
        }
    }
}

impl Drop for AlsaBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::AudioBackend;

    // We cannot exercise the FFI on every Linux CI runner (no device + no
    // libasound on minimal images), but we *can* assert the module compiled
    // and basic types are sound.
    #[test]
    fn alsa_module_compiles_on_linux() {
        let _ = super::SND_PCM_STREAM_PLAYBACK;
    }

    #[test]
    fn write_after_shutdown_is_an_error_not_a_null_handle_call() {
        // A backend whose device is already closed (pcm = NULL), as after shutdown().
        let mut backend = AlsaBackend {
            pcm: std::ptr::null_mut(),
            sample_rate: 48_000,
            channels: 2,
            buffer_frames: 1024,
        };
        assert!(backend.write(&[0.0; 128]).is_err());
    }

    /// Real-hardware smoke test (F6). Opens the default ALSA device, pushes
    /// 64 frames of silence, and tears down. Skipped by default (`#[ignore]`)
    /// because most CI runners have no audio hardware; gracefully returns
    /// instead of panicking when `AlsaBackend::new()` fails.
    ///
    /// Run on a real Linux box with:
    ///   cargo test -p voltex_audio --lib backend::alsa -- --ignored --nocapture
    #[test]
    #[ignore]
    fn alsa_real_device_smoke() {
        let mut backend = match AlsaBackend::new() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skipping: no ALSA device or libasound: {e}");
                return;
            }
        };
        let frames = backend.buffer_frames() as usize;
        let channels = backend.channels() as usize;
        let silence = vec![0.0_f32; frames * channels];
        let written = backend
            .write(&silence)
            .expect("write to default ALSA device");
        assert!(written > 0, "ALSA backend wrote 0 samples");
        backend.shutdown();
    }
}
