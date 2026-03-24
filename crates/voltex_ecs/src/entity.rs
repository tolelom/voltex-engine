#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    pub id: u32,
    pub generation: u32,
}

struct EntityEntry {
    generation: u32,
    alive: bool,
}

pub struct EntityAllocator {
    entries: Vec<EntityEntry>,
    free_list: Vec<u32>,
    alive_count: usize,
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            free_list: Vec::new(),
            alive_count: 0,
        }
    }

    pub fn allocate(&mut self) -> Entity {
        self.alive_count += 1;
        if let Some(id) = self.free_list.pop() {
            let entry = &mut self.entries[id as usize];
            // generation was already incremented on deallocate
            entry.alive = true;
            Entity {
                id,
                generation: entry.generation,
            }
        } else {
            let id = self.entries.len() as u32;
            self.entries.push(EntityEntry {
                generation: 0,
                alive: true,
            });
            Entity { id, generation: 0 }
        }
    }

    pub fn deallocate(&mut self, entity: Entity) -> bool {
        let Some(entry) = self.entries.get_mut(entity.id as usize) else {
            return false;
        };
        if !entry.alive || entry.generation != entity.generation {
            return false;
        }
        entry.alive = false;
        entry.generation = entry.generation.wrapping_add(1);
        self.free_list.push(entity.id);
        self.alive_count -= 1;
        true
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entries
            .get(entity.id as usize)
            .map_or(false, |e| e.alive && e.generation == entity.generation)
    }

    pub fn alive_count(&self) -> usize {
        self.alive_count
    }
}

impl Default for EntityAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate() {
        let mut alloc = EntityAllocator::new();
        let e0 = alloc.allocate();
        let e1 = alloc.allocate();
        assert_eq!(e0.id, 0);
        assert_eq!(e1.id, 1);
        assert_eq!(e0.generation, 0);
        assert_eq!(e1.generation, 0);
    }

    #[test]
    fn test_deallocate_and_reuse() {
        let mut alloc = EntityAllocator::new();
        let e0 = alloc.allocate();
        let _e1 = alloc.allocate();
        assert!(alloc.deallocate(e0));
        let e0_new = alloc.allocate();
        assert_eq!(e0_new.id, 0);
        assert_eq!(e0_new.generation, 1);
    }

    #[test]
    fn test_is_alive() {
        let mut alloc = EntityAllocator::new();
        let e = alloc.allocate();
        assert!(alloc.is_alive(e));
        alloc.deallocate(e);
        assert!(!alloc.is_alive(e));
    }

    #[test]
    fn test_stale_entity_rejected() {
        let mut alloc = EntityAllocator::new();
        let e = alloc.allocate();
        alloc.deallocate(e);
        // stale entity not alive
        assert!(!alloc.is_alive(e));
        // double-delete fails
        assert!(!alloc.deallocate(e));
    }

    #[test]
    fn test_alive_count() {
        let mut alloc = EntityAllocator::new();
        assert_eq!(alloc.alive_count(), 0);
        let e0 = alloc.allocate();
        let e1 = alloc.allocate();
        assert_eq!(alloc.alive_count(), 2);
        alloc.deallocate(e0);
        assert_eq!(alloc.alive_count(), 1);
        alloc.deallocate(e1);
        assert_eq!(alloc.alive_count(), 0);
    }
}
