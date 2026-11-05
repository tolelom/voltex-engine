//! Binary scene format (.vscn binary).
//!
//! Format:
//!   Header: "VSCN" (4 bytes) + version u32 LE + entity_count u32 LE
//!   Per entity:
//!     parent_index i32 LE (-1 = no parent)
//!     component_count u32 LE
//!     Per component:
//!       name_len u16 LE + name bytes
//!       data_len u32 LE + data bytes

use crate::component_registry::ComponentRegistry;
use crate::entity::Entity;
use crate::hierarchy::{add_child, repair_hierarchy_cycles, Parent};
use crate::transform::Transform;
use crate::world::World;
use std::collections::HashMap;

const MAGIC: &[u8; 4] = b"VSCN";
const VERSION: u32 = 1;

/// Maximum entity count we accept in a single binary scene file. A header
/// declaring billions of entities used to drive a multi-GB
/// `Vec::with_capacity` allocation even when the body was tiny.
/// Mirrors `MAX_SCENE_ENTITIES` in `scene.rs`.
const MAX_SCENE_ENTITIES: usize = 1_000_000;

/// Serialize all entities with a Transform to the binary scene format.
pub fn serialize_scene_binary(world: &World, registry: &ComponentRegistry) -> Vec<u8> {
    let entities_with_transform: Vec<(Entity, Transform)> =
        world.query::<Transform>().map(|(e, t)| (e, *t)).collect();

    let entity_to_index: HashMap<Entity, usize> = entities_with_transform
        .iter()
        .enumerate()
        .map(|(i, (e, _))| (*e, i))
        .collect();

    let entity_count = entities_with_transform.len() as u32;
    let mut buf = Vec::new();

    // Header
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&entity_count.to_le_bytes());

    // Entities
    for (entity, _) in &entities_with_transform {
        // Parent index
        let parent_idx: i32 = if let Some(parent_comp) = world.get::<Parent>(*entity) {
            entity_to_index
                .get(&parent_comp.0)
                .map(|&i| i as i32)
                .unwrap_or(-1)
        } else {
            -1
        };
        buf.extend_from_slice(&parent_idx.to_le_bytes());

        // Collect serializable components
        let mut comp_data: Vec<(&str, Vec<u8>)> = Vec::new();
        for entry in registry.entries() {
            if let Some(data) = (entry.serialize)(world, *entity) {
                comp_data.push((&entry.name, data));
            }
        }

        let comp_count = comp_data.len() as u32;
        buf.extend_from_slice(&comp_count.to_le_bytes());

        for (name, data) in &comp_data {
            let name_bytes = name.as_bytes();
            buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(name_bytes);
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(data);
        }
    }

    buf
}

/// Deserialize entities from binary scene data.
pub fn deserialize_scene_binary(
    world: &mut World,
    data: &[u8],
    registry: &ComponentRegistry,
) -> crate::Result<Vec<Entity>> {
    if data.len() < 12 {
        return Err("Binary scene data too short".into());
    }

    // Verify magic
    if &data[0..4] != MAGIC {
        return Err("Invalid magic bytes — expected VSCN".into());
    }

    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version != 1 {
        return Err(format!("Unsupported binary scene version: {}", version).into());
    }

    let entity_count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
    if entity_count > MAX_SCENE_ENTITIES {
        return Err(format!(
            "Binary scene entity count {} exceeds limit {}",
            entity_count, MAX_SCENE_ENTITIES
        )
        .into());
    }
    let mut pos = 12;

    let mut created: Vec<Entity> = Vec::with_capacity(entity_count);
    let mut parent_indices: Vec<i32> = Vec::with_capacity(entity_count);

    for _ in 0..entity_count {
        // Parent index
        if pos + 4 > data.len() {
            return Err("Unexpected end of data reading parent index".into());
        }
        let parent_idx =
            i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        // Component count
        if pos + 4 > data.len() {
            return Err("Unexpected end of data reading component count".into());
        }
        let comp_count =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        let entity = world.spawn();

        for _ in 0..comp_count {
            // Name length
            if pos + 2 > data.len() {
                return Err("Unexpected end of data reading name length".into());
            }
            let name_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            pos += 2;

            // Name
            if pos + name_len > data.len() {
                return Err("Unexpected end of data reading component name".into());
            }
            let name = std::str::from_utf8(&data[pos..pos + name_len])
                .map_err(|_| crate::EcsError::from("Invalid UTF-8 in component name"))?;
            pos += name_len;

            // Data length
            if pos + 4 > data.len() {
                return Err("Unexpected end of data reading data length".into());
            }
            let data_len =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
            pos += 4;

            // Data
            if pos + data_len > data.len() {
                return Err("Unexpected end of data reading component data".into());
            }
            let comp_data = &data[pos..pos + data_len];
            pos += data_len;

            // Deserialize via registry
            if let Some(entry) = registry.find(name) {
                (entry.deserialize)(world, entity, comp_data)?;
            }
        }

        created.push(entity);
        parent_indices.push(parent_idx);
    }

    // Apply parent relationships
    for (child_idx, &parent_idx) in parent_indices.iter().enumerate() {
        if parent_idx >= 0 {
            let pi = parent_idx as usize;
            if pi < created.len() {
                let child_entity = created[child_idx];
                let parent_entity = created[pi];
                add_child(world, parent_entity, child_entity);
            }
        }
    }

    // Defense-in-depth: scan for cycles a custom-registered Parent component
    // (deserialized via the ComponentRegistry path, bypassing `add_child`)
    // could have introduced. No-op for well-formed scenes.
    let _ = repair_hierarchy_cycles(world);

    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Tag;
    use voltex_math::Vec3;

    #[test]
    fn test_binary_roundtrip() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(5.0, 0.0, -1.0)));
        world.add(e, Tag("enemy".into()));

        let data = serialize_scene_binary(&world, &registry);
        assert_eq!(&data[0..4], b"VSCN");

        let mut world2 = World::new();
        let entities = deserialize_scene_binary(&mut world2, &data, &registry).unwrap();
        assert_eq!(entities.len(), 1);
        let t = world2.get::<Transform>(entities[0]).unwrap();
        assert!((t.position.x - 5.0).abs() < 1e-6);
        assert!((t.position.z - (-1.0)).abs() < 1e-6);
        let tag = world2.get::<Tag>(entities[0]).unwrap();
        assert_eq!(tag.0, "enemy");
    }

    #[test]
    fn test_binary_with_hierarchy() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.add(a, Transform::new());
        world.add(b, Transform::new());
        add_child(&mut world, a, b);

        let data = serialize_scene_binary(&world, &registry);
        let mut world2 = World::new();
        let entities = deserialize_scene_binary(&mut world2, &data, &registry).unwrap();
        assert_eq!(entities.len(), 2);
        assert!(world2.get::<Parent>(entities[1]).is_some());
        let p = world2.get::<Parent>(entities[1]).unwrap();
        assert_eq!(p.0, entities[0]);
    }

    #[test]
    fn test_binary_invalid_magic() {
        let data = vec![0u8; 20];
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        assert!(deserialize_scene_binary(&mut world, &data, &registry).is_err());
    }

    #[test]
    fn test_binary_too_short() {
        let data = vec![0u8; 5];
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        assert!(deserialize_scene_binary(&mut world, &data, &registry).is_err());
    }

    #[test]
    fn test_binary_multiple_entities() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        for i in 0..3 {
            let e = world.spawn();
            world.add(e, Transform::from_position(Vec3::new(i as f32, 0.0, 0.0)));
            world.add(e, Tag(format!("e{}", i)));
        }

        let data = serialize_scene_binary(&world, &registry);
        let mut world2 = World::new();
        let entities = deserialize_scene_binary(&mut world2, &data, &registry).unwrap();
        assert_eq!(entities.len(), 3);

        for (i, &e) in entities.iter().enumerate() {
            let t = world2.get::<Transform>(e).unwrap();
            assert!((t.position.x - i as f32).abs() < 1e-6);
            let tag = world2.get::<Tag>(e).unwrap();
            assert_eq!(tag.0, format!("e{}", i));
        }
    }

    #[test]
    fn deserialize_binary_scene_rejects_huge_entity_count() {
        // Header claims u32::MAX entities. Without the cap this drove a
        // multi-GB `Vec::with_capacity` allocation even though the file body
        // was tiny.
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.extend_from_slice(&VERSION.to_le_bytes());
        data.extend_from_slice(&u32::MAX.to_le_bytes());
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        let err = deserialize_scene_binary(&mut world, &data, &registry)
            .unwrap_err()
            .to_string();
        assert!(err.contains("exceeds limit"), "unexpected error: {err}");
    }
}
