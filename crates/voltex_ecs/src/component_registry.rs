/// Registration-based component serialization for scene formats.
/// Each registered component type has a name, a serialize function,
/// and a deserialize function.

use crate::entity::Entity;
use crate::world::World;

pub type SerializeFn = fn(&World, Entity) -> Option<Vec<u8>>;
pub type DeserializeFn = fn(&mut World, Entity, &[u8]) -> crate::Result<()>;

pub struct ComponentEntry {
    pub name: String,
    pub serialize: SerializeFn,
    pub deserialize: DeserializeFn,
}

pub struct ComponentRegistry {
    entries: Vec<ComponentEntry>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn register(&mut self, name: &str, ser: SerializeFn, deser: DeserializeFn) {
        self.entries.push(ComponentEntry {
            name: name.to_string(),
            serialize: ser,
            deserialize: deser,
        });
    }

    pub fn find(&self, name: &str) -> Option<&ComponentEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn entries(&self) -> &[ComponentEntry] {
        &self.entries
    }

    /// Register the default built-in component types: transform and tag.
    pub fn register_defaults(&mut self) {
        self.register("transform", serialize_transform, deserialize_transform);
        self.register("tag", serialize_tag, deserialize_tag);
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Transform: 9 f32s in little-endian (pos.xyz, rot.xyz, scale.xyz) ─

fn serialize_transform(world: &World, entity: Entity) -> Option<Vec<u8>> {
    let t = world.get::<crate::Transform>(entity)?;
    let mut data = Vec::with_capacity(36);
    for &v in &[
        t.position.x, t.position.y, t.position.z,
        t.rotation.x, t.rotation.y, t.rotation.z,
        t.scale.x, t.scale.y, t.scale.z,
    ] {
        data.extend_from_slice(&v.to_le_bytes());
    }
    Some(data)
}

fn deserialize_transform(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> {
    if data.len() < 36 {
        return Err("Transform data too short".into());
    }
    let f = |off: usize| f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
    let t = crate::Transform {
        position: voltex_math::Vec3::new(f(0), f(4), f(8)),
        rotation: voltex_math::Vec3::new(f(12), f(16), f(20)),
        scale: voltex_math::Vec3::new(f(24), f(28), f(32)),
    };
    world.add(entity, t);
    Ok(())
}

// ── Tag: UTF-8 string bytes ─────────────────────────────────────────

fn serialize_tag(world: &World, entity: Entity) -> Option<Vec<u8>> {
    let tag = world.get::<crate::scene::Tag>(entity)?;
    Some(tag.0.as_bytes().to_vec())
}

fn deserialize_tag(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> {
    let s = std::str::from_utf8(data).map_err(|_| crate::EcsError::from("Invalid UTF-8 in tag"))?;
    world.add(entity, crate::scene::Tag(s.to_string()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{World, Transform};
    use voltex_math::Vec3;

    #[test]
    fn test_register_and_find() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        assert!(registry.find("transform").is_some());
        assert!(registry.find("tag").is_some());
        assert!(registry.find("nonexistent").is_none());
    }

    #[test]
    fn test_entries_count() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        assert_eq!(registry.entries().len(), 2);
    }

    #[test]
    fn test_serialize_transform() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));

        let entry = registry.find("transform").unwrap();
        let data = (entry.serialize)(&world, e);
        assert!(data.is_some());
        assert_eq!(data.unwrap().len(), 36); // 9 f32s * 4 bytes
    }

    #[test]
    fn test_serialize_missing_component() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        // no Transform added

        let entry = registry.find("transform").unwrap();
        assert!((entry.serialize)(&world, e).is_none());
    }

    #[test]
    fn test_roundtrip_transform() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Vec3::new(0.1, 0.2, 0.3),
            scale: Vec3::new(4.0, 5.0, 6.0),
        });

        let entry = registry.find("transform").unwrap();
        let data = (entry.serialize)(&world, e).unwrap();

        let mut world2 = World::new();
        let e2 = world2.spawn();
        (entry.deserialize)(&mut world2, e2, &data).unwrap();

        let t = world2.get::<Transform>(e2).unwrap();
        assert!((t.position.x - 1.0).abs() < 1e-6);
        assert!((t.position.y - 2.0).abs() < 1e-6);
        assert!((t.position.z - 3.0).abs() < 1e-6);
        assert!((t.rotation.x - 0.1).abs() < 1e-6);
        assert!((t.scale.x - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_roundtrip_tag() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, crate::scene::Tag("hello world".to_string()));

        let entry = registry.find("tag").unwrap();
        let data = (entry.serialize)(&world, e).unwrap();

        let mut world2 = World::new();
        let e2 = world2.spawn();
        (entry.deserialize)(&mut world2, e2, &data).unwrap();

        let tag = world2.get::<crate::scene::Tag>(e2).unwrap();
        assert_eq!(tag.0, "hello world");
    }

    #[test]
    fn test_deserialize_transform_too_short() {
        let mut world = World::new();
        let e = world.spawn();
        let result = deserialize_transform(&mut world, e, &[0u8; 10]);
        assert!(result.is_err());
    }
}
