//! AudioSystem: high-level audio playback management with a dedicated audio thread.

use std::sync::{Arc, mpsc};
use std::thread;

use crate::{AudioClip, PlayingSound, mix_sounds};

// ---------------------------------------------------------------------------
// AudioCommand
// ---------------------------------------------------------------------------

/// Commands sent to the audio thread.
pub enum AudioCommand {
    /// Start playing a clip.
    Play {
        clip_index: usize,
        volume: f32,
        looping: bool,
    },
    /// Stop all instances of a clip.
    Stop { clip_index: usize },
    /// Change volume of all playing instances of a clip.
    SetVolume { clip_index: usize, volume: f32 },
    /// Stop all currently playing sounds.
    StopAll,
    /// Shut down the audio thread.
    Shutdown,
}

// ---------------------------------------------------------------------------
// AudioSystem
// ---------------------------------------------------------------------------

/// Manages audio playback via a dedicated OS-level thread.
pub struct AudioSystem {
    sender: mpsc::Sender<AudioCommand>,
}

impl AudioSystem {
    /// Create an AudioSystem with the given pre-loaded clips.
    ///
    /// Spawns a background audio thread that owns the clips and drives the
    /// WASAPI device (on Windows) or runs in silent mode (other platforms).
    pub fn new(clips: Vec<AudioClip>) -> Result<Self, String> {
        let clips = Arc::new(clips);
        let (tx, rx) = mpsc::channel::<AudioCommand>();

        let clips_arc = Arc::clone(&clips);
        thread::Builder::new()
            .name("voltex_audio_thread".to_string())
            .spawn(move || {
                audio_thread(rx, clips_arc);
            })
            .map_err(|e| format!("Failed to spawn audio thread: {}", e))?;

        Ok(AudioSystem { sender: tx })
    }

    /// Start playing a clip at the given volume and looping setting.
    pub fn play(&self, clip_index: usize, volume: f32, looping: bool) {
        let _ = self.sender.send(AudioCommand::Play {
            clip_index,
            volume,
            looping,
        });
    }

    /// Stop all instances of the specified clip.
    pub fn stop(&self, clip_index: usize) {
        let _ = self.sender.send(AudioCommand::Stop { clip_index });
    }

    /// Set the volume for all playing instances of a clip.
    pub fn set_volume(&self, clip_index: usize, volume: f32) {
        let _ = self.sender.send(AudioCommand::SetVolume { clip_index, volume });
    }

    /// Stop all currently playing sounds.
    pub fn stop_all(&self) {
        let _ = self.sender.send(AudioCommand::StopAll);
    }
}

impl Drop for AudioSystem {
    fn drop(&mut self) {
        let _ = self.sender.send(AudioCommand::Shutdown);
    }
}

// ---------------------------------------------------------------------------
// Audio thread implementation
// ---------------------------------------------------------------------------

/// Main audio thread function.
///
/// On Windows, initializes WasapiDevice and drives playback.
/// On other platforms, runs in a silent "null" mode (processes commands only).
fn audio_thread(rx: mpsc::Receiver<AudioCommand>, clips: Arc<Vec<AudioClip>>) {
    #[cfg(target_os = "windows")]
    audio_thread_windows(rx, clips);

    #[cfg(not(target_os = "windows"))]
    audio_thread_null(rx, clips);
}

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn audio_thread_windows(rx: mpsc::Receiver<AudioCommand>, clips: Arc<Vec<AudioClip>>) {
    use crate::wasapi::WasapiDevice;

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

    loop {
        // Process all pending commands (non-blocking)
        loop {
            match rx.try_recv() {
                Ok(cmd) => {
                    match cmd {
                        AudioCommand::Play { clip_index, volume, looping } => {
                            playing.push(PlayingSound::new(clip_index, volume, looping));
                        }
                        AudioCommand::Stop { clip_index } => {
                            playing.retain(|s| s.clip_index != clip_index);
                        }
                        AudioCommand::SetVolume { clip_index, volume } => {
                            for s in playing.iter_mut() {
                                if s.clip_index == clip_index {
                                    s.volume = volume;
                                }
                            }
                        }
                        AudioCommand::StopAll => {
                            playing.clear();
                        }
                        AudioCommand::Shutdown => {
                            return;
                        }
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

        // Sleep ~5 ms between iterations
        thread::sleep(std::time::Duration::from_millis(5));
    }
}

// ---------------------------------------------------------------------------
// Null (non-Windows) implementation
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
fn audio_thread_null(rx: mpsc::Receiver<AudioCommand>, clips: Arc<Vec<AudioClip>>) {
    audio_thread_null_impl(rx, clips);
}

#[cfg(target_os = "windows")]
fn audio_thread_null(rx: mpsc::Receiver<AudioCommand>, _clips: Arc<Vec<AudioClip>>) {
    audio_thread_null_impl(rx, _clips);
}

fn audio_thread_null_impl(rx: mpsc::Receiver<AudioCommand>, _clips: Arc<Vec<AudioClip>>) {
    loop {
        match rx.recv() {
            Ok(AudioCommand::Shutdown) | Err(_) => return,
            Ok(_) => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AudioClip;

    fn silence_clip() -> AudioClip {
        AudioClip::new(vec![0.0f32; 44100], 44100, 1)
    }

    #[test]
    fn create_and_drop() {
        // AudioSystem should be created and dropped without panicking.
        // The audio thread is spawned but will run in null mode in CI.
        let _sys = AudioSystem::new(vec![silence_clip()])
            .expect("AudioSystem::new failed");
        // Drop happens here, Shutdown command is sent automatically.
    }

    #[test]
    fn send_commands() {
        let sys = AudioSystem::new(vec![silence_clip(), silence_clip()])
            .expect("AudioSystem::new failed");

        sys.play(0, 1.0, false);
        sys.play(1, 0.5, true);
        sys.set_volume(0, 0.8);
        sys.stop(0);
        sys.stop_all();
        // All sends must succeed without panic.
    }
}
