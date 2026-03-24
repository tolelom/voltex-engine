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
