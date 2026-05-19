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
    _registry: &ComponentRegistry,
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
        _ => Err(CommandError::Io("not implemented yet".into())),
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
}
