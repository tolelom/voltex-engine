//! Editor commands — the single dispatch entry point for every user/AI action.
//!
//! Every mutation of the editor's ECS world goes through `EditorCommand` so that
//! GUI clicks, Lua scripts, and AI-generated command streams produce identical
//! side-effects (state mutation + log entry + scene file sync).

use voltex_ecs::Entity;
use voltex_math::Vec3;

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
    SaveScene { path: String },
    LoadScene { path: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandError {
    EntityNotAlive(Entity),
    Io(String),
    Parse(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::EntityNotAlive(e) => write!(f, "entity not alive: id={} gen={}", e.id, e.generation),
            CommandError::Io(msg) => write!(f, "io: {}", msg),
            CommandError::Parse(msg) => write!(f, "parse: {}", msg),
        }
    }
}

impl std::error::Error for CommandError {}

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
) -> Result<Entity, CommandError> {
    match cmd {
        EditorCommand::Spawn => {
            let e = world.spawn();
            world.add(e, Transform::new());
            Ok(e)
        }
        EditorCommand::Despawn { entity } => {
            if !world.is_alive(*entity) {
                return Err(CommandError::EntityNotAlive(*entity));
            }
            world.despawn(*entity);
            Ok(*entity)
        }
        EditorCommand::SetTransform { entity, position, rotation, scale } => {
            if !world.is_alive(*entity) {
                return Err(CommandError::EntityNotAlive(*entity));
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
                return Err(CommandError::EntityNotAlive(*entity));
            }
            // Replace existing tag if any
            world.remove::<voltex_ecs::scene::Tag>(*entity);
            world.add(*entity, voltex_ecs::scene::Tag(tag.clone()));
            Ok(*entity)
        }
        EditorCommand::SaveScene { path } => {
            let json = voltex_ecs::serialize_scene_json(world, registry);
            std::fs::write(path, json).map_err(|e| CommandError::Io(e.to_string()))?;
            Ok(Entity { id: 0, generation: 0 })
        }
        EditorCommand::LoadScene { path } => {
            let text = std::fs::read_to_string(path).map_err(|e| CommandError::Io(e.to_string()))?;
            voltex_ecs::deserialize_scene_json(world, &text, registry)
                .map_err(CommandError::Parse)?;
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
        let err = CommandError::EntityNotAlive(Entity { id: 7, generation: 2 });
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
        assert_eq!(result, Err(CommandError::EntityNotAlive(entity)));
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
            Err(CommandError::EntityNotAlive(entity)),
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
            Err(CommandError::Parse(_)) => {}
            other => panic!("expected Parse error, got {:?}", other),
        }
        fs::remove_file(&tmp).ok();
    }
}
