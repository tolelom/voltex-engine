//! EditorSession owns the editor's mutable state: the ECS world, the component
//! registry, the currently selected entity, the path of the active scene file,
//! and the append-only command log. Every command flows through `dispatch`,
//! which mutates the world, appends a jsonl log line, and (for non-Load commands)
//! re-saves the active scene file so that a watcher / AI / human reading from
//! disk always sees the latest state.

use crate::command::{apply_command, EditorCommand};
use crate::error::EditorError;
use std::fs::OpenOptions;
use std::io::Write;
use voltex_ecs::{ComponentRegistry, Entity, World};

pub struct EditorSession {
    pub world: World,
    pub registry: ComponentRegistry,
    pub selected: Option<Entity>,
    pub scene_path: String,
    pub log_path: String,
}

impl EditorSession {
    /// Create a session bound to a scene file path. The log path defaults to
    /// `<scene_path>.log` so each scene has its own append-only history.
    pub fn new(scene_path: impl Into<String>) -> Self {
        let scene_path = scene_path.into();
        let log_path = format!("{}.log", scene_path);
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        Self {
            world: World::new(),
            registry,
            selected: None,
            scene_path,
            log_path,
        }
    }

    /// Execute a command, append a jsonl log entry, and (for state-mutating
    /// commands) re-save the scene to disk so the file is always current.
    pub fn dispatch(&mut self, cmd: EditorCommand) -> Result<Entity, EditorError> {
        let result = apply_command(&mut self.world, &self.registry, &cmd);
        self.append_log(&cmd, &result);

        // Auto-sync scene file on every state mutation except LoadScene (which
        // already read from disk) and SaveScene (already wrote).
        let is_save_or_load = matches!(
            cmd,
            EditorCommand::SaveScene { .. } | EditorCommand::LoadScene { .. }
        );
        if result.is_ok() && !is_save_or_load {
            let json = voltex_ecs::serialize_scene_json(&self.world, &self.registry);
            std::fs::write(&self.scene_path, json)
                .map_err(|e| EditorError::Io(e.to_string()))?;
        }
        result
    }

    fn append_log(&self, cmd: &EditorCommand, result: &Result<Entity, EditorError>) {
        let outcome = match result {
            Ok(e) => format!("{{\"ok\":{{\"id\":{},\"generation\":{}}}}}", e.id, e.generation),
            Err(e) => format!("{{\"err\":{:?}}}", format!("{}", e)),
        };
        let line = format!("{{\"cmd\":{:?},\"result\":{}}}\n", format!("{:?}", cmd), outcome);
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.log_path) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_math::Vec3;

    fn temp_path(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("{}_{}", name, std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn dispatch_spawn_writes_scene_to_disk() {
        let scene = temp_path("voltex_session_spawn.vscn");
        let log = format!("{}.log", scene);
        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(&log);

        let mut session = EditorSession::new(scene.clone());
        session.dispatch(EditorCommand::Spawn).unwrap();

        let on_disk = std::fs::read_to_string(&scene).expect("scene file written");
        assert!(on_disk.contains("\"entities\""));

        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(&log);
    }

    #[test]
    fn dispatch_appends_to_log() {
        let scene = temp_path("voltex_session_log.vscn");
        let log = format!("{}.log", scene);
        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(&log);

        let mut session = EditorSession::new(scene.clone());
        let e = session.dispatch(EditorCommand::Spawn).unwrap();
        session
            .dispatch(EditorCommand::SetTransform {
                entity: e,
                position: Vec3::new(1.0, 2.0, 3.0),
                rotation: Vec3::ZERO,
                scale: Vec3::new(1.0, 1.0, 1.0),
            })
            .unwrap();

        let log_text = std::fs::read_to_string(&log).expect("log written");
        let lines: Vec<&str> = log_text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("Spawn"));
        assert!(lines[1].contains("SetTransform"));

        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(&log);
    }
}
