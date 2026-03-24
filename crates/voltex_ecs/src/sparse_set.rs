use std::any::Any;
use crate::entity::Entity;

pub struct SparseSet<T> {
    sparse: Vec<Option<usize>>,
    dense_entities: Vec<Entity>,
    dense_data: Vec<T>,
}

impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self {
            sparse: Vec::new(),
            dense_entities: Vec::new(),
            dense_data: Vec::new(),
        }
    }

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
        } else {
            let dense_idx = self.dense_data.len();
            self.sparse[id] = Some(dense_idx);
            self.dense_entities.push(entity);
            self.dense_data.push(value);
        }
    }

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
            Some(self.dense_data.pop().unwrap())
        } else {
            // Swap with last
            let swapped_entity = self.dense_entities[last_idx];
            self.sparse[swapped_entity.id as usize] = Some(dense_idx);
            self.dense_entities.swap_remove(dense_idx);
            Some(self.dense_data.swap_remove(dense_idx))
        }
    }

    pub fn get(&self, entity: Entity) -> Option<&T> {
        let id = entity.id as usize;
        let dense_idx = self.sparse.get(id)?.as_ref().copied()?;
        if self.dense_entities[dense_idx] != entity {
            return None;
        }
        Some(&self.dense_data[dense_idx])
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let id = entity.id as usize;
        let dense_idx = self.sparse.get(id)?.as_ref().copied()?;
        if self.dense_entities[dense_idx] != entity {
            return None;
        }
        Some(&mut self.dense_data[dense_idx])
    }

    pub fn contains(&self, entity: Entity) -> bool {
        let id = entity.id as usize;
        self.sparse
            .get(id)
            .and_then(|opt| opt.as_ref())
            .map_or(false, |&dense_idx| {
                self.dense_entities[dense_idx] == entity
            })
    }

    pub fn len(&self) -> usize {
        self.dense_data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense_data.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.dense_entities.iter().copied().zip(self.dense_data.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
        self.dense_entities.iter().copied().zip(self.dense_data.iter_mut())
    }

    pub fn entities(&self) -> &[Entity] {
        &self.dense_entities
    }

    pub fn data(&self) -> &[T] {
        &self.dense_data
    }

    pub fn data_mut(&mut self) -> &mut [T] {
        &mut self.dense_data
    }
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait ComponentStorage: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn remove_entity(&mut self, entity: Entity);
    fn storage_len(&self) -> usize;
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
