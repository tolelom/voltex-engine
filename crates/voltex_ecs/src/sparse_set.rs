//! Sparse set storage for a single component type `T`. Provides O(1) insert,
//! remove, lookup, and a packed dense iterator. Tracks per-component change
//! ticks for filtering "changed this frame" queries.

use std::any::Any;
use crate::entity::Entity;

/// Sparse set of `(Entity, T)` pairs with O(1) operations + change tracking.
pub struct SparseSet<T> {
    sparse: Vec<Option<usize>>,
    dense_entities: Vec<Entity>,
    dense_data: Vec<T>,
    ticks: Vec<u64>,
    current_tick: u64,
}

impl<T> SparseSet<T> {
    /// Creates an empty sparse set.
    pub fn new() -> Self {
        Self {
            sparse: Vec::new(),
            dense_entities: Vec::new(),
            dense_data: Vec::new(),
            ticks: Vec::new(),
            current_tick: 1,
        }
    }

    /// Inserts (or overwrites) `value` for `entity`, marking it as changed.
    pub fn insert(&mut self, entity: Entity, value: T) {
        let id = entity.id as usize;
        // Grow sparse vec if needed
        if id >= self.sparse.len() {
            self.sparse.resize(id + 1, None);
        }

        if let Some(dense_idx) = self.sparse[id] {
            // Overwrite existing
            self.dense_data[dense_idx] = value;
            self.dense_entities[dense_idx] = entity;
            self.ticks[dense_idx] = self.current_tick;
        } else {
            let dense_idx = self.dense_data.len();
            self.sparse[id] = Some(dense_idx);
            self.dense_entities.push(entity);
            self.dense_data.push(value);
            self.ticks.push(self.current_tick);
        }
    }

    /// Removes `entity`'s component, returning the value if present.
    pub fn remove(&mut self, entity: Entity) -> Option<T> {
        let id = entity.id as usize;
        let dense_idx = *self.sparse.get(id)?.as_ref()?;

        // Check entity matches (generation safety)
        if self.dense_entities[dense_idx] != entity {
            return None;
        }

        let last_idx = self.dense_data.len() - 1;
        self.sparse[id] = None;

        if dense_idx == last_idx {
            self.dense_entities.pop();
            self.ticks.pop();
            Some(self.dense_data.pop().unwrap())
        } else {
            // Swap with last
            let swapped_entity = self.dense_entities[last_idx];
            self.sparse[swapped_entity.id as usize] = Some(dense_idx);
            self.dense_entities.swap_remove(dense_idx);
            self.ticks.swap_remove(dense_idx);
            Some(self.dense_data.swap_remove(dense_idx))
        }
    }

    /// Returns the component for `entity`, or `None`.
    pub fn get(&self, entity: Entity) -> Option<&T> {
        let id = entity.id as usize;
        let dense_idx = self.sparse.get(id)?.as_ref().copied()?;
        if self.dense_entities[dense_idx] != entity {
            return None;
        }
        Some(&self.dense_data[dense_idx])
    }

    /// Mutable equivalent of [`get`](Self::get); bumps `entity`'s change tick.
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let id = entity.id as usize;
        let dense_idx = self.sparse.get(id)?.as_ref().copied()?;
        if self.dense_entities[dense_idx] != entity {
            return None;
        }
        self.ticks[dense_idx] = self.current_tick;
        Some(&mut self.dense_data[dense_idx])
    }

    /// `true` if `entity` has a component in this set.
    pub fn contains(&self, entity: Entity) -> bool {
        let id = entity.id as usize;
        self.sparse
            .get(id)
            .and_then(|opt| opt.as_ref())
            .is_some_and(|&dense_idx| self.dense_entities[dense_idx] == entity)
    }

    /// Number of stored components.
    pub fn len(&self) -> usize {
        self.dense_data.len()
    }

    /// `true` if no components are stored.
    pub fn is_empty(&self) -> bool {
        self.dense_data.is_empty()
    }

    /// Packed iterator of `(entity, &component)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.dense_entities.iter().copied().zip(self.dense_data.iter())
    }

    /// Packed iterator of `(entity, &mut component)` pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
        self.dense_entities.iter().copied().zip(self.dense_data.iter_mut())
    }

    /// Borrows the dense entity slice (parallel to [`data`](Self::data)).
    pub fn entities(&self) -> &[Entity] {
        &self.dense_entities
    }

    /// Borrows the dense component slice.
    pub fn data(&self) -> &[T] {
        &self.dense_data
    }

    /// Mutable borrow of the dense component slice.
    pub fn data_mut(&mut self) -> &mut [T] {
        &mut self.dense_data
    }

    /// Check if an entity's component was changed this tick.
    pub fn is_changed(&self, entity: Entity) -> bool {
        if let Some(&index) = self.sparse.get(entity.id as usize).and_then(|o| o.as_ref()) {
            // Verify the slot actually holds this entity (generation safety,
            // matching get/contains/remove) before trusting its tick.
            self.dense_entities[index] == entity && self.ticks[index] == self.current_tick
        } else {
            false
        }
    }

    /// Advance the tick counter (call at end of frame).
    pub fn increment_tick(&mut self) {
        self.current_tick += 1;
    }

    /// Return entities changed this tick with their data.
    pub fn iter_changed(&self) -> impl Iterator<Item = (Entity, &T)> + '_ {
        self.dense_entities.iter()
            .zip(self.dense_data.iter())
            .zip(self.ticks.iter())
            .filter(move |((_, _), &tick)| tick == self.current_tick)
            .map(|((entity, data), _)| (*entity, data))
    }
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Type-erased view of a [`SparseSet<T>`] — lets [`World`](crate::World)
/// hold heterogeneous component stores in a single map.
pub trait ComponentStorage: Any {
    /// Downcast view as `&dyn Any`.
    fn as_any(&self) -> &dyn Any;
    /// Downcast view as `&mut dyn Any`.
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Removes `entity`'s component from this storage, if any.
    fn remove_entity(&mut self, entity: Entity);
    /// Number of components currently stored.
    fn storage_len(&self) -> usize;
    /// Advance the internal change-tick counter (call once per frame).
    fn increment_tick(&mut self);
}

impl<T: 'static> ComponentStorage for SparseSet<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn remove_entity(&mut self, entity: Entity) {
        self.remove(entity);
    }
    fn storage_len(&self) -> usize {
        self.dense_data.len()
    }
    fn increment_tick(&mut self) {
        self.current_tick += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    fn make_entity(id: u32, generation: u32) -> Entity {
        Entity { id, generation }
    }

    #[test]
    fn test_insert_and_get() {
        let mut set: SparseSet<i32> = SparseSet::new();
        let e = make_entity(0, 0);
        set.insert(e, 42);
        assert_eq!(set.get(e), Some(&42));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_overwrite() {
        let mut set: SparseSet<i32> = SparseSet::new();
        let e = make_entity(0, 0);
        set.insert(e, 1);
        set.insert(e, 99);
        assert_eq!(set.get(e), Some(&99));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut set: SparseSet<i32> = SparseSet::new();
        let e0 = make_entity(0, 0);
        let e1 = make_entity(1, 0);
        let e2 = make_entity(2, 0);
        set.insert(e0, 10);
        set.insert(e1, 20);
        set.insert(e2, 30);
        // Remove middle
        let removed = set.remove(e1);
        assert_eq!(removed, Some(20));
        assert_eq!(set.len(), 2);
        assert!(set.get(e1).is_none());
        // Remaining still accessible
        assert_eq!(set.get(e0), Some(&10));
        assert_eq!(set.get(e2), Some(&30));
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut set: SparseSet<i32> = SparseSet::new();
        let e = make_entity(5, 0);
        assert_eq!(set.remove(e), None);
    }

    #[test]
    fn test_iter() {
        let mut set: SparseSet<i32> = SparseSet::new();
        let e0 = make_entity(0, 0);
        let e1 = make_entity(1, 0);
        set.insert(e0, 100);
        set.insert(e1, 200);
        let mut values: Vec<i32> = set.iter().map(|(_, v)| *v).collect();
        values.sort();
        assert_eq!(values, vec![100, 200]);
    }

    #[test]
    fn test_iter_mut() {
        let mut set: SparseSet<i32> = SparseSet::new();
        let e0 = make_entity(0, 0);
        let e1 = make_entity(1, 0);
        set.insert(e0, 1);
        set.insert(e1, 2);
        for (_, v) in set.iter_mut() {
            *v *= 10;
        }
        assert_eq!(set.get(e0), Some(&10));
        assert_eq!(set.get(e1), Some(&20));
    }

    #[test]
    fn test_contains() {
        let mut set: SparseSet<i32> = SparseSet::new();
        let e = make_entity(3, 0);
        assert!(!set.contains(e));
        set.insert(e, 7);
        assert!(set.contains(e));
        set.remove(e);
        assert!(!set.contains(e));
    }

    #[test]
    fn test_insert_is_changed() {
        let mut set = SparseSet::<u32>::new();
        let e = make_entity(0, 0);
        set.insert(e, 42);
        assert!(set.is_changed(e));
    }

    #[test]
    fn test_get_mut_marks_changed() {
        let mut set = SparseSet::<u32>::new();
        let e = make_entity(0, 0);
        set.insert(e, 42);
        set.increment_tick();
        assert!(!set.is_changed(e));
        let _ = set.get_mut(e);
        assert!(set.is_changed(e));
    }

    #[test]
    fn test_get_not_changed() {
        let mut set = SparseSet::<u32>::new();
        let e = make_entity(0, 0);
        set.insert(e, 42);
        set.increment_tick();
        let _ = set.get(e);
        assert!(!set.is_changed(e));
    }

    #[test]
    fn test_clear_resets_changed() {
        let mut set = SparseSet::<u32>::new();
        let e = make_entity(0, 0);
        set.insert(e, 42);
        assert!(set.is_changed(e));
        set.increment_tick();
        assert!(!set.is_changed(e));
    }

    #[test]
    fn test_iter_changed() {
        let mut set = SparseSet::<u32>::new();
        let e1 = make_entity(0, 0);
        let e2 = make_entity(1, 0);
        let e3 = make_entity(2, 0);
        set.insert(e1, 10);
        set.insert(e2, 20);
        set.insert(e3, 30);
        set.increment_tick();
        let _ = set.get_mut(e2);
        let changed: Vec<_> = set.iter_changed().collect();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].0.id, 1);
    }

    #[test]
    fn test_remove_preserves_ticks() {
        let mut set = SparseSet::<u32>::new();
        let e1 = make_entity(0, 0);
        let e2 = make_entity(1, 0);
        set.insert(e1, 10);
        set.insert(e2, 20);
        set.increment_tick();
        let _ = set.get_mut(e2);
        set.remove(e1);
        assert!(set.is_changed(e2));
    }

    #[test]
    fn is_changed_respects_generation() {
        // A stale handle to a slot that has been reused at a newer generation
        // must NOT be reported as changed — consistent with get/contains/remove,
        // which all reject a generation mismatch. (is_changed used to consult
        // the sparse slot's tick without verifying the stored entity matches.)
        let mut set = SparseSet::<u32>::new();
        let e = make_entity(5, 0);
        set.insert(e, 42);
        assert!(set.is_changed(e), "freshly inserted entity is changed");

        let stale = make_entity(5, 1); // same slot, different generation
        assert!(set.get(stale).is_none(), "get already guards generation");
        assert!(
            !set.is_changed(stale),
            "is_changed must reject a generation mismatch like get does"
        );
    }

    #[test]
    fn test_swap_remove_correctness() {
        let mut set: SparseSet<i32> = SparseSet::new();
        let e0 = make_entity(0, 0);
        let e1 = make_entity(1, 0);
        let e2 = make_entity(2, 0);
        set.insert(e0, 10);
        set.insert(e1, 20);
        set.insert(e2, 30);
        // Remove first (triggers swap with last)
        let removed = set.remove(e0);
        assert_eq!(removed, Some(10));
        assert_eq!(set.len(), 2);
        assert!(set.get(e0).is_none());
        // Remaining still accessible
        assert_eq!(set.get(e1), Some(&20));
        assert_eq!(set.get(e2), Some(&30));
    }
}
