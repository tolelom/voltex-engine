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
}
