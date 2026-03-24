use std::any::TypeId;
use std::collections::HashMap;

use crate::handle::Handle;
use crate::storage::{AssetStorage, AssetStorageDyn};

pub struct Assets {
    storages: HashMap<TypeId, Box<dyn AssetStorageDyn>>,
}

impl Assets {
    pub fn new() -> Self {
        Self {
            storages: HashMap::new(),
        }
    }

    fn storage_mut_or_insert<T: 'static>(&mut self) -> &mut AssetStorage<T> {
        self.storages
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(AssetStorage::<T>::new()))
            .as_any_mut()
            .downcast_mut::<AssetStorage<T>>()
            .unwrap()
    }

    pub fn insert<T: 'static>(&mut self, asset: T) -> Handle<T> {
        self.storage_mut_or_insert::<T>().insert(asset)
    }

    pub fn get<T: 'static>(&self, handle: Handle<T>) -> Option<&T> {
        self.storages
            .get(&TypeId::of::<T>())?
            .as_any()
            .downcast_ref::<AssetStorage<T>>()?
            .get(handle)
    }

    pub fn get_mut<T: 'static>(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.storages
            .get_mut(&TypeId::of::<T>())?
            .as_any_mut()
            .downcast_mut::<AssetStorage<T>>()?
            .get_mut(handle)
    }

    pub fn add_ref<T: 'static>(&mut self, handle: Handle<T>) {
        if let Some(storage) = self
            .storages
            .get_mut(&TypeId::of::<T>())
            .and_then(|s| s.as_any_mut().downcast_mut::<AssetStorage<T>>())
        {
            storage.add_ref(handle);
        }
    }

    pub fn release<T: 'static>(&mut self, handle: Handle<T>) -> bool {
        if let Some(storage) = self
            .storages
            .get_mut(&TypeId::of::<T>())
            .and_then(|s| s.as_any_mut().downcast_mut::<AssetStorage<T>>())
        {
            storage.release(handle)
        } else {
            false
        }
    }

    pub fn count<T: 'static>(&self) -> usize {
        self.storages
            .get(&TypeId::of::<T>())
            .map(|s| s.count())
            .unwrap_or(0)
    }

    pub fn storage<T: 'static>(&self) -> Option<&AssetStorage<T>> {
        self.storages
            .get(&TypeId::of::<T>())?
            .as_any()
            .downcast_ref::<AssetStorage<T>>()
    }

    pub fn storage_mut<T: 'static>(&mut self) -> &mut AssetStorage<T> {
        self.storage_mut_or_insert::<T>()
    }
}

impl Default for Assets {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mesh {
        verts: u32,
    }

    struct Texture {
        width: u32,
    }

    #[test]
    fn insert_and_get_different_types() {
        let mut assets = Assets::new();
        let hm = assets.insert(Mesh { verts: 3 });
        let ht = assets.insert(Texture { width: 512 });
        assert_eq!(assets.get(hm).unwrap().verts, 3);
        assert_eq!(assets.get(ht).unwrap().width, 512);
    }

    #[test]
    fn count_per_type() {
        let mut assets = Assets::new();
        assets.insert(Mesh { verts: 3 });
        assets.insert(Mesh { verts: 6 });
        assets.insert(Texture { width: 512 });
        assert_eq!(assets.count::<Mesh>(), 2);
        assert_eq!(assets.count::<Texture>(), 1);
    }

    #[test]
    fn release_through_assets() {
        let mut assets = Assets::new();
        let h = assets.insert(Mesh { verts: 3 });
        assert_eq!(assets.count::<Mesh>(), 1);
        let removed = assets.release(h);
        assert!(removed);
        assert_eq!(assets.count::<Mesh>(), 0);
        assert!(assets.get(h).is_none());
    }

    #[test]
    fn ref_counting_through_assets() {
        let mut assets = Assets::new();
        let h = assets.insert(Mesh { verts: 3 });
        assets.add_ref(h);
        let r1 = assets.release(h);
        assert!(!r1);
        assert!(assets.get(h).is_some());
        let r2 = assets.release(h);
        assert!(r2);
        assert!(assets.get(h).is_none());
    }

    #[test]
    fn storage_access() {
        let mut assets = Assets::new();
        let h = assets.insert(Mesh { verts: 3 });
        {
            let s = assets.storage::<Mesh>().unwrap();
            assert_eq!(s.len(), 1);
            assert_eq!(s.get(h).unwrap().verts, 3);
        }
        {
            let s = assets.storage_mut::<Mesh>();
            s.get_mut(h).unwrap().verts = 9;
        }
        assert_eq!(assets.get(h).unwrap().verts, 9);
    }
}
