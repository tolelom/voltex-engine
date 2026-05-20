/// Tracks which meshes need BLAS rebuild.
pub struct BlasTracker {
    dirty: Vec<(u32, bool)>, // (mesh_id, needs_rebuild)
}

impl Default for BlasTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BlasTracker {
    pub fn new() -> Self { BlasTracker { dirty: Vec::new() } }
    pub fn register(&mut self, mesh_id: u32) { self.dirty.push((mesh_id, false)); }
    pub fn mark_dirty(&mut self, mesh_id: u32) {
        if let Some(entry) = self.dirty.iter_mut().find(|(id, _)| *id == mesh_id) { entry.1 = true; }
    }
    pub fn dirty_meshes(&self) -> Vec<u32> { self.dirty.iter().filter(|(_, d)| *d).map(|(id, _)| *id).collect() }
    pub fn clear_dirty(&mut self) { for entry in &mut self.dirty { entry.1 = false; } }
    pub fn mesh_count(&self) -> usize { self.dirty.len() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_register_and_dirty() {
        let mut t = BlasTracker::new();
        t.register(1); t.register(2);
        t.mark_dirty(1);
        assert_eq!(t.dirty_meshes(), vec![1]);
    }
    #[test]
    fn test_clear_dirty() {
        let mut t = BlasTracker::new();
        t.register(1); t.mark_dirty(1);
        t.clear_dirty();
        assert!(t.dirty_meshes().is_empty());
    }
}
