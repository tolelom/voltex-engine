//! AudioSystem: high-level audio playback management with a dedicated audio thread.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;

use crate::mixing::VoiceId;
use crate::AudioClip;
use crate::spatial::SpatialParams;
use crate::mix_group::MixGroup;
use crate::{PlayingSound, spatial::Listener, mix_group::MixerState};

// ---------------------------------------------------------------------------
// AudioCommand
// ---------------------------------------------------------------------------

/// Commands sent to the audio thread.
pub(crate) enum AudioCommand {
    /// Start playing a clip.
    Play {
        /// Clip index into the system's clip list.
        clip_index: usize,
        /// Linear volume `[0.0, 1.0]`.
        volume: f32,
        /// Whether the clip restarts when it ends.
        looping: bool,
        /// Pre-allocated voice handle for this instance.
        voice: VoiceId,
    },
    /// Start playing a clip with 3D spatial parameters.
    Play3d {
        /// Clip index into the system's clip list.
        clip_index: usize,
        /// Linear volume `[0.0, 1.0]`.
        volume: f32,
        /// Whether the clip restarts when it ends.
        looping: bool,
        /// 3D emitter parameters (position + min/max distance).
        spatial: SpatialParams,
        /// Pre-allocated voice handle for this instance.
        voice: VoiceId,
    },
    /// Update the listener position and orientation.
    SetListener {
        /// Listener world position.
        position: voltex_math::Vec3,
        /// Listener forward direction (unit).
        forward: voltex_math::Vec3,
        /// Listener right direction (unit).
        right: voltex_math::Vec3,
    },
    /// Stop all instances of a clip.
    Stop {
        /// Clip index whose instances should stop.
        clip_index: usize,
    },
    /// Stop exactly one playback instance by its voice handle.
    StopVoice {
        /// Voice handle returned by `play` / `play_3d`.
        voice: VoiceId,
    },
    /// Change volume of all playing instances of a clip.
    SetVolume {
        /// Clip index whose instances are affected.
        clip_index: usize,
        /// New linear volume `[0.0, 1.0]`.
        volume: f32,
    },
    /// Stop all currently playing sounds.
    StopAll,
    /// Set volume for a mix group immediately.
    SetGroupVolume {
        /// Target mix group.
        group: MixGroup,
        /// New volume.
        volume: f32,
    },
    /// Fade a mix group to a target volume over a duration (seconds).
    FadeGroup {
        /// Target mix group.
        group: MixGroup,
        /// End-of-fade volume.
        target: f32,
        /// Fade length in seconds.
        duration: f32,
    },
    /// Append a clip to the thread's clip list (hot-load).
    AddClip {
        /// The decoded clip to append.
        clip: AudioClip,
    },
    /// Replace the clip at `clip_index` in place (hot-reload). Out-of-range
    /// indices are ignored.
    ReplaceClip {
        /// Index of the clip to replace.
        clip_index: usize,
        /// The new clip data.
        clip: AudioClip,
    },
    /// Shut down the audio thread.
    Shutdown,
}

/// Replace `clips[clip_index]` with `clip`, returning whether the index was
/// in range. Out-of-range is a silent no-op — consistent with the
/// fire-and-forget contract of the other clip commands (`stop`, `set_volume`).
fn replace_clip_in(clips: &mut [AudioClip], clip_index: usize, clip: AudioClip) -> bool {
    if let Some(slot) = clips.get_mut(clip_index) {
        *slot = clip;
        true
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// AudioSystem
// ---------------------------------------------------------------------------

/// Manages audio playback via a dedicated OS-level thread.
pub struct AudioSystem {
    sender: mpsc::Sender<AudioCommand>,
    /// Number of clips the thread holds (initial + added). Hands out the next
    /// `add_clip` index; stays in lockstep with the thread's `Vec` because
    /// `AddClip` commands are processed in mpsc FIFO order.
    clip_count: usize,
    /// Next [`VoiceId`] to hand out; starts at 1 (`VoiceId(0)` = anonymous).
    next_voice: AtomicU64,
}

impl AudioSystem {
    /// Create an AudioSystem with the given pre-loaded clips.
    ///
    /// Spawns a background audio thread that owns the clips and drives the
    /// output device: WASAPI on Windows, ALSA on Linux, CoreAudio on macOS.
    /// Without a usable device (headless, CI) it runs silently.
    pub fn new(clips: Vec<AudioClip>) -> crate::Result<Self> {
        let clip_count = clips.len();
        let (tx, rx) = mpsc::channel::<AudioCommand>();

        thread::Builder::new()
            .name("voltex_audio_thread".to_string())
            .spawn(move || {
                audio_thread(rx, clips);
            })
            .map_err(|e| crate::AudioError::from(format!("Failed to spawn audio thread: {}", e)))?;

        Ok(AudioSystem { sender: tx, clip_count, next_voice: AtomicU64::new(1) })
    }

    /// Allocate a fresh voice handle (thread-safe, monotonically increasing).
    fn alloc_voice(&self) -> VoiceId {
        VoiceId(self.next_voice.fetch_add(1, Ordering::Relaxed))
    }

    /// Hot-load: append a decoded clip at runtime and return its `clip_index`,
    /// usable with [`Self::play`] immediately (mpsc FIFO guarantees the thread
    /// pushes the clip before processing any later command). Like every other
    /// command, the send is fire-and-forget: if the audio thread has died
    /// (panic), the index is still handed out but all playback is silent.
    pub fn add_clip(&mut self, clip: AudioClip) -> usize {
        let index = self.clip_count;
        self.clip_count += 1;
        let _ = self.sender.send(AudioCommand::AddClip { clip });
        index
    }

    /// Hot-reload: replace the clip behind an existing `clip_index` in place.
    /// Instances already playing sample the new data on the next mix block; if
    /// their position exceeds the new length they end (or loop) naturally.
    /// Out-of-range indices are ignored (fire-and-forget, like [`Self::stop`]).
    pub fn replace_clip(&self, clip_index: usize, clip: AudioClip) {
        let _ = self.sender.send(AudioCommand::ReplaceClip { clip_index, clip });
    }

    /// Start playing a clip at the given volume and looping setting.
    ///
    /// Returns the instance's [`VoiceId`], usable with [`Self::stop_voice`] to
    /// stop exactly this playback (unlike clip-index [`Self::stop`]).
    pub fn play(&self, clip_index: usize, volume: f32, looping: bool) -> VoiceId {
        let voice = self.alloc_voice();
        let _ = self.sender.send(AudioCommand::Play {
            clip_index,
            volume,
            looping,
            voice,
        });
        voice
    }

    /// Start playing a clip with 3D spatial audio parameters.
    ///
    /// Returns the instance's [`VoiceId`] (see [`Self::play`]).
    pub fn play_3d(&self, clip_index: usize, volume: f32, looping: bool, spatial: SpatialParams) -> VoiceId {
        let voice = self.alloc_voice();
        let _ = self.sender.send(AudioCommand::Play3d { clip_index, volume, looping, spatial, voice });
        voice
    }

    /// Stop exactly one playback instance. Unknown / already-finished voices
    /// are a silent no-op (fire-and-forget, like [`Self::stop`]).
    pub fn stop_voice(&self, voice: VoiceId) {
        let _ = self.sender.send(AudioCommand::StopVoice { voice });
    }

    /// Update the listener position and orientation for 3D audio.
    pub fn set_listener(&self, position: voltex_math::Vec3, forward: voltex_math::Vec3, right: voltex_math::Vec3) {
        let _ = self.sender.send(AudioCommand::SetListener { position, forward, right });
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

    /// Set volume for a mix group immediately.
    pub fn set_group_volume(&self, group: MixGroup, volume: f32) {
        let _ = self.sender.send(AudioCommand::SetGroupVolume { group, volume });
    }

    /// Fade a mix group to a target volume over `duration` seconds.
    pub fn fade_group(&self, group: MixGroup, target: f32, duration: f32) {
        let _ = self.sender.send(AudioCommand::FadeGroup { group, target, duration });
    }
}

impl Drop for AudioSystem {
    fn drop(&mut self) {
        let _ = self.sender.send(AudioCommand::Shutdown);
    }
}

// ---------------------------------------------------------------------------
// Command application (shared by the audio thread; unit-testable)
// ---------------------------------------------------------------------------

/// Apply one [`AudioCommand`] to the audio thread's state. Returns `true` for
/// `Shutdown` (the caller should exit its loop). Extracted from the thread
/// loop so the command semantics are unit-testable without a device.
pub(crate) fn apply_command(
    cmd: AudioCommand,
    playing: &mut Vec<PlayingSound>,
    clips: &mut Vec<AudioClip>,
    listener: &mut Listener,
    mixer: &mut MixerState,
) -> bool {
    match cmd {
        AudioCommand::Play { clip_index, volume, looping, voice } => {
            playing.push(PlayingSound::new(clip_index, volume, looping).with_voice(voice));
        }
        AudioCommand::Play3d { clip_index, volume, looping, spatial, voice } => {
            playing.push(PlayingSound::new_3d(clip_index, volume, looping, spatial).with_voice(voice));
        }
        AudioCommand::SetListener { position, forward, right } => {
            *listener = Listener { position, forward, right };
        }
        AudioCommand::Stop { clip_index } => {
            playing.retain(|s| s.clip_index != clip_index);
        }
        AudioCommand::StopVoice { voice } => {
            playing.retain(|s| s.voice != voice);
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
        AudioCommand::SetGroupVolume { group, volume } => {
            mixer.set_volume(group, volume);
        }
        AudioCommand::FadeGroup { group, target, duration } => {
            mixer.fade(group, target, duration);
        }
        AudioCommand::AddClip { clip } => {
            clips.push(clip);
        }
        AudioCommand::ReplaceClip { clip_index, clip } => {
            replace_clip_in(clips, clip_index, clip);
        }
        AudioCommand::Shutdown => {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Audio thread implementation
// ---------------------------------------------------------------------------

/// Main audio thread function.
///
/// On Windows, initializes `wasapi_ffi::WasapiDevice` and drives playback.
/// Elsewhere it opens the OS device through [`crate::backend::open_device_backend`]
/// (ALSA / CoreAudio). Without a device it runs in a silent "null" mode that
/// only processes commands.
fn audio_thread(rx: mpsc::Receiver<AudioCommand>, clips: Vec<AudioClip>) {
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

    #[test]
    fn add_clip_hands_out_sequential_indices_after_initial_clips() {
        let mut sys = AudioSystem::new(vec![silence_clip(), silence_clip()])
            .expect("AudioSystem::new failed");

        assert_eq!(sys.add_clip(silence_clip()), 2);
        assert_eq!(sys.add_clip(silence_clip()), 3);
        // The new index is immediately playable (FIFO: AddClip lands first).
        sys.play(3, 1.0, false);
    }

    #[test]
    fn add_clip_on_empty_system_starts_at_zero() {
        let mut sys = AudioSystem::new(vec![]).expect("AudioSystem::new failed");
        assert_eq!(sys.add_clip(silence_clip()), 0);
    }

    #[test]
    fn replace_clip_sends_without_panic_even_out_of_range() {
        let sys = AudioSystem::new(vec![silence_clip()]).expect("AudioSystem::new failed");
        sys.replace_clip(0, silence_clip()); // in range
        sys.replace_clip(99, silence_clip()); // out of range → thread-side no-op
    }

    #[test]
    fn play_returns_unique_voice_ids() {
        let sys = AudioSystem::new(vec![silence_clip()]).expect("AudioSystem::new failed");
        let a = sys.play(0, 1.0, true);
        let b = sys.play(0, 1.0, true);
        let c = sys.play_3d(0, 1.0, true, SpatialParams::new(voltex_math::Vec3::ZERO, 1.0, 50.0));
        assert_ne!(a, b, "each play must hand out a fresh voice id");
        assert_ne!(b, c, "play_3d shares the same voice id space");
        sys.stop_voice(a); // fire-and-forget, must not panic
    }

    #[test]
    fn apply_command_stop_voice_removes_only_that_voice() {
        let mut playing: Vec<PlayingSound> = Vec::new();
        let mut clips = vec![silence_clip()];
        let mut listener = Listener::default();
        let mut mixer = MixerState::new();
        let v1 = VoiceId(1);
        let v2 = VoiceId(2);
        // 블록 스코프로 클로저의 가변 차용을 해제 (clippy drop_non_drop 회피).
        {
            let mut apply =
                |cmd| apply_command(cmd, &mut playing, &mut clips, &mut listener, &mut mixer);
            assert!(!apply(AudioCommand::Play { clip_index: 0, volume: 1.0, looping: true, voice: v1 }));
            assert!(!apply(AudioCommand::Play { clip_index: 0, volume: 1.0, looping: true, voice: v2 }));
        }
        assert_eq!(playing.len(), 2);

        // Stopping one voice leaves the other instance of the SAME clip alive —
        // this is what clip-index Stop cannot do.
        let shutdown = apply_command(
            AudioCommand::StopVoice { voice: v1 },
            &mut playing,
            &mut clips,
            &mut listener,
            &mut mixer,
        );
        assert!(!shutdown);
        assert_eq!(playing.len(), 1);
        assert_eq!(playing[0].voice, v2);

        // Stopping an unknown / already-finished voice is a no-op.
        apply_command(
            AudioCommand::StopVoice { voice: VoiceId(999) },
            &mut playing,
            &mut clips,
            &mut listener,
            &mut mixer,
        );
        assert_eq!(playing.len(), 1);
    }

    #[test]
    fn replace_clip_in_swaps_in_range_and_ignores_out_of_range() {
        let mut clips = vec![silence_clip()];
        let replacement = AudioClip::new(vec![0.5f32; 100], 22050, 1);

        assert!(replace_clip_in(&mut clips, 0, replacement));
        assert_eq!(clips[0].sample_rate, 22050);
        assert_eq!(clips[0].samples.len(), 100);

        let other = AudioClip::new(vec![0.1f32; 10], 48000, 1);
        assert!(!replace_clip_in(&mut clips, 5, other));
        // Original slot untouched by the out-of-range attempt.
        assert_eq!(clips[0].sample_rate, 22050);
    }
}
