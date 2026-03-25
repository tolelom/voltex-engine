/// Mixer group identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MixGroup {
    Master = 0,
    Bgm = 1,
    Sfx = 2,
    Voice = 3,
}

/// Per-group volume state with optional fade.
#[derive(Debug, Clone)]
pub struct GroupState {
    pub volume: f32,
    pub fade_target: f32,
    pub fade_speed: f32,
}

impl GroupState {
    pub fn new() -> Self {
        Self {
            volume: 1.0,
            fade_target: 1.0,
            fade_speed: 0.0,
        }
    }

    /// Advance fade by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if self.fade_speed == 0.0 {
            return;
        }
        let delta = self.fade_speed * dt;
        if self.volume < self.fade_target {
            self.volume = (self.volume + delta).min(self.fade_target);
        } else {
            self.volume = (self.volume - delta).max(self.fade_target);
        }
        if (self.volume - self.fade_target).abs() < f32::EPSILON {
            self.volume = self.fade_target;
            self.fade_speed = 0.0;
        }
    }
}

impl Default for GroupState {
    fn default() -> Self {
        Self::new()
    }
}

/// Global mixer state holding one `GroupState` per `MixGroup`.
pub struct MixerState {
    pub groups: [GroupState; 4],
}

impl MixerState {
    pub fn new() -> Self {
        Self {
            groups: [
                GroupState::new(),
                GroupState::new(),
                GroupState::new(),
                GroupState::new(),
            ],
        }
    }

    fn idx(group: MixGroup) -> usize {
        group as usize
    }

    /// Immediately set volume for `group`, cancelling any active fade.
    pub fn set_volume(&mut self, group: MixGroup, volume: f32) {
        let v = volume.clamp(0.0, 1.0);
        let g = &mut self.groups[Self::idx(group)];
        g.volume = v;
        g.fade_target = v;
        g.fade_speed = 0.0;
    }

    /// Begin a fade from the current volume to `target` over `duration` seconds.
    /// If `duration` <= 0, set volume instantly.
    pub fn fade(&mut self, group: MixGroup, target: f32, duration: f32) {
        let target = target.clamp(0.0, 1.0);
        let g = &mut self.groups[Self::idx(group)];
        if duration <= 0.0 {
            g.volume = target;
            g.fade_target = target;
            g.fade_speed = 0.0;
        } else {
            let speed = (target - g.volume).abs() / duration;
            g.fade_target = target;
            g.fade_speed = speed;
        }
    }

    /// Advance all groups by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        for g in &mut self.groups {
            g.tick(dt);
        }
    }

    /// Current volume of `group`.
    pub fn volume(&self, group: MixGroup) -> f32 {
        self.groups[Self::idx(group)].volume
    }

    /// Effective volume: group volume multiplied by master volume.
    /// For `Master`, returns its own volume only.
    pub fn effective_volume(&self, group: MixGroup) -> f32 {
        let master = self.groups[Self::idx(MixGroup::Master)].volume;
        if group == MixGroup::Master {
            master
        } else {
            self.groups[Self::idx(group)].volume * master
        }
    }
}

impl Default for MixerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_state_tick_fade() {
        let mut g = GroupState::new();
        g.volume = 1.0;
        g.fade_target = 0.0;
        g.fade_speed = 2.0;

        g.tick(0.25); // 0.25 * 2.0 = 0.5 moved
        assert!((g.volume - 0.5).abs() < 1e-5, "expected ~0.5, got {}", g.volume);

        g.tick(0.25); // another 0.5 → reaches 0.0
        assert!((g.volume - 0.0).abs() < 1e-5, "expected 0.0, got {}", g.volume);
        assert_eq!(g.fade_speed, 0.0, "fade_speed should be 0 after reaching target");
    }

    #[test]
    fn group_state_tick_no_overshoot() {
        let mut g = GroupState::new();
        g.volume = 1.0;
        g.fade_target = 0.0;
        g.fade_speed = 100.0; // very fast

        g.tick(1.0);
        assert_eq!(g.volume, 0.0, "should not overshoot below 0");
        assert_eq!(g.fade_speed, 0.0);
    }

    #[test]
    fn mixer_set_volume() {
        let mut m = MixerState::new();
        // Start a fade, then override with set_volume
        m.fade(MixGroup::Sfx, 0.0, 5.0);
        m.set_volume(MixGroup::Sfx, 0.3);
        assert!((m.volume(MixGroup::Sfx) - 0.3).abs() < 1e-5);
        assert_eq!(m.groups[MixGroup::Sfx as usize].fade_speed, 0.0, "fade cancelled");
    }

    #[test]
    fn mixer_fade() {
        let mut m = MixerState::new();
        m.fade(MixGroup::Sfx, 0.0, 1.0); // 1→0 over 1s, speed=1.0

        m.tick(0.5);
        let v = m.volume(MixGroup::Sfx);
        assert!((v - 0.5).abs() < 1e-5, "at 0.5s expected ~0.5, got {}", v);

        m.tick(0.5);
        let v = m.volume(MixGroup::Sfx);
        assert!((v - 0.0).abs() < 1e-5, "at 1.0s expected 0.0, got {}", v);
    }

    #[test]
    fn effective_volume() {
        let mut m = MixerState::new();
        m.set_volume(MixGroup::Master, 0.5);
        m.set_volume(MixGroup::Sfx, 0.8);
        let ev = m.effective_volume(MixGroup::Sfx);
        assert!((ev - 0.4).abs() < 1e-5, "expected 0.4, got {}", ev);
    }

    #[test]
    fn master_zero_mutes_all() {
        let mut m = MixerState::new();
        m.set_volume(MixGroup::Master, 0.0);
        for group in [MixGroup::Bgm, MixGroup::Sfx, MixGroup::Voice] {
            assert_eq!(m.effective_volume(group), 0.0);
        }
    }

    #[test]
    fn fade_up() {
        let mut m = MixerState::new();
        m.set_volume(MixGroup::Bgm, 0.0);
        m.fade(MixGroup::Bgm, 1.0, 2.0); // 0→1 over 2s, speed=0.5

        m.tick(1.0);
        let v = m.volume(MixGroup::Bgm);
        assert!((v - 0.5).abs() < 1e-5, "at 1s expected ~0.5, got {}", v);

        m.tick(1.0);
        let v = m.volume(MixGroup::Bgm);
        assert!((v - 1.0).abs() < 1e-5, "at 2s expected 1.0, got {}", v);
    }
}
