/// ECS component that attaches audio playback to an entity.
#[derive(Debug, Clone)]
pub struct AudioSource {
    pub clip_id: u32,
    pub volume: f32,
    pub spatial: bool,
    pub looping: bool,
    pub playing: bool,
    pub played_once: bool,
}

impl AudioSource {
    pub fn new(clip_id: u32) -> Self {
        AudioSource {
            clip_id,
            volume: 1.0,
            spatial: false,
            looping: false,
            playing: false,
            played_once: false,
        }
    }

    pub fn spatial(mut self) -> Self { self.spatial = true; self }
    pub fn looping(mut self) -> Self { self.looping = true; self }

    pub fn play(&mut self) {
        self.playing = true;
        self.played_once = false;
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let src = AudioSource::new(42);
        assert_eq!(src.clip_id, 42);
        assert!((src.volume - 1.0).abs() < 1e-6);
        assert!(!src.spatial);
        assert!(!src.looping);
        assert!(!src.playing);
        assert!(!src.played_once);
    }

    #[test]
    fn test_builder_pattern() {
        let src = AudioSource::new(1).spatial().looping();
        assert!(src.spatial);
        assert!(src.looping);
    }

    #[test]
    fn test_play_stop() {
        let mut src = AudioSource::new(1);
        src.play();
        assert!(src.playing);
        assert!(!src.played_once);
        src.played_once = true;
        src.play(); // re-play should reset played_once
        assert!(!src.played_once);
        src.stop();
        assert!(!src.playing);
    }

    #[test]
    fn test_volume() {
        let mut src = AudioSource::new(1);
        src.volume = 0.5;
        assert!((src.volume - 0.5).abs() < 1e-6);
    }
}
