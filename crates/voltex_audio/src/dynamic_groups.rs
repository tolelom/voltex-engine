use std::collections::HashMap;

/// Dynamic audio mix group system.
pub struct MixGroupManager {
    groups: HashMap<String, MixGroupConfig>,
}

#[derive(Debug, Clone)]
pub struct MixGroupConfig {
    pub name: String,
    pub volume: f32,
    pub muted: bool,
    pub parent: Option<String>,
}

impl Default for MixGroupManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MixGroupManager {
    pub fn new() -> Self {
        let mut mgr = MixGroupManager { groups: HashMap::new() };
        mgr.add_group("Master", None);
        mgr
    }

    pub fn add_group(&mut self, name: &str, parent: Option<&str>) -> bool {
        if self.groups.contains_key(name) { return false; }
        if let Some(p) = parent {
            if !self.groups.contains_key(p) { return false; }
        }
        self.groups.insert(name.to_string(), MixGroupConfig {
            name: name.to_string(),
            volume: 1.0,
            muted: false,
            parent: parent.map(|s| s.to_string()),
        });
        true
    }

    pub fn remove_group(&mut self, name: &str) -> bool {
        if name == "Master" { return false; } // can't remove master
        self.groups.remove(name).is_some()
    }

    pub fn set_volume(&mut self, name: &str, volume: f32) {
        if let Some(g) = self.groups.get_mut(name) { g.volume = volume.clamp(0.0, 1.0); }
    }

    pub fn effective_volume(&self, name: &str) -> f32 {
        let mut vol = 1.0;
        let mut current = name;
        for _ in 0..10 { // max depth to prevent infinite loops
            if let Some(g) = self.groups.get(current) {
                if g.muted { return 0.0; }
                vol *= g.volume;
                if let Some(ref p) = g.parent { current = p; } else { break; }
            } else { break; }
        }
        vol
    }

    pub fn group_count(&self) -> usize { self.groups.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_has_master() {
        let mgr = MixGroupManager::new();
        assert_eq!(mgr.group_count(), 1);
        assert!((mgr.effective_volume("Master") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_add_group() {
        let mut mgr = MixGroupManager::new();
        assert!(mgr.add_group("SFX", Some("Master")));
        assert_eq!(mgr.group_count(), 2);
    }

    #[test]
    fn test_effective_volume_chain() {
        let mut mgr = MixGroupManager::new();
        mgr.set_volume("Master", 0.8);
        mgr.add_group("SFX", Some("Master"));
        mgr.set_volume("SFX", 0.5);
        assert!((mgr.effective_volume("SFX") - 0.4).abs() < 1e-6); // 0.5 * 0.8
    }

    #[test]
    fn test_cant_remove_master() {
        let mut mgr = MixGroupManager::new();
        assert!(!mgr.remove_group("Master"));
    }

    #[test]
    fn test_add_duplicate_fails() {
        let mut mgr = MixGroupManager::new();
        mgr.add_group("SFX", Some("Master"));
        assert!(!mgr.add_group("SFX", Some("Master")));
    }
}
