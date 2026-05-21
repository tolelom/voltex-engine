//! In-memory representation of decoded audio samples.

/// A loaded audio clip stored as normalized f32 PCM samples.
#[derive(Debug)]
pub struct AudioClip {
    /// Interleaved PCM samples in the range [-1.0, 1.0].
    pub samples: Vec<f32>,
    /// Number of samples per second (e.g. 44100).
    pub sample_rate: u32,
    /// Number of audio channels (1 = mono, 2 = stereo).
    pub channels: u16,
}

impl AudioClip {
    /// Create a new AudioClip from raw f32 samples.
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        Self {
            samples,
            sample_rate,
            channels,
        }
    }

    /// Total number of multi-channel frames.
    /// A frame contains one sample per channel.
    pub fn frame_count(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    /// Duration in seconds.
    pub fn duration(&self) -> f32 {
        self.frame_count() as f32 / self.sample_rate as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_clip() {
        // 44100 Hz, 1 channel, 1 second of silence
        let samples = vec![0.0f32; 44100];
        let clip = AudioClip::new(samples, 44100, 1);
        assert_eq!(clip.frame_count(), 44100);
        assert!((clip.duration() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn stereo_clip() {
        // 44100 Hz, 2 channels, 0.5 seconds
        let samples = vec![0.0f32; 44100]; // 44100 interleaved samples = 22050 frames
        let clip = AudioClip::new(samples, 44100, 2);
        assert_eq!(clip.frame_count(), 22050);
        assert!((clip.duration() - 0.5).abs() < 1e-6);
    }
}
