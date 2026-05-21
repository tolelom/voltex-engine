//! `World` — owns the entity allocator + every type-erased component storage.

use std::any::TypeId;
use std::collections::HashMap;
use crate::entity::{Entity, EntityAllocator};
use crate::sparse_set::{SparseSet, ComponentStorage};

/// The container that owns every entity and every component storage.
///
/// `World` is the single source of truth for the editor and the runtime. Entity
/// lifecycle goes through [`World::spawn`] / [`World::despawn`]; component
/// access goes through [`World::add`] / [`World::get`] / [`World::get_mut`]
/// / [`World::remove`]; cross-cutting iteration uses [`World::query`].
pub struct World {
    allocator: EntityAllocator,
    storages: HashMap<TypeId, Box<dyn ComponentStorage>>,
}

impl World {
    /// Create an empty world with no entities and no component storages.
    pub fn new() -> Self {
        Self {
            allocator: EntityAllocator::new(),
            storages: HashMap::new(),
        }
    }

    /// Allocate a fresh entity. Reuses freed slots with bumped generations
    /// so prior [`Entity`] handles to dead slots stay stale.
    pub fn spawn(&mut self) -> Entity {
        self.allocator.allocate()
    }

    /// Mark `entity` dead and drop every attached component. Returns false
    /// if the entity was already dead. Stale handles (same id, lower
    /// generation) are rejected without side effects.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.allocator.deallocate(entity) {
            return false;
        }
        for storage in self.storages.values_mut() {
            storage.remove_entity(entity);
        }
        true
    }

    /// True if `entity` refers to a currently alive slot at the matching
    /// generation. Used by every command/system to reject stale handles.
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.allocator.is_alive(entity)
    }

    /// Number of currently alive entities.
    pub fn entity_count(&self) -> usize {
        self.allocator.alive_count()
    }

    /// Iterate every currently alive entity. Used by script systems and the
    /// Lua `iter_entities()` API to walk the world without component filters.
    pub fn live_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.allocator.live_entities()
    }

    /// Re-activate a previously despawned entity at its original generation.
    /// Returns false if the slot is alive, out of range, or has moved past
    /// `target_generation` (see `EntityAllocator::restore_at_generation`).
    ///
    /// Component storages remain untouched — despawn already removed them.
    /// Callers (e.g. `EditorSession` Despawn-undo) re-add components after
    /// a successful restore.
    pub fn restore_at_generation(&mut self, id: u32, target_generation: u32) -> bool {
        self.allocator.restore_at_generation(id, target_generation)
    }

    /// Drop every entity, every component storage, and the entity allocator.
    /// Any externally-held `Entity` handles become invalid (their `is_alive`
    /// returns false). Use before loading a scene from disk to enforce
    /// "disk is the source of truth" semantics.
    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// Attach (or overwrite) a component of type `T` on `entity`. The
    /// underlying storage is created lazily on first call per type.
    pub fn add<T: 'static>(&mut self, entity: Entity, component: T) {
        let type_id = TypeId::of::<T>();
        let storage = self.storages
            .entry(type_id)
            .or_insert_with(|| Box::new(SparseSet::<T>::new()));
        let set = storage.as_any_mut().downcast_mut::<SparseSet<T>>().unwrap();
        set.insert(entity, component);
    }

    /// Borrow the component of type `T` on `entity`, or `None` if absent.
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get(&type_id)?;
        let set = storage.as_any().downcast_ref::<SparseSet<T>>()?;
        set.get(entity)
    }

    /// Mutably borrow the component of type `T` on `entity`, or `None` if absent.
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get_mut(&type_id)?;
        let set = storage.as_any_mut().downcast_mut::<SparseSet<T>>()?;
        set.get_mut(entity)
    }

    /// Detach the component of type `T` from `entity` and return it, or
    /// `None` if it wasn't present.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get_mut(&type_id)?;
        let set = storage.as_any_mut().downcast_mut::<SparseSet<T>>()?;
        set.remove(entity)
    }

    /// Borrows the underlying [`SparseSet<T>`], if a storage for `T` exists yet.
    pub fn storage<T: 'static>(&self) -> Option<&SparseSet<T>> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get(&type_id)?;
        storage.as_any().downcast_ref::<SparseSet<T>>()
    }

    /// Mutable equivalent of [`storage`](Self::storage).
    pub fn storage_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get_mut(&type_id)?;
        storage.as_any_mut().downcast_mut::<SparseSet<T>>()
    }

    /// Iterates `(entity, &T)` for every entity that has component `T`.
    pub fn query<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.storage::<T>()
            .map(|s| s.iter())
            .into_iter()
            .flatten()
    }

    /// Joins every entity that has both `A` and `B`. Iterates the smaller
    /// storage and looks up in the larger.
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

    /// 3-way join — every entity that has `A` + `B` + `C`.
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

    /// 4-way join — every entity that has `A` + `B` + `C` + `D`.
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

    /// `true` if `entity` has a component of type `T`.
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
mod tests;
