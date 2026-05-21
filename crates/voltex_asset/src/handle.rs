//! Generation-checked asset reference. Stale handles (slot was freed and the
//! id reused) are detected by the `generation` field instead of dangling.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// Typed reference to an asset stored in an [`AssetStorage`](crate::AssetStorage)
/// or [`Assets`](crate::Assets).
///
/// Implements `Copy` so handles can be cloned freely. Equality compares both
/// the slot id and the generation — a handle whose slot was freed and reused
/// will never compare equal to the new occupant's handle.
pub struct Handle<T> {
    pub(crate) id: u32,
    pub(crate) generation: u32,
    _marker: PhantomData<T>,
}

impl<T> Handle<T> {
    pub(crate) fn new(id: u32, generation: u32) -> Self {
        Self {
            id,
            generation,
            _marker: PhantomData,
        }
    }

    /// Returns the raw id (slot index) of this handle.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Returns the generation counter of this handle.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Reconstructs a handle from raw (id, generation) parts.
    ///
    /// **Validity:** the caller is responsible for ensuring the resulting
    /// handle refers to a still-live asset. Used by scene deserialization to
    /// restore previously persisted handles; the normal path to obtain a
    /// handle is `AssetStorage::insert`.
    pub fn from_raw_parts(id: u32, generation: u32) -> Self {
        Self {
            id,
            generation,
            _marker: PhantomData,
        }
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handle")
            .field("id", &self.id)
            .field("generation", &self.generation)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;

    #[test]
    fn test_handle_copy() {
        let h: Handle<Dummy> = Handle::new(0, 0);
        let h2 = h; // copy
        let h3 = h; // still usable after copy
        assert_eq!(h2, h3);
    }

    #[test]
    fn test_handle_eq() {
        let h1: Handle<Dummy> = Handle::new(1, 2);
        let h2: Handle<Dummy> = Handle::new(1, 2);
        let h3: Handle<Dummy> = Handle::new(1, 3);
        let h4: Handle<Dummy> = Handle::new(2, 2);

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert_ne!(h1, h4);
    }

    #[test]
    fn test_handle_accessors() {
        let h: Handle<Dummy> = Handle::from_raw_parts(3, 9);
        assert_eq!(h.id(), 3);
        assert_eq!(h.generation(), 9);
    }

    #[test]
    fn test_handle_from_raw_parts_roundtrip() {
        let h: Handle<Dummy> = Handle::from_raw_parts(42, 7);
        let h2: Handle<Dummy> = Handle::from_raw_parts(h.id(), h.generation());
        assert_eq!(h, h2);
    }
}
