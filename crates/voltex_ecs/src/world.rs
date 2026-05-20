use std::any::TypeId;
use std::collections::HashMap;
use crate::entity::{Entity, EntityAllocator};
use crate::sparse_set::{SparseSet, ComponentStorage};

pub struct World {
    allocator: EntityAllocator,
    storages: HashMap<TypeId, Box<dyn ComponentStorage>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            allocator: EntityAllocator::new(),
            storages: HashMap::new(),
        }
    }

    pub fn spawn(&mut self) -> Entity {
        self.allocator.allocate()
    }

    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.allocator.deallocate(entity) {
            return false;
        }
        for storage in self.storages.values_mut() {
            storage.remove_entity(entity);
        }
        true
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.allocator.is_alive(entity)
    }

    pub fn entity_count(&self) -> usize {
        self.allocator.alive_count()
    }

    pub fn add<T: 'static>(&mut self, entity: Entity, component: T) {
        let type_id = TypeId::of::<T>();
        let storage = self.storages
            .entry(type_id)
            .or_insert_with(|| Box::new(SparseSet::<T>::new()));
        let set = storage.as_any_mut().downcast_mut::<SparseSet<T>>().unwrap();
        set.insert(entity, component);
    }

    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get(&type_id)?;
        let set = storage.as_any().downcast_ref::<SparseSet<T>>()?;
        set.get(entity)
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get_mut(&type_id)?;
        let set = storage.as_any_mut().downcast_mut::<SparseSet<T>>()?;
        set.get_mut(entity)
    }

    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get_mut(&type_id)?;
        let set = storage.as_any_mut().downcast_mut::<SparseSet<T>>()?;
        set.remove(entity)
    }

    pub fn storage<T: 'static>(&self) -> Option<&SparseSet<T>> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get(&type_id)?;
        storage.as_any().downcast_ref::<SparseSet<T>>()
    }

    pub fn storage_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get_mut(&type_id)?;
        storage.as_any_mut().downcast_mut::<SparseSet<T>>()
    }

    pub fn query<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.storage::<T>()
            .map(|s| s.iter())
            .into_iter()
            .flatten()
    }

    pub fn query2<A: 'static, B: 'static>(&self) -> Vec<(Entity, &A, &B)> {
        let a_storage = match self.storage::<A>() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let b_storage = match self.storage::<B>() {
            Some(s) => s,
            None => return Vec::new(),
        };

        // Iterate the smaller set, look up in the larger
        let mut result = Vec::new();
        if a_storage.len() <= b_storage.len() {
            for (entity, a) in a_storage.iter() {
                if let Some(b) = b_storage.get(entity) {
                    result.push((entity, a, b));
                }
            }
        } else {
            for (entity, b) in b_storage.iter() {
                if let Some(a) = a_storage.get(entity) {
                    result.push((entity, a, b));
                }
            }
        }
        result
    }

    pub fn query3<A: 'static, B: 'static, C: 'static>(&self) -> Vec<(Entity, &A, &B, &C)> {
        let a_storage = match self.storage::<A>() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let b_storage = match self.storage::<B>() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let c_storage = match self.storage::<C>() {
            Some(s) => s,
            None => return Vec::new(),
        };

        // Find the smallest storage to iterate
        let a_len = a_storage.len();
        let b_len = b_storage.len();
        let c_len = c_storage.len();

        let mut result = Vec::new();
        if a_len <= b_len && a_len <= c_len {
            for (entity, a) in a_storage.iter() {
                if let (Some(b), Some(c)) = (b_storage.get(entity), c_storage.get(entity)) {
                    result.push((entity, a, b, c));
                }
            }
        } else if b_len <= a_len && b_len <= c_len {
            for (entity, b) in b_storage.iter() {
                if let (Some(a), Some(c)) = (a_storage.get(entity), c_storage.get(entity)) {
                    result.push((entity, a, b, c));
                }
            }
        } else {
            for (entity, c) in c_storage.iter() {
                if let (Some(a), Some(b)) = (a_storage.get(entity), b_storage.get(entity)) {
                    result.push((entity, a, b, c));
                }
            }
        }
        result
    }

    pub fn query4<A: 'static, B: 'static, C: 'static, D: 'static>(
        &self,
    ) -> Vec<(Entity, &A, &B, &C, &D)> {
        let a_storage = match self.storage::<A>() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let b_storage = match self.storage::<B>() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let c_storage = match self.storage::<C>() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let d_storage = match self.storage::<D>() {
            Some(s) => s,
            None => return Vec::new(),
        };

        // Find the smallest storage to iterate
        let a_len = a_storage.len();
        let b_len = b_storage.len();
        let c_len = c_storage.len();
        let d_len = d_storage.len();

        let mut result = Vec::new();
        if a_len <= b_len && a_len <= c_len && a_len <= d_len {
            for (entity, a) in a_storage.iter() {
                if let (Some(b), Some(c), Some(d)) = (
                    b_storage.get(entity),
                    c_storage.get(entity),
                    d_storage.get(entity),
                ) {
                    result.push((entity, a, b, c, d));
                }
            }
        } else if b_len <= a_len && b_len <= c_len && b_len <= d_len {
            for (entity, b) in b_storage.iter() {
                if let (Some(a), Some(c), Some(d)) = (
                    a_storage.get(entity),
                    c_storage.get(entity),
                    d_storage.get(entity),
                ) {
                    result.push((entity, a, b, c, d));
                }
            }
        } else if c_len <= a_len && c_len <= b_len && c_len <= d_len {
            for (entity, c) in c_storage.iter() {
                if let (Some(a), Some(b), Some(d)) = (
                    a_storage.get(entity),
                    b_storage.get(entity),
                    d_storage.get(entity),
                ) {
                    result.push((entity, a, b, c, d));
                }
            }
        } else {
            for (entity, d) in d_storage.iter() {
                if let (Some(a), Some(b), Some(c)) = (
                    a_storage.get(entity),
                    b_storage.get(entity),
                    c_storage.get(entity),
                ) {
                    result.push((entity, a, b, c, d));
                }
            }
        }
        result
    }

    /// Query entities whose component T was changed this tick.
    pub fn query_changed<T: 'static>(&self) -> Vec<(Entity, &T)> {
        if let Some(storage) = self.storages.get(&TypeId::of::<T>()) {
            let set = storage.as_any().downcast_ref::<SparseSet<T>>().unwrap();
            set.iter_changed().collect()
        } else {
            Vec::new()
        }
    }

    /// Advance tick on all component storages (call at end of frame).
    pub fn clear_changed(&mut self) {
        for storage in self.storages.values_mut() {
            storage.increment_tick();
        }
    }

    pub fn has_component<T: 'static>(&self, entity: Entity) -> bool {
        self.storage::<T>().is_some_and(|s| s.contains(entity))
    }

    /// Query entities that have component T AND also have component W.
    pub fn query_with<T: 'static, W: 'static>(&self) -> Vec<(Entity, &T)> {
        let t_storage = match self.storage::<T>() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let mut result = Vec::new();
        for (entity, data) in t_storage.iter() {
            if self.has_component::<W>(entity) {
                result.push((entity, data));
            }
        }
        result
    }

    /// Query entities that have component T but NOT component W.
    pub fn query_without<T: 'static, W: 'static>(&self) -> Vec<(Entity, &T)> {
        let t_storage = match self.storage::<T>() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let mut result = Vec::new();
        for (entity, data) in t_storage.iter() {
            if !self.has_component::<W>(entity) {
                result.push((entity, data));
            }
        }
        result
    }

    /// Query entities with components A and B, that also have component W.
    pub fn query2_with<A: 'static, B: 'static, W: 'static>(&self) -> Vec<(Entity, &A, &B)> {
        self.query2::<A, B>().into_iter()
            .filter(|(e, _, _)| self.has_component::<W>(*e))
            .collect()
    }

    /// Query entities with components A and B, that do NOT have component W.
    pub fn query2_without<A: 'static, B: 'static, W: 'static>(&self) -> Vec<(Entity, &A, &B)> {
        self.query2::<A, B>().into_iter()
            .filter(|(e, _, _)| !self.has_component::<W>(*e))
            .collect()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Position { x: f32, y: f32 }

    #[derive(Debug, PartialEq)]
    struct Velocity { dx: f32, dy: f32 }

    #[derive(Debug, PartialEq)]
    struct Name(String);

    #[test]
    fn test_spawn_and_add() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Position { x: 1.0, y: 2.0 });
        let pos = world.get::<Position>(e).unwrap();
        assert_eq!(pos.x, 1.0);
        assert_eq!(pos.y, 2.0);
    }

    #[test]
    fn test_get_missing() {
        let world = World::new();
        let e = Entity { id: 0, generation: 0 };
        assert!(world.get::<Position>(e).is_none());
    }

    #[test]
    fn test_get_mut() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Position { x: 0.0, y: 0.0 });
        {
            let pos = world.get_mut::<Position>(e).unwrap();
            pos.x = 42.0;
        }
        assert_eq!(world.get::<Position>(e).unwrap().x, 42.0);
    }

    #[test]
    fn test_remove_component() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Position { x: 5.0, y: 6.0 });
        let removed = world.remove::<Position>(e);
        assert_eq!(removed, Some(Position { x: 5.0, y: 6.0 }));
        assert!(world.get::<Position>(e).is_none());
    }

    #[test]
    fn test_despawn() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Position { x: 1.0, y: 2.0 });
        world.add(e, Velocity { dx: 3.0, dy: 4.0 });
        assert!(world.despawn(e));
        assert!(!world.is_alive(e));
        assert!(world.get::<Position>(e).is_none());
        assert!(world.get::<Velocity>(e).is_none());
    }

    #[test]
    fn test_query_single() {
        let mut world = World::new();
        let e0 = world.spawn();
        let e1 = world.spawn();
        let _e2 = world.spawn(); // no Position
        world.add(e0, Position { x: 1.0, y: 0.0 });
        world.add(e1, Position { x: 2.0, y: 0.0 });

        let results: Vec<(Entity, &Position)> = world.query::<Position>().collect();
        assert_eq!(results.len(), 2);
        let entities: Vec<Entity> = results.iter().map(|(e, _)| *e).collect();
        assert!(entities.contains(&e0));
        assert!(entities.contains(&e1));
    }

    #[test]
    fn test_query2() {
        let mut world = World::new();
        let e0 = world.spawn();
        let e1 = world.spawn();
        let e2 = world.spawn(); // only Position, no Velocity
        world.add(e0, Position { x: 1.0, y: 0.0 });
        world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
        world.add(e1, Position { x: 2.0, y: 0.0 });
        world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
        world.add(e2, Position { x: 3.0, y: 0.0 });

        let results = world.query2::<Position, Velocity>();
        assert_eq!(results.len(), 2);
        let entities: Vec<Entity> = results.iter().map(|(e, _, _)| *e).collect();
        assert!(entities.contains(&e0));
        assert!(entities.contains(&e1));
        assert!(!entities.contains(&e2));
    }

    #[test]
    fn test_query3() {
        #[derive(Debug, PartialEq)]
        struct Health(i32);

        let mut world = World::new();
        let e0 = world.spawn();
        world.add(e0, Position { x: 1.0, y: 0.0 });
        world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
        world.add(e0, Health(100));

        let e1 = world.spawn();
        world.add(e1, Position { x: 2.0, y: 0.0 });
        world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
        // e1 has no Health

        let results = world.query3::<Position, Velocity, Health>();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e0);
    }

    #[test]
    fn test_query4() {
        #[derive(Debug, PartialEq)]
        struct Health(i32);

        #[derive(Debug, PartialEq)]
        struct Tag(u8);

        let mut world = World::new();
        let e0 = world.spawn();
        world.add(e0, Position { x: 1.0, y: 0.0 });
        world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
        world.add(e0, Health(100));
        world.add(e0, Tag(1));

        let e1 = world.spawn();
        world.add(e1, Position { x: 2.0, y: 0.0 });
        world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
        world.add(e1, Health(50));
        // e1 has no Tag

        let e2 = world.spawn();
        world.add(e2, Position { x: 3.0, y: 0.0 });
        world.add(e2, Velocity { dx: 3.0, dy: 0.0 });
        world.add(e2, Tag(2));
        // e2 has no Health

        let results = world.query4::<Position, Velocity, Health, Tag>();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e0);
    }

    #[test]
    fn test_has_component() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Position { x: 1.0, y: 2.0 });
        assert!(world.has_component::<Position>(e));
        assert!(!world.has_component::<Velocity>(e));
    }

    #[test]
    fn test_query_with() {
        let mut world = World::new();
        let e0 = world.spawn();
        let e1 = world.spawn();
        let e2 = world.spawn();
        world.add(e0, Position { x: 1.0, y: 0.0 });
        world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
        world.add(e1, Position { x: 2.0, y: 0.0 });
        // e1 has Position but no Velocity
        world.add(e2, Position { x: 3.0, y: 0.0 });
        world.add(e2, Velocity { dx: 3.0, dy: 0.0 });

        let results = world.query_with::<Position, Velocity>();
        assert_eq!(results.len(), 2);
        let entities: Vec<Entity> = results.iter().map(|(e, _)| *e).collect();
        assert!(entities.contains(&e0));
        assert!(entities.contains(&e2));
        assert!(!entities.contains(&e1));
    }

    #[test]
    fn test_query_without() {
        let mut world = World::new();
        let e0 = world.spawn();
        let e1 = world.spawn();
        let e2 = world.spawn();
        world.add(e0, Position { x: 1.0, y: 0.0 });
        world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
        world.add(e1, Position { x: 2.0, y: 0.0 });
        // e1 has Position but no Velocity — should be included
        world.add(e2, Position { x: 3.0, y: 0.0 });
        world.add(e2, Velocity { dx: 3.0, dy: 0.0 });

        let results = world.query_without::<Position, Velocity>();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e1);
    }

    #[test]
    fn test_query2_with() {
        #[derive(Debug, PartialEq)]
        struct Health(i32);

        let mut world = World::new();
        let e0 = world.spawn();
        world.add(e0, Position { x: 1.0, y: 0.0 });
        world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
        world.add(e0, Health(100));

        let e1 = world.spawn();
        world.add(e1, Position { x: 2.0, y: 0.0 });
        world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
        // e1 has no Health

        let results = world.query2_with::<Position, Velocity, Health>();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e0);
    }

    #[test]
    fn test_query2_without() {
        #[derive(Debug, PartialEq)]
        struct Health(i32);

        let mut world = World::new();
        let e0 = world.spawn();
        world.add(e0, Position { x: 1.0, y: 0.0 });
        world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
        world.add(e0, Health(100));

        let e1 = world.spawn();
        world.add(e1, Position { x: 2.0, y: 0.0 });
        world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
        // e1 has no Health

        let results = world.query2_without::<Position, Velocity, Health>();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e1);
    }

    #[test]
    fn test_query_changed() {
        let mut world = World::new();
        let e1 = world.spawn();
        let e2 = world.spawn();
        world.add(e1, 10u32);
        world.add(e2, 20u32);
        world.clear_changed();
        if let Some(v) = world.get_mut::<u32>(e1) {
            *v = 100;
        }
        let changed = world.query_changed::<u32>();
        assert_eq!(changed.len(), 1);
        assert_eq!(*changed[0].1, 100);
    }

    #[test]
    fn test_clear_changed_all_storages() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, 42u32);
        world.add(e, 3.14f32);
        let changed_u32 = world.query_changed::<u32>();
        assert_eq!(changed_u32.len(), 1);
        world.clear_changed();
        let changed_u32 = world.query_changed::<u32>();
        assert_eq!(changed_u32.len(), 0);
        let changed_f32 = world.query_changed::<f32>();
        assert_eq!(changed_f32.len(), 0);
    }

    #[test]
    fn test_entity_count() {
        let mut world = World::new();
        assert_eq!(world.entity_count(), 0);
        let e0 = world.spawn();
        let e1 = world.spawn();
        assert_eq!(world.entity_count(), 2);
        world.despawn(e0);
        assert_eq!(world.entity_count(), 1);
        world.despawn(e1);
        assert_eq!(world.entity_count(), 0);
    }
}
