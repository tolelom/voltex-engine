//! Single-type slab storage with ref counting and generation-checked handles.

use std::any::Any;

use crate::handle::Handle;

struct AssetEntry<T> {
    asset: T,
    generation: u32,
    ref_count: u32,
}

/// Slab storage for a single asset type `T`.
///
/// Slots are recycled via a free list — but each reuse bumps the generation
/// counter so old [`Handle`]s pointing at the freed slot are rejected. Each
/// live entry carries a reference count; the asset is removed when it drops
/// to zero (see [`release`](Self::release)).
pub struct AssetStorage<T> {
    entries: Vec<Option<AssetEntry<T>>>,
    free_ids: Vec<u32>,
}

impl<T> AssetStorage<T> {
    /// Creates an empty storage.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            free_ids: Vec::new(),
        }
    }

    /// Inserts a new asset and returns a handle with initial ref count = 1.
    pub fn insert(&mut self, asset: T) -> Handle<T> {
        if let Some(id) = self.free_ids.pop() {
            let generation = self.entries[id as usize]
                .as_ref()
                .map(|e| e.generation)
                .unwrap_or(0)
                + 1;
            self.entries[id as usize] = Some(AssetEntry {
                asset,
                generation,
                ref_count: 1,
            });
            Handle::new(id, generation)
        } else {
            let id = self.entries.len() as u32;
            self.entries.push(Some(AssetEntry {
                asset,
                generation: 0,
                ref_count: 1,
            }));
            Handle::new(id, 0)
        }
    }

    /// Returns a reference to the asset, or `None` if the handle is stale.
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.entries
            .get(handle.id as usize)?
            .as_ref()
            .filter(|e| e.generation == handle.generation)
            .map(|e| &e.asset)
    }

    /// Returns a mutable reference to the asset, or `None` if stale.
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.entries
            .get_mut(handle.id as usize)?
            .as_mut()
            .filter(|e| e.generation == handle.generation)
            .map(|e| &mut e.asset)
    }

    /// Bumps the asset's ref count by one. No-op for stale handles.
    pub fn add_ref(&mut self, handle: Handle<T>) {
        if let Some(Some(entry)) = self.entries.get_mut(handle.id as usize) {
            if entry.generation == handle.generation {
                entry.ref_count += 1;
            }
        }
    }

    /// Decrements the ref_count. Returns true if the asset was removed.
    pub fn release(&mut self, handle: Handle<T>) -> bool {
        if let Some(slot) = self.entries.get_mut(handle.id as usize) {
            if let Some(entry) = slot.as_mut() {
                if entry.generation == handle.generation {
                    entry.ref_count = entry.ref_count.saturating_sub(1);
                    if entry.ref_count == 0 {
                        *slot = None;
                        self.free_ids.push(handle.id);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Number of currently-live entries.
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    /// `true` if [`len`](Self::len) is zero.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the ref count for `handle`, or `0` if it is stale.
    pub fn ref_count(&self, handle: Handle<T>) -> u32 {
        self.entries
            .get(handle.id as usize)
            .and_then(|e| e.as_ref())
            .filter(|e| e.generation == handle.generation)
            .map(|e| e.ref_count)
            .unwrap_or(0)
    }

    /// Replace the asset data without changing generation or ref_count.
    /// Used for hot reload — existing handles remain valid.
    pub fn replace_in_place(&mut self, handle: Handle<T>, new_asset: T) -> bool {
        if let Some(Some(entry)) = self.entries.get_mut(handle.id as usize) {
            if entry.generation == handle.generation {
                entry.asset = new_asset;
                return true;
            }
        }
        false
    }

    /// Iterates `(handle, &asset)` over every live entry.
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(id, slot)| {
                slot.as_ref().map(|e| (Handle::new(id as u32, e.generation), &e.asset))
            })
    }
}

impl<T> Default for AssetStorage<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for type-erased access to an AssetStorage.
pub trait AssetStorageDyn: Any {
    /// Downcast view of `self` as `&dyn Any`.
    fn as_any(&self) -> &dyn Any;
    /// Downcast view of `self` as `&mut dyn Any`.
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Number of live assets in this storage.
    fn count(&self) -> usize;
}

impl<T: 'static> AssetStorageDyn for AssetStorage<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn count(&self) -> usize {
        self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mesh {
        verts: u32,
    }

    #[test]
    fn insert_and_get() {
        let mut storage: AssetStorage<Mesh> = AssetStorage::new();
        let h = storage.insert(Mesh { verts: 3 });
        assert_eq!(storage.get(h).unwrap().verts, 3);
    }

    #[test]
    fn get_mut() {
        let mut storage: AssetStorage<Mesh> = AssetStorage::new();
        let h = storage.insert(Mesh { verts: 3 });
        storage.get_mut(h).unwrap().verts = 6;
        assert_eq!(storage.get(h).unwrap().verts, 6);
    }

    #[test]
    fn release_removes_at_zero() {
        let mut storage: AssetStorage<Mesh> = AssetStorage::new();
        let h = storage.insert(Mesh { verts: 3 });
        assert_eq!(storage.len(), 1);
        let removed = storage.release(h);
        assert!(removed);
        assert_eq!(storage.len(), 0);
        assert!(storage.get(h).is_none());
    }

    #[test]
    fn ref_counting() {
        let mut storage: AssetStorage<Mesh> = AssetStorage::new();
        let h = storage.insert(Mesh { verts: 3 });
        storage.add_ref(h);
        assert_eq!(storage.ref_count(h), 2);
        let removed1 = storage.release(h);
        assert!(!removed1);
        assert_eq!(storage.ref_count(h), 1);
        let removed2 = storage.release(h);
        assert!(removed2);
        assert!(storage.get(h).is_none());
    }

    #[test]
    fn stale_handle() {
        let mut storage: AssetStorage<Mesh> = AssetStorage::new();
        let h = storage.insert(Mesh { verts: 3 });
        storage.release(h);
        // h is now stale; get should return None
        assert!(storage.get(h).is_none());
    }

    #[test]
    fn id_reuse() {
        let mut storage: AssetStorage<Mesh> = AssetStorage::new();
        let h1 = storage.insert(Mesh { verts: 3 });
        storage.release(h1);
        let h2 = storage.insert(Mesh { verts: 9 });
        // Same slot reused but different generation
        assert_eq!(h1.id, h2.id);
        assert_ne!(h1.generation, h2.generation);
        assert!(storage.get(h1).is_none());
        assert_eq!(storage.get(h2).unwrap().verts, 9);
    }

    #[test]
    fn replace_in_place() {
        let mut storage: AssetStorage<Mesh> = AssetStorage::new();
        let h = storage.insert(Mesh { verts: 3 });
        assert!(storage.replace_in_place(h, Mesh { verts: 99 }));
        assert_eq!(storage.get(h).unwrap().verts, 99);
        // Same handle still works — generation unchanged
        assert_eq!(storage.ref_count(h), 1);
    }

    #[test]
    fn replace_in_place_stale_handle() {
        let mut storage: AssetStorage<Mesh> = AssetStorage::new();
        let h = storage.insert(Mesh { verts: 3 });
        storage.release(h);
        let h2 = storage.insert(Mesh { verts: 10 });
        // h is stale, replace should fail
        assert!(!storage.replace_in_place(h, Mesh { verts: 99 }));
        assert_eq!(storage.get(h2).unwrap().verts, 10);
    }

    #[test]
    fn iter() {
        let mut storage: AssetStorage<Mesh> = AssetStorage::new();
        let h1 = storage.insert(Mesh { verts: 3 });
        let h2 = storage.insert(Mesh { verts: 6 });
        let mut verts: Vec<u32> = storage.iter().map(|(_, m)| m.verts).collect();
        verts.sort();
        assert_eq!(verts, vec![3, 6]);
        // handles from iter should be usable
        let handles: Vec<Handle<Mesh>> = storage.iter().map(|(h, _)| h).collect();
        assert!(handles.contains(&h1));
        assert!(handles.contains(&h2));
    }
}
