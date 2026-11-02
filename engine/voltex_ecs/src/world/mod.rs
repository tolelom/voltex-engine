//! `World` — owns the entity allocator + every type-erased component storage.

use crate::entity::{Entity, EntityAllocator};
use crate::sparse_set::{ComponentStorage, SparseSet};
use std::any::{Any, TypeId};
use std::collections::HashMap;

/// The container that owns every entity and every component storage.
///
/// `World` is the single source of truth for the editor and the runtime. Entity
/// lifecycle goes through [`World::spawn`] / [`World::despawn`]; component
/// access goes through [`World::add`] / [`World::get`] / [`World::get_mut`]
/// / [`World::remove`]; cross-cutting iteration uses [`World::query`].
pub struct World {
    allocator: EntityAllocator,
    storages: HashMap<TypeId, Box<dyn ComponentStorage>>,
    /// Type-erased singleton resources (one value per type) — shared state that
    /// isn't tied to an entity: game config, score, event queues, RNG, etc.
    resources: HashMap<TypeId, Box<dyn Any>>,
}

impl World {
    /// Create an empty world with no entities and no component storages.
    pub fn new() -> Self {
        Self {
            allocator: EntityAllocator::new(),
            storages: HashMap::new(),
            resources: HashMap::new(),
        }
    }

    /// Insert (or replace) the singleton resource of type `T`. Resources are
    /// world-global shared state not attached to any entity — every system has
    /// `&mut World`, so any system can read/write them without extra plumbing.
    pub fn insert_resource<T: 'static>(&mut self, value: T) {
        self.resources.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Borrow the singleton resource of type `T`, or `None` if not present.
    pub fn resource<T: 'static>(&self) -> Option<&T> {
        self.resources
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<T>())
    }

    /// Mutably borrow the singleton resource of type `T`, or `None` if absent.
    pub fn resource_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.resources
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.downcast_mut::<T>())
    }

    /// Remove and return the singleton resource of type `T`, if present.
    pub fn remove_resource<T: 'static>(&mut self) -> Option<T> {
        self.resources
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast::<T>().ok())
            .map(|b| *b)
    }

    /// Borrow the resource of type `T`, inserting `default` first if absent.
    pub fn resource_or_insert_with<T: 'static>(&mut self, default: impl FnOnce() -> T) -> &mut T {
        self.resources
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(default()))
            .downcast_mut::<T>()
            .expect("resource TypeId maps to its own type")
    }

    /// True if a resource of type `T` is present.
    pub fn has_resource<T: 'static>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
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

    /// Drop every entity, component storage and resource. Entity slots are
    /// freed with a generation bump rather than reset, so externally-held
    /// `Entity` handles stay invalid even after new spawns reuse their ids.
    /// Use before loading a scene from disk to enforce "disk is the source of
    /// truth" semantics.
    pub fn clear(&mut self) {
        self.storages.clear();
        self.resources.clear();
        self.allocator.clear();
    }

    /// Attach (or overwrite) a component of type `T` on `entity`. The
    /// underlying storage is created lazily on first call per type. No-op if
    /// `entity` is not alive (despawned or stale handle).
    pub fn add<T: 'static>(&mut self, entity: Entity, component: T) {
        // Storage is keyed by slot id: writing through a stale handle would
        // overwrite (and hide) the component of the live entity that now
        // owns the slot. Adding to a dead entity is a no-op instead.
        if !self.is_alive(entity) {
            return;
        }
        let type_id = TypeId::of::<T>();
        let storage = self
            .storages
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
        self.storage::<T>().map(|s| s.iter()).into_iter().flatten()
    }

    // ----- query2/3/4 helpers ---------------------------------------
    //
    // The joins below dispatch at runtime to whichever storage is
    // smallest. To keep the return type `impl Iterator` (zero allocation)
    // we wrap the per-branch iterators in `Either{2,3,4}` enums that
    // implement `Iterator` themselves — a stack-allocated tagged union
    // instead of `Box<dyn Iterator>` (one alloc per call) or `Vec<…>`
    // (one alloc per call sized by the join result).

    /// Joins every entity that has both `A` and `B`. Iterates the smaller
    /// storage and looks up in the larger — returns a borrow-tied iterator
    /// to avoid the per-frame Vec allocation the previous API forced.
    pub fn query2<A: 'static, B: 'static>(&self) -> impl Iterator<Item = (Entity, &A, &B)> + '_ {
        let pair = self.storage::<A>().zip(self.storage::<B>());
        pair.into_iter().flat_map(|(a, b)| -> Either2<_, _> {
            if a.len() <= b.len() {
                Either2::A(
                    a.iter()
                        .filter_map(move |(e, av)| b.get(e).map(|bv| (e, av, bv))),
                )
            } else {
                Either2::B(
                    b.iter()
                        .filter_map(move |(e, bv)| a.get(e).map(|av| (e, av, bv))),
                )
            }
        })
    }

    /// 3-way join — every entity that has `A` + `B` + `C`. Iterator-returning
    /// to avoid per-frame Vec allocation.
    pub fn query3<A: 'static, B: 'static, C: 'static>(
        &self,
    ) -> impl Iterator<Item = (Entity, &A, &B, &C)> + '_ {
        let trip = self
            .storage::<A>()
            .zip(self.storage::<B>())
            .zip(self.storage::<C>())
            .map(|((a, b), c)| (a, b, c));
        trip.into_iter().flat_map(|(a, b, c)| -> Either3<_, _, _> {
            match argmin3(a.len(), b.len(), c.len()) {
                0 => Either3::A(
                    a.iter()
                        .filter_map(move |(e, av)| Some((e, av, b.get(e)?, c.get(e)?))),
                ),
                1 => Either3::B(
                    b.iter()
                        .filter_map(move |(e, bv)| Some((e, a.get(e)?, bv, c.get(e)?))),
                ),
                _ => Either3::C(
                    c.iter()
                        .filter_map(move |(e, cv)| Some((e, a.get(e)?, b.get(e)?, cv))),
                ),
            }
        })
    }

    /// 4-way join — every entity that has `A` + `B` + `C` + `D`. Iterator-returning
    /// to avoid per-frame Vec allocation.
    pub fn query4<A: 'static, B: 'static, C: 'static, D: 'static>(
        &self,
    ) -> impl Iterator<Item = (Entity, &A, &B, &C, &D)> + '_ {
        let quad = self
            .storage::<A>()
            .zip(self.storage::<B>())
            .zip(self.storage::<C>())
            .zip(self.storage::<D>())
            .map(|(((a, b), c), d)| (a, b, c, d));
        quad.into_iter()
            .flat_map(|(a, b, c, d)| -> Either4<_, _, _, _> {
                match argmin4(a.len(), b.len(), c.len(), d.len()) {
                    0 => {
                        Either4::A(a.iter().filter_map(move |(e, av)| {
                            Some((e, av, b.get(e)?, c.get(e)?, d.get(e)?))
                        }))
                    }
                    1 => {
                        Either4::B(b.iter().filter_map(move |(e, bv)| {
                            Some((e, a.get(e)?, bv, c.get(e)?, d.get(e)?))
                        }))
                    }
                    2 => {
                        Either4::C(c.iter().filter_map(move |(e, cv)| {
                            Some((e, a.get(e)?, b.get(e)?, cv, d.get(e)?))
                        }))
                    }
                    _ => {
                        Either4::D(d.iter().filter_map(move |(e, dv)| {
                            Some((e, a.get(e)?, b.get(e)?, c.get(e)?, dv))
                        }))
                    }
                }
            })
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
        self.query2::<A, B>()
            .filter(|(e, _, _)| self.has_component::<W>(*e))
            .collect()
    }

    /// Query entities with components A and B, that do NOT have component W.
    pub fn query2_without<A: 'static, B: 'static, W: 'static>(&self) -> Vec<(Entity, &A, &B)> {
        self.query2::<A, B>()
            .filter(|(e, _, _)| !self.has_component::<W>(*e))
            .collect()
    }
}

// ---------- internal join machinery (see query2/3/4) ----------

/// Returns the 0-based index of the smallest of three values (ties → leftmost).
fn argmin3(a: usize, b: usize, c: usize) -> usize {
    if a <= b && a <= c {
        0
    } else if b <= c {
        1
    } else {
        2
    }
}

/// Returns the 0-based index of the smallest of four values (ties → leftmost).
fn argmin4(a: usize, b: usize, c: usize, d: usize) -> usize {
    let mut min = a;
    let mut idx = 0;
    if b < min {
        min = b;
        idx = 1;
    }
    if c < min {
        min = c;
        idx = 2;
    }
    if d < min {
        idx = 3;
    }
    idx
}

/// Stack-allocated tagged union of two iterators with the same item type.
/// Lets `query2` return `impl Iterator` without `Box<dyn>`.
enum Either2<A, B> {
    A(A),
    B(B),
}

impl<A, B, T> Iterator for Either2<A, B>
where
    A: Iterator<Item = T>,
    B: Iterator<Item = T>,
{
    type Item = T;
    fn next(&mut self) -> Option<T> {
        match self {
            Self::A(i) => i.next(),
            Self::B(i) => i.next(),
        }
    }
}

/// 3-way tagged-union iterator (see [`Either2`]).
enum Either3<A, B, C> {
    A(A),
    B(B),
    C(C),
}

impl<A, B, C, T> Iterator for Either3<A, B, C>
where
    A: Iterator<Item = T>,
    B: Iterator<Item = T>,
    C: Iterator<Item = T>,
{
    type Item = T;
    fn next(&mut self) -> Option<T> {
        match self {
            Self::A(i) => i.next(),
            Self::B(i) => i.next(),
            Self::C(i) => i.next(),
        }
    }
}

/// 4-way tagged-union iterator (see [`Either2`]).
enum Either4<A, B, C, D> {
    A(A),
    B(B),
    C(C),
    D(D),
}

impl<A, B, C, D, T> Iterator for Either4<A, B, C, D>
where
    A: Iterator<Item = T>,
    B: Iterator<Item = T>,
    C: Iterator<Item = T>,
    D: Iterator<Item = T>,
{
    type Item = T;
    fn next(&mut self) -> Option<T> {
        match self {
            Self::A(i) => i.next(),
            Self::B(i) => i.next(),
            Self::C(i) => i.next(),
            Self::D(i) => i.next(),
        }
    }
}

#[cfg(test)]
mod join_helper_tests {
    use super::{argmin3, argmin4};

    #[test]
    fn argmin3_picks_smallest_and_left_on_tie() {
        assert_eq!(argmin3(1, 2, 3), 0);
        assert_eq!(argmin3(3, 1, 2), 1);
        assert_eq!(argmin3(3, 2, 1), 2);
        // ties resolve to the leftmost
        assert_eq!(argmin3(2, 2, 3), 0);
        assert_eq!(argmin3(3, 2, 2), 1);
        assert_eq!(argmin3(2, 2, 2), 0);
    }

    #[test]
    fn argmin4_picks_smallest_and_left_on_tie() {
        assert_eq!(argmin4(1, 2, 3, 4), 0);
        assert_eq!(argmin4(4, 1, 2, 3), 1);
        assert_eq!(argmin4(4, 3, 1, 2), 2);
        assert_eq!(argmin4(4, 3, 2, 1), 3);
        // ties resolve to the leftmost
        assert_eq!(argmin4(2, 2, 3, 4), 0);
        assert_eq!(argmin4(3, 2, 2, 4), 1);
        assert_eq!(argmin4(4, 4, 2, 2), 2);
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
