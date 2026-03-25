use crate::AudioClip;
use crate::spatial::{Listener, SpatialParams, compute_spatial_gains};

/// Represents a sound currently being played back.
pub struct PlayingSound {
    /// Index into the clips slice passed to `mix_sounds`.
    pub clip_index: usize,
    /// Current playback position in frames (not samples).
    pub position: usize,
    /// Linear volume multiplier [0.0 = silent, 1.0 = full].
    pub volume: f32,
    /// Whether the sound loops when it reaches the end.
    pub looping: bool,
    /// Optional 3D spatial parameters. None = 2D (no spatialization).
    pub spatial: Option<SpatialParams>,
}

impl PlayingSound {
    pub fn new(clip_index: usize, volume: f32, looping: bool) -> Self {
        Self {
            clip_index,
            position: 0,
            volume,
            looping,
            spatial: None,
        }
    }

    pub fn new_3d(clip_index: usize, volume: f32, looping: bool, spatial: SpatialParams) -> Self {
        Self { clip_index, position: 0, volume, looping, spatial: Some(spatial) }
    }
}

/// Mix all active sounds in `playing` into `output`.
///
/// # Parameters
/// - `output`: interleaved output buffer (len = frames * device_channels).
/// - `playing`: mutable list of active sounds; finished non-looping sounds are removed.
/// - `clips`: the audio clip assets.
/// - `device_sample_rate`: output device sample rate.
/// - `device_channels`: output device channel count (1 or 2).
/// - `frames`: number of output frames to fill.
/// - `listener`: the listener for 3D spatialization.
pub fn mix_sounds(
    output: &mut Vec<f32>,
    playing: &mut Vec<PlayingSound>,
    clips: &[AudioClip],
    device_sample_rate: u32,
    device_channels: u16,
    frames: usize,
    listener: &Listener,
) {
    // Ensure output buffer is sized correctly and zeroed.
    let output_len = frames * device_channels as usize;
    output.clear();
    output.resize(output_len, 0.0);

    let mut finished = Vec::new();

    for (sound_idx, sound) in playing.iter_mut().enumerate() {
        let clip = &clips[sound.clip_index];

        // Compute per-channel effective volumes based on spatial params.
        let (vol_left, vol_right) = if let Some(ref spatial) = sound.spatial {
            let (atten, left_gain, right_gain) = compute_spatial_gains(listener, spatial);
            (sound.volume * atten * left_gain, sound.volume * atten * right_gain)
        } else {
            (sound.volume, sound.volume)
        };

        // Compute integer rate ratio for naive resampling.
        // For same-rate clips ratio == 1, so we read every frame.
        let rate_ratio = if device_sample_rate > 0 {
            clip.sample_rate as f64 / device_sample_rate as f64
        } else {
            1.0
        };

        for frame_out in 0..frames {
            // Map the output frame index to a clip frame index.
            let clip_frame = sound.position + (frame_out as f64 * rate_ratio) as usize;

            if clip_frame >= clip.frame_count() {
                // Sound exhausted within this render block.
                if sound.looping {
                    // We'll reset after the loop; for now just stop filling.
                }
                break;
            }

            // Fetch sample(s) from clip frame.
            let out_base = frame_out * device_channels as usize;

            if device_channels == 2 {
                let (left, right) = if clip.channels == 1 {
                    // Mono -> stereo: apply per-channel volumes
                    let raw = clip.samples[clip_frame];
                    (raw * vol_left, raw * vol_right)
                } else {
                    // Stereo clip
                    let l = clip.samples[clip_frame * 2] * vol_left;
                    let r = clip.samples[clip_frame * 2 + 1] * vol_right;
                    (l, r)
                };
                output[out_base] += left;
                output[out_base + 1] += right;
            } else {
                // Mono output: average left and right volumes
                let vol_mono = (vol_left + vol_right) * 0.5;
                let s = if clip.channels == 1 {
                    clip.samples[clip_frame] * vol_mono
                } else {
                    // Mix stereo clip to mono
                    (clip.samples[clip_frame * 2] + clip.samples[clip_frame * 2 + 1])
                        * 0.5
                        * vol_mono
                };
                output[out_base] += s;
            }
        }

        // Advance position by the number of clip frames consumed this block.
        let frames_consumed = (frames as f64 * rate_ratio) as usize;
        sound.position += frames_consumed;

        if sound.position >= clip.frame_count() {
            if sound.looping {
                sound.position = 0;
            } else {
                finished.push(sound_idx);
            }
        }
    }

    // Remove finished sounds in reverse order to preserve indices.
    for &idx in finished.iter().rev() {
        playing.remove(idx);
    }

    // Clamp output to [-1.0, 1.0].
    for sample in output.iter_mut() {
        *sample = sample.clamp(-1.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AudioClip;
    use crate::spatial::{Listener, SpatialParams};
    use voltex_math::Vec3;

    fn make_mono_clip(value: f32, frames: usize, sample_rate: u32) -> AudioClip {
        AudioClip::new(vec![value; frames], sample_rate, 1)
    }

    fn make_stereo_clip(left: f32, right: f32, frames: usize, sample_rate: u32) -> AudioClip {
        let mut samples = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            samples.push(left);
            samples.push(right);
        }
        AudioClip::new(samples, sample_rate, 2)
    }

    #[test]
    fn single_sound_volume() {
        let clips = vec![make_mono_clip(1.0, 100, 44100)];
        let mut playing = vec![PlayingSound::new(0, 0.5, false)];
        let mut output = Vec::new();

        mix_sounds(&mut output, &mut playing, &clips, 44100, 1, 10, &Listener::default());

        // All output frames should be 0.5 (1.0 * 0.5 volume)
        assert_eq!(output.len(), 10);
        for &s in &output {
            assert!((s - 0.5).abs() < 1e-6, "expected 0.5, got {}", s);
        }
    }

    #[test]
    fn two_sounds_sum() {
        // Two clips each at 0.3; sum is 0.6 (below clamp threshold)
        let clips = vec![
            make_mono_clip(0.3, 100, 44100),
            make_mono_clip(0.3, 100, 44100),
        ];
        let mut playing = vec![
            PlayingSound::new(0, 1.0, false),
            PlayingSound::new(1, 1.0, false),
        ];
        let mut output = Vec::new();

        mix_sounds(&mut output, &mut playing, &clips, 44100, 1, 10, &Listener::default());

        assert_eq!(output.len(), 10);
        for &s in &output {
            assert!((s - 0.6).abs() < 1e-5, "expected 0.6, got {}", s);
        }
    }

    #[test]
    fn clipping() {
        // Two clips at 1.0 each; sum would be 2.0 but must be clamped to 1.0
        let clips = vec![
            make_mono_clip(1.0, 100, 44100),
            make_mono_clip(1.0, 100, 44100),
        ];
        let mut playing = vec![
            PlayingSound::new(0, 1.0, false),
            PlayingSound::new(1, 1.0, false),
        ];
        let mut output = Vec::new();

        mix_sounds(&mut output, &mut playing, &clips, 44100, 1, 10, &Listener::default());

        for &s in &output {
            assert!(s <= 1.0, "output {} exceeds 1.0 (clamp failed)", s);
            assert!(s >= -1.0, "output {} below -1.0 (clamp failed)", s);
        }
    }

    #[test]
    fn non_looping_removal() {
        // Clip is only 5 frames; request 20 frames; sound should be removed after
        let clips = vec![make_mono_clip(0.5, 5, 44100)];
        let mut playing = vec![PlayingSound::new(0, 1.0, false)];
        let mut output = Vec::new();

        mix_sounds(&mut output, &mut playing, &clips, 44100, 1, 20, &Listener::default());

        // Sound should have been removed
        assert!(playing.is_empty(), "non-looping sound was not removed");
    }

    #[test]
    fn looping_continues() {
        // Clip is 5 frames; request 20 frames; looping sound should remain
        let clips = vec![make_mono_clip(0.5, 5, 44100)];
        let mut playing = vec![PlayingSound::new(0, 1.0, true)];
        let mut output = Vec::new();

        mix_sounds(&mut output, &mut playing, &clips, 44100, 1, 20, &Listener::default());

        // Sound should still be in the list
        assert_eq!(playing.len(), 1, "looping sound was incorrectly removed");
        // Position should have been reset to 0
        assert_eq!(playing[0].position, 0);
    }

    #[test]
    fn mono_to_stereo() {
        // Mono clip mixed to stereo output: both channels should have same value
        let clips = vec![make_mono_clip(0.7, 100, 44100)];
        let mut playing = vec![PlayingSound::new(0, 1.0, false)];
        let mut output = Vec::new();

        mix_sounds(&mut output, &mut playing, &clips, 44100, 2, 10, &Listener::default());

        // output length = 10 frames * 2 channels = 20
        assert_eq!(output.len(), 20);
        for i in 0..10 {
            let l = output[i * 2];
            let r = output[i * 2 + 1];
            assert!((l - 0.7).abs() < 1e-6, "left={}", l);
            assert!((r - 0.7).abs() < 1e-6, "right={}", r);
        }
    }

    // -----------------------------------------------------------------------
    // Spatial audio tests
    // -----------------------------------------------------------------------

    #[test]
    fn spatial_2d_unchanged() {
        // spatial=None should produce the same output as before (no spatialization)
        let clips = vec![make_mono_clip(1.0, 100, 44100)];
        let mut playing = vec![PlayingSound::new(0, 0.5, false)];
        let mut output = Vec::new();

        mix_sounds(&mut output, &mut playing, &clips, 44100, 1, 10, &Listener::default());

        assert_eq!(output.len(), 10);
        for &s in &output {
            assert!((s - 0.5).abs() < 1e-6, "expected 0.5, got {}", s);
        }
    }

    #[test]
    fn spatial_far_away_silent() {
        // Emitter beyond max_distance → attenuation = 0.0 → ~0 output
        let clips = vec![make_mono_clip(1.0, 100, 44100)];
        let spatial = SpatialParams::new(Vec3::new(200.0, 0.0, 0.0), 1.0, 100.0);
        let mut playing = vec![PlayingSound::new_3d(0, 1.0, false, spatial)];
        let mut output = Vec::new();

        mix_sounds(&mut output, &mut playing, &clips, 44100, 2, 10, &Listener::default());

        assert_eq!(output.len(), 20);
        for &s in &output {
            assert!(s.abs() < 1e-6, "expected ~0 for far-away emitter, got {}", s);
        }
    }

    #[test]
    fn spatial_right_panning() {
        // Emitter on +X → right channel should be louder than left channel (stereo output)
        let clips = vec![make_mono_clip(1.0, 100, 44100)];
        // Emitter close enough to be audible (within min_distance → atten=1)
        let spatial = SpatialParams::new(Vec3::new(0.5, 0.0, 0.0), 1.0, 100.0);
        let mut playing = vec![PlayingSound::new_3d(0, 1.0, false, spatial)];
        let mut output = Vec::new();

        mix_sounds(&mut output, &mut playing, &clips, 44100, 2, 10, &Listener::default());

        assert_eq!(output.len(), 20);
        // Check first frame: left=output[0], right=output[1]
        let left = output[0];
        let right = output[1];
        assert!(right > left, "expected right > left for +X emitter, got left={}, right={}", left, right);
    }
}
