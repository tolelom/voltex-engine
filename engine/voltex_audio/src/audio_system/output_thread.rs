//! The audio worker thread: mixes playing sounds and writes them to the OS
//! device (WASAPI on Windows, ALSA / CoreAudio elsewhere), or runs silently
//! when there is no device. Commands arrive from [`super::AudioSystem`].

use std::sync::mpsc;
use std::thread;

use super::{apply_command, replace_clip_in, AudioCommand};
use crate::{mix_group::MixerState, spatial::Listener, AudioClip, PlayingSound};

// ---------------------------------------------------------------------------
// Audio thread implementation
// ---------------------------------------------------------------------------

/// Main audio thread function.
///
/// On Windows, initializes `wasapi_ffi::WasapiDevice` and drives playback.
/// Elsewhere it opens the OS device through [`crate::backend::open_device_backend`]
/// (ALSA / CoreAudio). Without a device it runs in a silent "null" mode that
/// only processes commands.
pub(super) fn audio_thread(rx: mpsc::Receiver<AudioCommand>, clips: Vec<AudioClip>) {
    #[cfg(target_os = "windows")]
    audio_thread_windows(rx, clips);

    #[cfg(not(target_os = "windows"))]
    match crate::backend::open_device_backend() {
        Some(backend) => audio_thread_backend(rx, clips, backend),
        None => audio_thread_null(rx, clips),
    }
}

// ---------------------------------------------------------------------------
// Linux / macOS implementation (ALSA / CoreAudio through `AudioBackend`)
// ---------------------------------------------------------------------------

/// Mixes one device buffer per iteration and pushes it to `backend`. The
/// backend's `write` blocks until the device has room, so it paces the loop;
/// nothing else sleeps. Silence is written while nothing plays to keep the
/// device fed.
#[cfg(not(target_os = "windows"))]
fn audio_thread_backend(
    rx: mpsc::Receiver<AudioCommand>,
    mut clips: Vec<AudioClip>,
    mut backend: Box<dyn crate::backend::AudioBackend>,
) {
    use crate::mix_sounds;

    let sample_rate = backend.sample_rate();
    let channels = backend.channels();
    let buffer_frames = backend.buffer_frames() as usize;
    let buffer_duration = std::time::Duration::from_secs_f64(buffer_frames as f64 / sample_rate as f64);

    let mut playing: Vec<PlayingSound> = Vec::new();
    let mut output: Vec<f32> = Vec::new();
    let mut listener = Listener::default();
    let mut mixer = MixerState::new();
    let mut write_failing = false;

    loop {
        // Advance mixer fades by the length of one buffer.
        mixer.tick(buffer_duration.as_secs_f32());

        loop {
            match rx.try_recv() {
                Ok(cmd) => {
                    if apply_command(cmd, &mut playing, &mut clips, &mut listener, &mut mixer) {
                        return;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }

        // `mix_sounds` always fills exactly one buffer (zeros when idle).
        mix_sounds(
            &mut output,
            &mut playing,
            &clips,
            sample_rate,
            channels,
            buffer_frames,
            &listener,
            &mixer,
        );
        match backend.write(&output) {
            Ok(_) => write_failing = false,
            Err(e) => {
                // Log once per failure streak; wait one buffer instead of
                // spinning while the device is gone.
                if !write_failing {
                    eprintln!("[voltex_audio] audio write failed: {}", e);
                    write_failing = true;
                }
                thread::sleep(buffer_duration);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

/// Period of one Windows audio-thread iteration: mixer fades advance by this
/// much and the thread sleeps this long between mix/write passes.
#[cfg(target_os = "windows")]
const MIX_TICK: std::time::Duration = std::time::Duration::from_millis(5);

#[cfg(target_os = "windows")]
fn audio_thread_windows(rx: mpsc::Receiver<AudioCommand>, mut clips: Vec<AudioClip>) {
    use crate::mix_sounds;
    use crate::wasapi_ffi::WasapiDevice;

    let device = match WasapiDevice::new() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[voltex_audio] WASAPI init failed: {}. Running in silent mode.", e);
            audio_thread_null(rx, clips);
            return;
        }
    };

    let device_sample_rate = device.sample_rate;
    let device_channels = device.channels;
    let buffer_frames = device.buffer_frames() as usize;

    let mut playing: Vec<PlayingSound> = Vec::new();
    let mut output: Vec<f32> = Vec::new();
    let mut listener = Listener::default();
    let mut mixer = MixerState::new();

    loop {
        // Advance mixer fades by one iteration period.
        mixer.tick(MIX_TICK.as_secs_f32());

        // Process all pending commands (non-blocking)
        loop {
            match rx.try_recv() {
                Ok(cmd) => {
                    if apply_command(cmd, &mut playing, &mut clips, &mut listener, &mut mixer) {
                        return;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }

        // Mix and write audio
        if !playing.is_empty() || !output.is_empty() {
            mix_sounds(
                &mut output,
                &mut playing,
                &clips,
                device_sample_rate,
                device_channels,
                buffer_frames,
                &listener,
                &mixer,
            );

            if let Err(e) = device.write_samples(&output) {
                eprintln!("[voltex_audio] write_samples error: {}", e);
            }
        } else {
            // Write silence to keep the device happy
            let silence_len = buffer_frames * device_channels as usize;
            let silence = vec![0.0f32; silence_len];
            let _ = device.write_samples(&silence);
        }

        thread::sleep(MIX_TICK);
    }
}

// ---------------------------------------------------------------------------
// Null (no output device) implementation
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
fn audio_thread_null(rx: mpsc::Receiver<AudioCommand>, clips: Vec<AudioClip>) {
    audio_thread_null_impl(rx, clips);
}

#[cfg(target_os = "windows")]
fn audio_thread_null(rx: mpsc::Receiver<AudioCommand>, clips: Vec<AudioClip>) {
    audio_thread_null_impl(rx, clips);
}

fn audio_thread_null_impl(rx: mpsc::Receiver<AudioCommand>, mut clips: Vec<AudioClip>) {
    loop {
        match rx.recv() {
            Ok(AudioCommand::Shutdown) | Err(_) => return,
            // Keep the clip list in lockstep even in silent mode so `play`
            // after `add_clip` stays index-consistent if a real backend is
            // ever swapped in mid-session (and for symmetry with windows).
            Ok(AudioCommand::AddClip { clip }) => clips.push(clip),
            Ok(AudioCommand::ReplaceClip { clip_index, clip }) => {
                replace_clip_in(&mut clips, clip_index, clip);
            }
            Ok(_) => {}
        }
    }
}
