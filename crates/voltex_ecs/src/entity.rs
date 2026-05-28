//! Entity handle + slot allocator with generation tracking.

/// Generation-tracked entity handle. Composed of a stable id (slot index) and
/// a generation counter that increments on each despawn, so handles to slots
/// that have been reused refer to a different generation and `World::is_alive`
/// returns false. Cheap to copy; small enough to pass by value everywhere.
///
/// The fields are intentionally public: `Entity` is a plain-data handle that
/// round-trips through network snapshots ([`voltex_net`]), the Lua FFI, and
/// scene (de)serialization, all of which must reconstruct an `(id, generation)`
/// pair from the wire/disk/script. Safe handles come from
/// [`EntityAllocator::allocate`]; code reconstructing a handle from external
/// data should use [`Entity::from_raw_parts`] and then validate it with
/// [`World::is_alive`](crate::World::is_alive) before dereferencing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    /// Slot index. Stable across the lifetime of the slot, but reused after despawn.
    pub id: u32,
    /// Generation counter bumped on each despawn. Used to detect stale handles.
    pub generation: u32,
}

impl Entity {
    /// Reconstruct an `Entity` from a raw `(id, generation)` pair — e.g. a
    /// handle read back from a network snapshot, the Lua FFI, or a saved scene.
    ///
    /// The result is **not** guaranteed to reference a live entity; validate it
    /// with [`World::is_alive`](crate::World::is_alive) before use. Mirrors
    /// `voltex_asset::Handle::from_raw_parts`.
    pub fn from_raw_parts(id: u32, generation: u32) -> Self {
        Entity { id, generation }
    }
}

struct EntityEntry {
    generation: u32,
    alive: bool,
}

/// Generation-bumping allocator used by [`World`](crate::World) to hand out
/// safe [`Entity`] ids.
pub struct EntityAllocator {
    entries: Vec<EntityEntry>,
    free_list: Vec<u32>,
    alive_count: usize,
}

impl EntityAllocator {
    /// Creates an empty allocator.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            free_list: Vec::new(),
            alive_count: 0,
        }
    }

    /// Returns a fresh [`Entity`]; reuses a freed slot if any.
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

    /// Frees `entity`'s slot, bumping its generation. Returns `false` if the
    /// handle is stale or out of range.
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

    /// `true` if `entity`'s slot is still live at the same generation.
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entries
            .get(entity.id as usize)
            .is_some_and(|e| e.alive && e.generation == entity.generation)
    }

    /// Number of currently-live entities.
    pub fn alive_count(&self) -> usize {
        self.alive_count
    }

    /// Iterate every currently alive entity, in id order.
    pub fn live_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.alive)
            .map(|(id, entry)| Entity {
                id: id as u32,
                generation: entry.generation,
            })
    }

    /// Re-activate a previously deallocated slot at the same generation it
    /// had before deallocation. Returns false if the slot is alive, out of
    /// range, or has moved past `target_generation` (e.g. the slot was
    /// reallocated to a different entity that has since been deallocated
    /// again — the original handle is no longer recoverable).
    ///
    /// Used by `EditorCommand::Despawn` undo to revive the exact same
    /// `Entity { id, generation }` handle the user had before despawn (E3).
    pub fn restore_at_generation(&mut self, id: u32, target_generation: u32) -> bool {
        let Some(entry) = self.entries.get_mut(id as usize) else { return false; };
        if entry.alive {
            return false;
        }
        // deallocate() incremented generation by 1, so the slot's current
        // generation must be exactly target+1 for the original handle to be
        // recoverable. Anything else means another alloc/dealloc cycle ran
        // in between.
        if entry.generation != target_generation.wrapping_add(1) {
            return false;
        }
        entry.generation = target_generation;
        entry.alive = true;
        if let Some(pos) = self.free_list.iter().position(|&x| x == id) {
            self.free_list.remove(pos);
        }
        self.alive_count += 1;
        true
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
    fn from_raw_parts_round_trips_id_and_generation() {
        let e = Entity::from_raw_parts(42, 7);
        assert_eq!(e.id, 42);
        assert_eq!(e.generation, 7);
        // A reconstructed handle is not implicitly alive in any world.
        let alloc = EntityAllocator::new();
        assert!(!alloc.is_alive(e));
    }

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
    fn restore_at_generation_revives_dead_slot() {
        let mut alloc = EntityAllocator::new();
        let e = alloc.allocate();
        assert_eq!(e.generation, 0);
        alloc.deallocate(e);
        assert!(!alloc.is_alive(e));

        let restored = alloc.restore_at_generation(e.id, e.generation);
        assert!(restored, "restore_at_generation should succeed on dead slot");
        assert!(alloc.is_alive(e), "entity must be alive again at original generation");
        assert_eq!(alloc.alive_count(), 1);
    }

    #[test]
    fn restore_at_generation_rejects_alive_slot() {
        let mut alloc = EntityAllocator::new();
        let e = alloc.allocate();
        // Slot is currently alive — restore must refuse.
        let restored = alloc.restore_at_generation(e.id, e.generation);
        assert!(!restored);
    }

    #[test]
    fn restore_at_generation_rejects_advanced_generation() {
        let mut alloc = EntityAllocator::new();
        let e0 = alloc.allocate(); // (0, 0)
        alloc.deallocate(e0);      // entries[0] = (gen 1, dead)
        let e1 = alloc.allocate(); // (0, 1) — reuses slot
        alloc.deallocate(e1);      // entries[0] = (gen 2, dead)
        // Try to restore the *original* (0, 0) — slot has moved past it.
        let restored = alloc.restore_at_generation(e0.id, e0.generation);
        assert!(!restored, "restore must fail when slot has been alloc/dealloc'd in between");
    }

    #[test]
    fn restore_at_generation_rejects_out_of_range() {
        let mut alloc = EntityAllocator::new();
        let _ = alloc.allocate();
        assert!(!alloc.restore_at_generation(999, 0));
    }

    #[test]
    fn restore_then_deallocate_then_restore_again() {
        // Restore should not be a one-shot — repeated cycles work.
        let mut alloc = EntityAllocator::new();
        let e = alloc.allocate();
        alloc.deallocate(e);
        assert!(alloc.restore_at_generation(e.id, e.generation));
        alloc.deallocate(e);
        assert!(alloc.restore_at_generation(e.id, e.generation));
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
