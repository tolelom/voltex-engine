//! Editor commands — the single dispatch entry point for every user/AI action.
//!
//! Every mutation of the editor's ECS world goes through `EditorCommand` so that
//! GUI clicks, Lua scripts, and AI-generated command streams produce identical
//! side-effects (state mutation + log entry + scene file sync).

use voltex_ecs::Entity;
use voltex_math::Vec3;

use crate::error::EditorError;

#[derive(Debug, Clone, PartialEq)]
pub enum EditorCommand {
    Spawn,
    Despawn { entity: Entity },
    SetTransform {
        entity: Entity,
        position: Vec3,
        rotation: Vec3,
        scale: Vec3,
    },
    SetTag { entity: Entity, tag: String },
    /// Attach `child` under `parent` (sets Parent on child, adds to Children).
    /// No-op (still Ok) if the relationship already exists. Fails if either
    /// entity is not alive.
    AddChild { parent: Entity, child: Entity },
    /// Detach `child` from `parent` (clears Parent on child, removes from
    /// parent's Children). No-op (still Ok) if not currently a child.
    RemoveChild { parent: Entity, child: Entity },
    SaveScene { path: String },
    LoadScene { path: String },
}

use voltex_ecs::{World, Transform};
use voltex_ecs::ComponentRegistry;

/// Apply a command to the world. Returns the entity affected (for commands that
/// create or target an entity) or `Entity { id: 0, generation: 0 }` for whole-scene
/// commands like SaveScene/LoadScene.
///
/// The caller is responsible for logging and persistence — this function only
/// mutates the world.
pub fn apply_command(
    world: &mut World,
    registry: &ComponentRegistry,
    cmd: &EditorCommand,
) -> Result<Entity, EditorError> {
    match cmd {
        EditorCommand::Spawn => {
            let e = world.spawn();
            world.add(e, Transform::new());
            Ok(e)
        }
        EditorCommand::Despawn { entity } => {
            if !world.is_alive(*entity) {
                return Err(EditorError::EntityNotAlive(*entity));
            }
            world.despawn(*entity);
            Ok(*entity)
        }
        EditorCommand::SetTransform { entity, position, rotation, scale } => {
            if !world.is_alive(*entity) {
                return Err(EditorError::EntityNotAlive(*entity));
            }
            if let Some(t) = world.get_mut::<Transform>(*entity) {
                t.position = *position;
                t.rotation = *rotation;
                t.scale = *scale;
            } else {
                world.add(*entity, Transform { position: *position, rotation: *rotation, scale: *scale });
            }
            Ok(*entity)
        }
        EditorCommand::SetTag { entity, tag } => {
            if !world.is_alive(*entity) {
                return Err(EditorError::EntityNotAlive(*entity));
            }
            // Replace existing tag if any
            world.remove::<voltex_ecs::scene::Tag>(*entity);
            world.add(*entity, voltex_ecs::scene::Tag(tag.clone()));
            Ok(*entity)
        }
        EditorCommand::AddChild { parent, child } => {
            if !world.is_alive(*parent) {
                return Err(EditorError::EntityNotAlive(*parent));
            }
            if !world.is_alive(*child) {
                return Err(EditorError::EntityNotAlive(*child));
            }
            voltex_ecs::hierarchy::add_child(world, *parent, *child);
            Ok(*child)
        }
        EditorCommand::RemoveChild { parent, child } => {
            if !world.is_alive(*parent) {
                return Err(EditorError::EntityNotAlive(*parent));
            }
            if !world.is_alive(*child) {
                return Err(EditorError::EntityNotAlive(*child));
            }
            voltex_ecs::hierarchy::remove_child(world, *parent, *child);
            Ok(*child)
        }
        EditorCommand::SaveScene { path } => {
            let json = voltex_ecs::serialize_scene_json(world, registry);
            std::fs::write(path, json).map_err(|e| EditorError::Io(e.to_string()))?;
            Ok(Entity { id: 0, generation: 0 })
        }
        EditorCommand::LoadScene { path } => {
            // Editor semantics: Load == "disk is the source of truth", so wipe
            // the in-memory world before deserializing. Library callers that
            // want append-style behavior call deserialize_scene_json directly.
            let text = std::fs::read_to_string(path).map_err(|e| EditorError::Io(e.to_string()))?;
            world.clear();
            voltex_ecs::deserialize_scene_json(world, &text, registry)
                .map_err(|e| EditorError::Parse(e.to_string()))?;
            Ok(Entity { id: 0, generation: 0 })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_command_debug_includes_variant_name() {
        let cmd = EditorCommand::Spawn;
        let s = format!("{:?}", cmd);
        assert!(s.contains("Spawn"));
    }

    #[test]
    fn command_error_display_entity_not_alive() {
        let err = EditorError::EntityNotAlive(Entity { id: 7, generation: 2 });
        let s = format!("{}", err);
        assert!(s.contains("id=7"));
        assert!(s.contains("gen=2"));
    }

    use voltex_ecs::{World, Transform};
    use voltex_ecs::ComponentRegistry;

    #[test]
    fn spawn_creates_entity_with_default_transform() {
        let mut world = World::new();
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();

        let result = apply_command(&mut world, &registry, &EditorCommand::Spawn);
        let entity = result.expect("spawn returned Err");
        assert!(world.is_alive(entity));
        assert!(world.get::<Transform>(entity).is_some(), "Spawn must attach default Transform");
    }

    #[test]
    fn despawn_returns_err_when_already_dead() {
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        let entity = world.spawn();
        world.despawn(entity);
        let result = apply_command(&mut world, &registry, &EditorCommand::Despawn { entity });
        assert_eq!(result, Err(EditorError::EntityNotAlive(entity)));
    }

    #[test]
    fn despawn_alive_entity_succeeds_and_removes_it() {
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        let entity = world.spawn();
        let result = apply_command(&mut world, &registry, &EditorCommand::Despawn { entity });
        assert!(result.is_ok());
        assert!(!world.is_alive(entity));
    }

    #[test]
    fn set_transform_updates_existing_component() {
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        let entity = world.spawn();
        world.add(entity, Transform::new());

        let cmd = EditorCommand::SetTransform {
            entity,
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Vec3::new(0.0, 0.5, 0.0),
            scale: Vec3::new(2.0, 2.0, 2.0),
        };
        apply_command(&mut world, &registry, &cmd).expect("ok");
        let t = world.get::<Transform>(entity).unwrap();
        assert_eq!(t.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(t.rotation, Vec3::new(0.0, 0.5, 0.0));
        assert_eq!(t.scale, Vec3::new(2.0, 2.0, 2.0));
    }

    #[test]
    fn set_transform_fails_when_entity_dead() {
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        let entity = world.spawn();
        world.despawn(entity);
        let cmd = EditorCommand::SetTransform {
            entity,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::new(1.0, 1.0, 1.0),
        };
        assert_eq!(
            apply_command(&mut world, &registry, &cmd),
            Err(EditorError::EntityNotAlive(entity)),
        );
    }

    #[test]
    fn set_tag_adds_or_replaces_tag() {
        use voltex_ecs::scene::Tag;
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        let entity = world.spawn();
        apply_command(&mut world, &registry, &EditorCommand::SetTag { entity, tag: "Player".into() }).unwrap();
        assert_eq!(world.get::<Tag>(entity).unwrap().0, "Player");

        apply_command(&mut world, &registry, &EditorCommand::SetTag { entity, tag: "Enemy".into() }).unwrap();
        assert_eq!(world.get::<Tag>(entity).unwrap().0, "Enemy");
    }

    #[test]
    fn save_then_load_roundtrips_a_scene() {
        use std::fs;
        let mut world1 = World::new();
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();

        let tmp = std::env::temp_dir().join(format!(
            "voltex_editor_test_scene_roundtrip_{}.vscn", std::process::id()
        ));
        let _ = std::fs::remove_file(&tmp);

        let e1 = apply_command(&mut world1, &registry, &EditorCommand::Spawn).unwrap();
        apply_command(&mut world1, &registry, &EditorCommand::SetTransform {
            entity: e1,
            position: Vec3::new(7.0, 8.0, 9.0),
            rotation: Vec3::ZERO,
            scale: Vec3::new(1.0, 1.0, 1.0),
        }).unwrap();
        apply_command(&mut world1, &registry, &EditorCommand::SetTag {
            entity: e1, tag: "Hero".into(),
        }).unwrap();

        let tmp_str = tmp.to_string_lossy().into_owned();

        apply_command(&mut world1, &registry, &EditorCommand::SaveScene { path: tmp_str.clone() })
            .expect("save");

        let mut world2 = World::new();
        apply_command(&mut world2, &registry, &EditorCommand::LoadScene { path: tmp_str.clone() })
            .expect("load");

        let entities: Vec<_> = world2.query::<Transform>().map(|(e, _)| e).collect();
        assert_eq!(entities.len(), 1);
        let loaded_t = world2.get::<Transform>(entities[0]).unwrap();
        assert_eq!(loaded_t.position, Vec3::new(7.0, 8.0, 9.0));
        let loaded_tag = world2.get::<voltex_ecs::scene::Tag>(entities[0]).unwrap();
        assert_eq!(loaded_tag.0, "Hero");

        fs::remove_file(&tmp).ok();
    }

    #[test]
    fn add_child_attaches_and_remove_child_detaches() {
        use voltex_ecs::hierarchy::{Children, Parent};
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        let p = world.spawn();
        let c = world.spawn();

        apply_command(&mut world, &registry, &EditorCommand::AddChild { parent: p, child: c }).unwrap();
        assert_eq!(world.get::<Parent>(c).map(|x| x.0), Some(p));
        assert_eq!(world.get::<Children>(p).map(|x| x.0.clone()), Some(vec![c]));

        apply_command(&mut world, &registry, &EditorCommand::RemoveChild { parent: p, child: c }).unwrap();
        assert!(world.get::<Parent>(c).is_none());
        assert!(world.get::<Children>(p).unwrap().0.is_empty());
    }

    #[test]
    fn add_child_fails_when_parent_dead() {
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        let p = world.spawn();
        let c = world.spawn();
        world.despawn(p);
        let result = apply_command(&mut world, &registry, &EditorCommand::AddChild { parent: p, child: c });
        assert!(matches!(result, Err(EditorError::EntityNotAlive(_))));
    }

    #[test]
    fn load_scene_clears_world_before_deserializing() {
        // Setup: world A has 1 entity, write it to disk.
        let mut world_a = World::new();
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();

        let tmp = std::env::temp_dir().join(format!(
            "voltex_editor_load_clears_{}.vscn", std::process::id()
        ));
        let _ = std::fs::remove_file(&tmp);
        let tmp_str = tmp.to_string_lossy().into_owned();

        let saved = apply_command(&mut world_a, &registry, &EditorCommand::Spawn).unwrap();
        apply_command(&mut world_a, &registry, &EditorCommand::SetTag {
            entity: saved, tag: "FromDisk".into(),
        }).unwrap();
        apply_command(&mut world_a, &registry, &EditorCommand::SaveScene { path: tmp_str.clone() }).unwrap();

        // World B is pre-seeded with 2 entities BEFORE Load.
        let mut world_b = World::new();
        let seed1 = world_b.spawn();
        world_b.add(seed1, Transform::new());
        let seed2 = world_b.spawn();
        world_b.add(seed2, Transform::new());
        assert_eq!(world_b.entity_count(), 2);

        // Load — should wipe the 2 seeds and end up with only 1 (from disk).
        apply_command(&mut world_b, &registry, &EditorCommand::LoadScene { path: tmp_str.clone() }).unwrap();
        assert_eq!(world_b.entity_count(), 1);
        // The remaining entity is the loaded one (tag "FromDisk").
        let tagged: Vec<_> = world_b
            .query::<voltex_ecs::scene::Tag>()
            .filter(|(_, t)| t.0 == "FromDisk")
            .collect();
        assert_eq!(tagged.len(), 1);

        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn load_returns_parse_error_on_garbage() {
        use std::fs;
        let mut world = World::new();
        let registry = ComponentRegistry::new();
        let tmp = std::env::temp_dir().join(format!(
            "voltex_editor_test_garbage_{}.vscn", std::process::id()
        ));
        let _ = std::fs::remove_file(&tmp);
        fs::write(&tmp, "{not json").unwrap();
        let result = apply_command(&mut world, &registry, &EditorCommand::LoadScene {
            path: tmp.to_string_lossy().into_owned(),
        });
        match result {
            Err(EditorError::Parse(_)) => {}
            other => panic!("expected Parse error, got {:?}", other),
        }
        fs::remove_file(&tmp).ok();
    }
}
