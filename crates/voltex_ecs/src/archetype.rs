//! Archetype storage — groups entities sharing the same component set.

use std::any::TypeId;
use std::collections::HashMap;
use crate::entity::Entity;

/// An archetype signature: sorted set of component TypeIds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchetypeSignature(Vec<TypeId>);

impl ArchetypeSignature {
    /// Creates a signature; `types` is sorted + deduplicated for stable hashing.
    pub fn new(mut types: Vec<TypeId>) -> Self {
        types.sort();
        types.dedup();
        ArchetypeSignature(types)
    }

    /// `true` if the signature contains `type_id`.
    pub fn contains(&self, type_id: &TypeId) -> bool {
        self.0.contains(type_id)
    }

    /// Borrows the sorted slice of `TypeId`s.
    pub fn types(&self) -> &[TypeId] {
        &self.0
    }

    /// `true` if every type in `required` is part of this signature.
    pub fn matches_all(&self, required: &[TypeId]) -> bool {
        required.iter().all(|t| self.0.contains(t))
    }
}

/// A single archetype: stores entities and their component data.
pub struct Archetype {
    /// Sorted component signature shared by every entity in this archetype.
    pub signature: ArchetypeSignature,
    /// Entities currently in this archetype.
    pub entities: Vec<Entity>,
    // In a full implementation, component data would be stored in typed columns.
    // For now, just track which entities belong to this archetype.
}

impl Archetype {
    /// Creates an empty archetype with the given signature.
    pub fn new(signature: ArchetypeSignature) -> Self {
        Archetype { signature, entities: Vec::new() }
    }

    /// Appends `entity` to this archetype.
    pub fn add_entity(&mut self, entity: Entity) {
        self.entities.push(entity);
    }

    /// Removes `entity` if present. Returns `true` on success.
    pub fn remove_entity(&mut self, entity: Entity) -> bool {
        if let Some(pos) = self.entities.iter().position(|&e| e.id == entity.id) {
            self.entities.swap_remove(pos);
            true
        } else {
            false
        }
    }

    /// Number of entities in this archetype.
    pub fn len(&self) -> usize { self.entities.len() }
    /// `true` if no entities are in this archetype.
    pub fn is_empty(&self) -> bool { self.entities.is_empty() }
}

/// Registry of archetypes.
pub struct ArchetypeRegistry {
    archetypes: Vec<Archetype>,
    entity_archetype: HashMap<u32, usize>, // entity_id -> archetype index
}

impl Default for ArchetypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchetypeRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        ArchetypeRegistry { archetypes: Vec::new(), entity_archetype: HashMap::new() }
    }

    /// Find or create archetype for the given signature.
    pub fn get_or_create(&mut self, signature: ArchetypeSignature) -> usize {
        if let Some(idx) = self.archetypes.iter().position(|a| a.signature == signature) {
            return idx;
        }
        let idx = self.archetypes.len();
        self.archetypes.push(Archetype::new(signature));
        idx
    }

    /// Assign entity to an archetype.
    pub fn assign(&mut self, entity: Entity, signature: ArchetypeSignature) {
        // Remove from old archetype
        if let Some(&old_idx) = self.entity_archetype.get(&entity.id) {
            self.archetypes[old_idx].remove_entity(entity);
        }
        let idx = self.get_or_create(signature);
        self.archetypes[idx].add_entity(entity);
        self.entity_archetype.insert(entity.id, idx);
    }

    /// Find all archetypes that contain all required component types.
    pub fn matching_archetypes(&self, required: &[TypeId]) -> Vec<&Archetype> {
        self.archetypes.iter().filter(|a| a.signature.matches_all(required)).collect()
    }

    /// Number of distinct archetypes registered.
    pub fn archetype_count(&self) -> usize { self.archetypes.len() }

    /// Total number of entities tracked across all archetypes.
    pub fn entity_count(&self) -> usize { self.entity_archetype.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_sorted() {
        let a = TypeId::of::<u32>();
        let b = TypeId::of::<f32>();
        let sig1 = ArchetypeSignature::new(vec![a, b]);
        let sig2 = ArchetypeSignature::new(vec![b, a]);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_signature_contains() {
        let sig = ArchetypeSignature::new(vec![TypeId::of::<u32>(), TypeId::of::<f32>()]);
        assert!(sig.contains(&TypeId::of::<u32>()));
        assert!(!sig.contains(&TypeId::of::<bool>()));
    }

    #[test]
    fn test_registry_get_or_create() {
        let mut reg = ArchetypeRegistry::new();
        let sig = ArchetypeSignature::new(vec![TypeId::of::<u32>()]);
        let idx1 = reg.get_or_create(sig.clone());
        let idx2 = reg.get_or_create(sig);
        assert_eq!(idx1, idx2);
        assert_eq!(reg.archetype_count(), 1);
    }

    #[test]
    fn test_assign_entity() {
        let mut reg = ArchetypeRegistry::new();
        let e = Entity { id: 0, generation: 0 };
        let sig = ArchetypeSignature::new(vec![TypeId::of::<u32>()]);
        reg.assign(e, sig);
        assert_eq!(reg.entity_count(), 1);
    }

    #[test]
    fn test_matching_archetypes() {
        let mut reg = ArchetypeRegistry::new();
        let e1 = Entity { id: 0, generation: 0 };
        let e2 = Entity { id: 1, generation: 0 };
        reg.assign(e1, ArchetypeSignature::new(vec![TypeId::of::<u32>(), TypeId::of::<f32>()]));
        reg.assign(e2, ArchetypeSignature::new(vec![TypeId::of::<u32>()]));
        let matches = reg.matching_archetypes(&[TypeId::of::<u32>()]);
        assert_eq!(matches.len(), 2); // both have u32
    }

    #[test]
    fn test_reassign_entity() {
        let mut reg = ArchetypeRegistry::new();
        let e = Entity { id: 0, generation: 0 };
        reg.assign(e, ArchetypeSignature::new(vec![TypeId::of::<u32>()]));
        reg.assign(e, ArchetypeSignature::new(vec![TypeId::of::<u32>(), TypeId::of::<f32>()]));
        assert_eq!(reg.entity_count(), 1);
        assert_eq!(reg.archetype_count(), 2);
    }
}
