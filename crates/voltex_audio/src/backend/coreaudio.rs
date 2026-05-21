//! `CoreAudioBackend` — macOS PCM output via AudioToolbox's AudioQueue API.
//!
//! Format is locked to f32 little-endian stereo @48 kHz, matching the ALSA
//! backend, so any modern Mac running 13+ should be able to initialise it.
//! Requires the `AudioToolbox` framework at link time (always present on
//! macOS, no additional install).
//!
//! AudioQueue is callback-driven: the OS hands us empty buffers via the
//! output callback, and we fill them. We adapt that into our push-style
//! `AudioBackend::write()` trait with a Condvar-guarded free queue: write()
//! pops a buffer, copies samples in, and enqueues; the callback returns
//! consumed buffers to the free queue.
//!
//! NOTE: this module is compiled only when `cfg(target_os = "macos")`. The
//! initial version was written on a Windows development machine and has
//! not yet been runtime-verified against real Mac hardware — see
//! `docs/superpowers/specs/2026-05-21-b3-multi-os-build-design.md` § 6.

#![cfg(target_os = "macos")]
#![allow(non_camel_case_types, non_snake_case)]

use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use super::{AudioBackend, BackendError};

// ── AudioToolbox FFI (sufficient subset) ────────────────────────────────

type AudioQueueRef = *mut c_void;
type AudioQueueBufferRef = *mut AudioQueueBuffer;
type AudioQueueOutputCallback =
    extern "C" fn(*mut c_void, AudioQueueRef, AudioQueueBufferRef);

#[repr(C)]
struct AudioQueueBuffer {
    audio_data_bytes_capacity: u32,
    audio_data: *mut c_void,
    audio_data_byte_size: u32,
    user_data: *mut c_void,
    packet_description_capacity: u32,
    packet_descriptions: *mut c_void,
    packet_description_count: u32,
}

#[repr(C)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

// kAudioFormatLinearPCM = FourCC 'lpcm' (big-endian char order packed into u32)
const K_AUDIO_FORMAT_LINEAR_PCM: u32 = 0x6c70636d;
const K_AUDIO_FORMAT_FLAG_IS_FLOAT: u32 = 1 << 0;
const K_AUDIO_FORMAT_FLAG_IS_PACKED: u32 = 1 << 3;

#[link(name = "AudioToolbox", kind = "framework")]
unsafe extern "C" {
    fn AudioQueueNewOutput(
        in_format: *const AudioStreamBasicDescription,
        in_callback: AudioQueueOutputCallback,
        in_user_data: *mut c_void,
        in_callback_run_loop: *mut c_void,
        in_callback_run_loop_mode: *mut c_void,
        in_flags: u32,
        out_aq: *mut AudioQueueRef,
    ) -> i32;
    fn AudioQueueAllocateBuffer(
        in_aq: AudioQueueRef,
        in_buffer_byte_size: u32,
        out_buffer: *mut AudioQueueBufferRef,
    ) -> i32;
    fn AudioQueueEnqueueBuffer(
        in_aq: AudioQueueRef,
        in_buffer: AudioQueueBufferRef,
        in_num_packet_descs: u32,
        in_packet_descs: *const c_void,
    ) -> i32;
    fn AudioQueueStart(in_aq: AudioQueueRef, in_start_time: *const c_void) -> i32;
    fn AudioQueueStop(in_aq: AudioQueueRef, in_immediate: u8) -> i32;
    fn AudioQueueDispose(in_aq: AudioQueueRef, in_immediate: u8) -> i32;
}

// ── Shared state between OS callback thread and write() ─────────────────

struct FreeQueue {
    inner: Mutex<VecDeque<AudioQueueBufferRef>>,
    cv: Condvar,
}

// SAFETY: the raw pointers held inside the deque always come back to us via
// the AudioQueue callback and are only dereferenced inside write() while the
// queue is alive. We never share them across processes.
unsafe impl Send for FreeQueue {}
unsafe impl Sync for FreeQueue {}

extern "C" fn output_callback(
    user_data: *mut c_void,
    _queue: AudioQueueRef,
    buffer: AudioQueueBufferRef,
) {
    // Callback runs on an OS-owned thread. Unwinding across the FFI
    // boundary is UB, so swallow any panic from poisoned mutexes etc.
    let _ = std::panic::catch_unwind(|| {
        if user_data.is_null() || buffer.is_null() {
            return;
        }
        // SAFETY: user_data is the leaked Box<Arc<FreeQueue>> from new(),
        // and AudioQueueDispose() is guaranteed to have returned before
        // we free that box in shutdown().
        let arc = unsafe { &*(user_data as *const Arc<FreeQueue>) };
        if let Ok(mut guard) = arc.inner.lock() {
            guard.push_back(buffer);
            arc.cv.notify_one();
        }
    });
}

// ── Backend impl ────────────────────────────────────────────────────────

pub struct CoreAudioBackend {
    queue: AudioQueueRef,
    free: *mut Arc<FreeQueue>, // leaked Box; freed in shutdown after dispose
    sample_rate: u32,
    channels: u16,
    buffer_frames: u32,
}

// SAFETY: all raw pointers are owned by a single backend instance running on
// the audio worker thread (matching the ALSA pattern).
unsafe impl Send for CoreAudioBackend {}

impl CoreAudioBackend {
    pub fn new() -> Result<Self, BackendError> {
        let sample_rate = 48_000u32;
        let channels = 2u16;
        let buffer_frames = 1024u32;
        let n_buffers = 4u32;
        let bytes_per_frame = channels as u32 * 4; // f32 stereo
        let buffer_byte_size = buffer_frames * bytes_per_frame;

        let format = AudioStreamBasicDescription {
            sample_rate: sample_rate as f64,
            format_id: K_AUDIO_FORMAT_LINEAR_PCM,
            format_flags: K_AUDIO_FORMAT_FLAG_IS_FLOAT | K_AUDIO_FORMAT_FLAG_IS_PACKED,
            bytes_per_packet: bytes_per_frame,
            frames_per_packet: 1,
            bytes_per_frame,
            channels_per_frame: channels as u32,
            bits_per_channel: 32,
            reserved: 0,
        };

        let free = Arc::new(FreeQueue {
            inner: Mutex::new(VecDeque::with_capacity(n_buffers as usize)),
            cv: Condvar::new(),
        });
        let free_ptr: *mut Arc<FreeQueue> = Box::into_raw(Box::new(free.clone()));

        let mut queue: AudioQueueRef = std::ptr::null_mut();
        let rc = unsafe {
            AudioQueueNewOutput(
                &format,
                output_callback,
                free_ptr as *mut c_void,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut queue,
            )
        };
        if rc != 0 {
            // SAFETY: free_ptr came from Box::into_raw above and was not
            // shared with the OS yet (AudioQueueNewOutput failed).
            unsafe {
                drop(Box::from_raw(free_ptr));
            }
            return Err(BackendError::InitFailed(format!(
                "AudioQueueNewOutput: OSStatus {}",
                rc
            )));
        }

        // Allocate N buffers up front.
        let mut buffers: Vec<AudioQueueBufferRef> = Vec::with_capacity(n_buffers as usize);
        for _ in 0..n_buffers {
            let mut buf: AudioQueueBufferRef = std::ptr::null_mut();
            let rc = unsafe { AudioQueueAllocateBuffer(queue, buffer_byte_size, &mut buf) };
            if rc != 0 {
                // SAFETY: queue is alive; immediate dispose halts callbacks
                // before we drop the leaked Arc.
                unsafe {
                    AudioQueueDispose(queue, 1);
                    drop(Box::from_raw(free_ptr));
                }
                return Err(BackendError::InitFailed(format!(
                    "AudioQueueAllocateBuffer: OSStatus {}",
                    rc
                )));
            }
            buffers.push(buf);
        }

        // Prime with one zero-filled buffer so the queue starts producing
        // callbacks. The rest go straight to the free queue so write() can
        // pick them up without blocking.
        let starter = buffers[0];
        // SAFETY: `starter` was just allocated by AudioQueueAllocateBuffer.
        unsafe {
            std::ptr::write_bytes(
                (*starter).audio_data as *mut u8,
                0,
                buffer_byte_size as usize,
            );
            (*starter).audio_data_byte_size = buffer_byte_size;
            let rc = AudioQueueEnqueueBuffer(queue, starter, 0, std::ptr::null());
            if rc != 0 {
                AudioQueueDispose(queue, 1);
                drop(Box::from_raw(free_ptr));
                return Err(BackendError::InitFailed(format!(
                    "AudioQueueEnqueueBuffer (starter): OSStatus {}",
                    rc
                )));
            }
        }
        {
            let mut guard = free
                .inner
                .lock()
                .map_err(|e| BackendError::InitFailed(format!("free queue lock: {}", e)))?;
            for &b in &buffers[1..] {
                guard.push_back(b);
            }
        }

        let rc = unsafe { AudioQueueStart(queue, std::ptr::null()) };
        if rc != 0 {
            // SAFETY: same rationale as above.
            unsafe {
                AudioQueueDispose(queue, 1);
                drop(Box::from_raw(free_ptr));
            }
            return Err(BackendError::InitFailed(format!(
                "AudioQueueStart: OSStatus {}",
                rc
            )));
        }

        Ok(Self {
            queue,
            free: free_ptr,
            sample_rate,
            channels,
            buffer_frames,
        })
    }
}

impl AudioBackend for CoreAudioBackend {
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
        if samples.is_empty() {
            return Ok(0);
        }
        // SAFETY: `self.free` was leaked in new() and remains valid until
        // shutdown(), which we have not yet entered (we own &mut self).
        let arc = unsafe { &*self.free };
        let mut guard = arc
            .inner
            .lock()
            .map_err(|e| BackendError::WriteFailed(format!("free queue lock: {}", e)))?;
        let timeout = Duration::from_millis(100);
        let buffer = loop {
            if let Some(b) = guard.pop_front() {
                break b;
            }
            let (next_guard, timed_out) = arc
                .cv
                .wait_timeout(guard, timeout)
                .map_err(|e| BackendError::WriteFailed(format!("cv poisoned: {}", e)))?;
            guard = next_guard;
            if timed_out.timed_out() && guard.is_empty() {
                return Err(BackendError::WriteFailed(
                    "AudioQueue free buffer wait timed out (100ms)".into(),
                ));
            }
        };
        drop(guard);

        // Copy as much of `samples` as fits. The mixer is expected to call
        // with one buffer_frames-worth at a time, but we cap defensively.
        // SAFETY: `buffer` came from AudioQueueAllocateBuffer in new() and
        // is owned by us until we re-enqueue it just below.
        let cap_bytes = unsafe { (*buffer).audio_data_bytes_capacity } as usize;
        let bytes_in = samples.len() * std::mem::size_of::<f32>();
        let copy_bytes = bytes_in.min(cap_bytes);
        let frames = copy_bytes / (self.channels as usize * std::mem::size_of::<f32>());
        unsafe {
            std::ptr::copy_nonoverlapping(
                samples.as_ptr() as *const u8,
                (*buffer).audio_data as *mut u8,
                copy_bytes,
            );
            (*buffer).audio_data_byte_size = copy_bytes as u32;
            let rc = AudioQueueEnqueueBuffer(self.queue, buffer, 0, std::ptr::null());
            if rc != 0 {
                // Push the buffer back so we don't leak the slot.
                if let Ok(mut g) = arc.inner.lock() {
                    g.push_back(buffer);
                }
                return Err(BackendError::WriteFailed(format!(
                    "AudioQueueEnqueueBuffer: OSStatus {}",
                    rc
                )));
            }
        }
        Ok(frames)
    }
    fn shutdown(&mut self) {
        if !self.queue.is_null() {
            // immediate=1 ensures no further callbacks fire after this
            // returns, which is required before we free the leaked Arc.
            unsafe {
                AudioQueueStop(self.queue, 1);
                AudioQueueDispose(self.queue, 1);
            }
            self.queue = std::ptr::null_mut();
        }
        if !self.free.is_null() {
            // SAFETY: dispose returned above, so no callback will touch
            // the user_data pointer again.
            unsafe {
                drop(Box::from_raw(self.free));
            }
            self.free = std::ptr::null_mut();
        }
    }
}

impl Drop for CoreAudioBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // We cannot exercise AudioQueue without real macOS audio hardware, but
    // we can pin the format constants and confirm the module type-checks
    // on every macOS build.
    #[test]
    fn format_constants_match_apple_headers() {
        // 'lpcm' = 0x6C 0x70 0x63 0x6D
        assert_eq!(K_AUDIO_FORMAT_LINEAR_PCM, 0x6c70636d);
        assert_eq!(K_AUDIO_FORMAT_FLAG_IS_FLOAT, 0x01);
        assert_eq!(K_AUDIO_FORMAT_FLAG_IS_PACKED, 0x08);
    }

    #[test]
    fn asbd_struct_is_packed_as_expected() {
        // Apple's AudioStreamBasicDescription is 40 bytes on all platforms.
        assert_eq!(std::mem::size_of::<AudioStreamBasicDescription>(), 40);
    }
}
