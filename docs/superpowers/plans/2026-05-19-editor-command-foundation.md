# Editor Command Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `voltex_editor`를 위젯 라이브러리에서 **씬을 편집할 수 있는 에디터**로 격상한다. 모든 사용자 액션은 명명된 `EditorCommand`를 거치고, 모든 상태는 `World`에 살며 `.vscn` JSON 파일로 영속화된다. GUI와 AI가 동일한 파일·동일한 커맨드 인터페이스를 공유한다.

**Architecture:** `voltex_editor`가 `voltex_ecs`에 의존하게 만들고 `command` / `session` / `panels` 세 개의 모듈을 추가한다. `EditorSession`이 `World` + `ComponentRegistry` + 현재 씬 경로 + 커맨드 로그를 소유한다. `EditorCommand` enum (Spawn / Despawn / SetTransform / SetTag / SaveScene / LoadScene)을 dispatch 함수로 처리하고, 매 dispatch마다 jsonl 로그를 append + 현재 씬을 디스크에 즉시 sync 한다. 새 panel 위젯 `hierarchy_panel` (엔티티 리스트 + 선택) 과 `inspector_panel` (선택 엔티티의 Transform/Tag 편집)을 추가하고 `examples/editor_demo`를 재작성해 이를 사용한다.

**Tech Stack:** Rust (edition 2021), voltex_ecs (World, Entity, Transform, Tag, ComponentRegistry, serialize_scene_json/deserialize_scene_json), voltex_math (Vec3), voltex_editor (UiContext + 기존 widgets), winit 0.30 + wgpu 28.0 (editor_demo 윈도우).

---

## File Structure

- **Create**: `crates/voltex_editor/src/command.rs` — `EditorCommand` enum, `CommandError`, `apply_command()`
- **Create**: `crates/voltex_editor/src/session.rs` — `EditorSession` struct
- **Create**: `crates/voltex_editor/src/panels/mod.rs` — `pub mod hierarchy; pub mod inspector;`
- **Create**: `crates/voltex_editor/src/panels/hierarchy.rs` — `hierarchy_panel(ui, session, ...)`
- **Create**: `crates/voltex_editor/src/panels/inspector.rs` — `inspector_panel(ui, session, ...)`
- **Modify**: `crates/voltex_editor/Cargo.toml` — add `voltex_ecs.workspace = true`, `voltex_math.workspace = true`
- **Modify**: `crates/voltex_editor/src/lib.rs` — `pub mod command; pub mod session; pub mod panels;` + re-exports
- **Modify**: `examples/editor_demo/Cargo.toml` — add `voltex_ecs.workspace = true`, `voltex_math.workspace = true`
- **Rewrite**: `examples/editor_demo/src/main.rs` — replace widget showcase with scene editor using `EditorSession`

각 task가 끝날 때마다 `cargo test --workspace --lib` 가 통과해야 한다.

---

## Task 1: Wire voltex_ecs into voltex_editor

**Files:**
- Modify: `crates/voltex_editor/Cargo.toml`

- [ ] **Step 1: Add voltex_ecs and voltex_math as dependencies**

`crates/voltex_editor/Cargo.toml` 의 `[dependencies]` 섹션을 다음과 같이 만든다:

```toml
[dependencies]
bytemuck = { workspace = true }
wgpu = { workspace = true }
voltex_ecs = { workspace = true }
voltex_math = { workspace = true }
```

- [ ] **Step 2: Verify the workspace still builds**

Run: `cargo build -p voltex_editor 2>&1 | tail -5`
Expected: `Finished` line, no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/voltex_editor/Cargo.toml
git commit -m "feat(editor): depend on voltex_ecs/voltex_math for scene state"
```

---

## Task 2: Define EditorCommand enum + CommandError

**Files:**
- Create: `crates/voltex_editor/src/command.rs`
- Modify: `crates/voltex_editor/src/lib.rs`
- Test: in-file `#[cfg(test)] mod tests` in `command.rs`

- [ ] **Step 1: Add module declaration to lib.rs**

`crates/voltex_editor/src/lib.rs` 맨 위 모듈 선언 블록에 다음을 추가한다:

```rust
pub mod command;
```

그리고 파일 맨 아래에 다음 re-export를 추가:

```rust
pub use command::{EditorCommand, CommandError};
```

- [ ] **Step 2: Write the failing test for enum construction and Debug format**

`crates/voltex_editor/src/command.rs` 를 다음 내용으로 새로 만든다:

```rust
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
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test -p voltex_editor --lib command:: 2>&1 | tail -15`
Expected: `test result: ok. 2 passed`

- [ ] **Step 4: Commit**

```bash
git add crates/voltex_editor/src/command.rs crates/voltex_editor/src/lib.rs
git commit -m "feat(editor): introduce EditorCommand enum and CommandError"
```

---

## Task 3: Implement apply_command — Spawn/Despawn

**Files:**
- Modify: `crates/voltex_editor/src/command.rs`

- [ ] **Step 1: Write failing tests for Spawn and Despawn**

`crates/voltex_editor/src/command.rs` 의 `#[cfg(test)] mod tests` 안에 다음 두 테스트를 추가한다 (기존 두 테스트 뒤):

```rust
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
```

- [ ] **Step 2: Run tests to confirm they fail with "function not found"**

Run: `cargo test -p voltex_editor --lib command:: 2>&1 | tail -15`
Expected: compile error `cannot find function apply_command in this scope`.

- [ ] **Step 3: Implement apply_command for Spawn and Despawn**

`crates/voltex_editor/src/command.rs` 의 `impl std::error::Error for CommandError {}` 줄 바로 뒤에 다음 함수를 추가한다:

```rust
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
```

Transform이 `new()` 생성자가 있는지 확인. `crates/voltex_ecs/src/transform.rs` 첫 20줄을 읽고 없으면 `Transform { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::new(1.0,1.0,1.0) }` 로 바꾼다. (이미 존재함이 확인됨 → `Transform::new()` 사용.)

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p voltex_editor --lib command:: 2>&1 | tail -15`
Expected: `test result: ok. 5 passed`

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_editor/src/command.rs
git commit -m "feat(editor): apply_command handles Spawn and Despawn"
```

---

## Task 4: apply_command — SetTransform and SetTag

**Files:**
- Modify: `crates/voltex_editor/src/command.rs`

- [ ] **Step 1: Write failing tests**

`#[cfg(test)] mod tests` 안에 다음 테스트를 추가:

```rust
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
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test -p voltex_editor --lib command:: 2>&1 | tail -15`
Expected: `test result: FAILED. 5 passed; 3 failed` — the new commands return the "not implemented" Err.

- [ ] **Step 3: Implement the two arms**

`apply_command` 의 `_ => Err(CommandError::Io("not implemented yet".into()))` 직전에 다음 두 arm을 삽입:

```rust
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
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p voltex_editor --lib command:: 2>&1 | tail -15`
Expected: `test result: ok. 8 passed`

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_editor/src/command.rs
git commit -m "feat(editor): SetTransform and SetTag commands"
```

---

## Task 5: apply_command — SaveScene and LoadScene

**Files:**
- Modify: `crates/voltex_editor/src/command.rs`

- [ ] **Step 1: Write failing test for round-trip save/load**

`#[cfg(test)] mod tests` 안에 추가:

```rust
    #[test]
    fn save_then_load_roundtrips_a_scene() {
        use std::fs;
        let mut world1 = World::new();
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();

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

        let tmp = std::env::temp_dir().join("voltex_editor_test_scene.vscn");
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
        let tmp = std::env::temp_dir().join("voltex_editor_test_bad.vscn");
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
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p voltex_editor --lib command:: 2>&1 | tail -15`
Expected: `2 failed` — the catch-all arm still returns the placeholder Err.

- [ ] **Step 3: Implement SaveScene and LoadScene arms**

`apply_command` 의 `_ => Err(CommandError::Io("not implemented yet".into()))` 를 제거하고 그 자리에 다음 두 arm을 추가 (이전 arm 뒤, match의 마지막에):

```rust
        EditorCommand::SaveScene { path } => {
            let json = voltex_ecs::serialize_scene_json(world, _registry);
            std::fs::write(path, json).map_err(|e| CommandError::Io(e.to_string()))?;
            Ok(Entity { id: 0, generation: 0 })
        }
        EditorCommand::LoadScene { path } => {
            let text = std::fs::read_to_string(path).map_err(|e| CommandError::Io(e.to_string()))?;
            voltex_ecs::deserialize_scene_json(world, &text, _registry)
                .map_err(CommandError::Parse)?;
            Ok(Entity { id: 0, generation: 0 })
        }
```

이제 `_registry` 인자가 실제로 사용되므로 시그니처의 `_registry` 를 `registry` 로 바꾸고, 위 두 arm의 `_registry` 도 모두 `registry` 로 바꾼다.

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p voltex_editor --lib command:: 2>&1 | tail -15`
Expected: `test result: ok. 10 passed`

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_editor/src/command.rs
git commit -m "feat(editor): SaveScene and LoadScene commands write .vscn JSON"
```

---

## Task 6: Create EditorSession with command log

**Files:**
- Create: `crates/voltex_editor/src/session.rs`
- Modify: `crates/voltex_editor/src/lib.rs`

- [ ] **Step 1: Add module + re-export in lib.rs**

`crates/voltex_editor/src/lib.rs` 의 `pub mod command;` 아래 한 줄 추가:

```rust
pub mod session;
```

그리고 re-exports 영역에 추가:

```rust
pub use session::EditorSession;
```

- [ ] **Step 2: Write the file with failing test**

`crates/voltex_editor/src/session.rs` 를 다음 내용으로 새로 만든다:

```rust
//! EditorSession owns the editor's mutable state: the ECS world, the component
//! registry, the currently selected entity, the path of the active scene file,
//! and the append-only command log. Every command flows through `dispatch`,
//! which mutates the world, appends a jsonl log line, and (for non-Load commands)
//! re-saves the active scene file so that a watcher / AI / human reading from
//! disk always sees the latest state.

use crate::command::{apply_command, CommandError, EditorCommand};
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
    pub fn dispatch(&mut self, cmd: EditorCommand) -> Result<Entity, CommandError> {
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
                .map_err(|e| CommandError::Io(e.to_string()))?;
        }
        result
    }

    fn append_log(&self, cmd: &EditorCommand, result: &Result<Entity, CommandError>) {
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
            .join(name)
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
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p voltex_editor --lib session:: 2>&1 | tail -15`
Expected: `test result: ok. 2 passed`

- [ ] **Step 4: Commit**

```bash
git add crates/voltex_editor/src/session.rs crates/voltex_editor/src/lib.rs
git commit -m "feat(editor): EditorSession with auto-sync and command log"
```

---

## Task 7: Panels module skeleton

**Files:**
- Create: `crates/voltex_editor/src/panels/mod.rs`
- Create: `crates/voltex_editor/src/panels/hierarchy.rs` (empty stub)
- Create: `crates/voltex_editor/src/panels/inspector.rs` (empty stub)
- Modify: `crates/voltex_editor/src/lib.rs`

- [ ] **Step 1: Add panels module to lib.rs**

`crates/voltex_editor/src/lib.rs` 에 한 줄 추가:

```rust
pub mod panels;
```

- [ ] **Step 2: Create panels/mod.rs**

`crates/voltex_editor/src/panels/mod.rs`:

```rust
pub mod hierarchy;
pub mod inspector;
```

- [ ] **Step 3: Create empty hierarchy.rs**

`crates/voltex_editor/src/panels/hierarchy.rs`:

```rust
//! Hierarchy panel — scrollable list of entities, click to select.
```

- [ ] **Step 4: Create empty inspector.rs**

`crates/voltex_editor/src/panels/inspector.rs`:

```rust
//! Inspector panel — view/edit components of the selected entity.
```

- [ ] **Step 5: Verify build**

Run: `cargo build -p voltex_editor 2>&1 | tail -5`
Expected: `Finished`

- [ ] **Step 6: Commit**

```bash
git add crates/voltex_editor/src/lib.rs crates/voltex_editor/src/panels
git commit -m "feat(editor): add panels module skeleton"
```

---

## Task 8: hierarchy_panel widget

**Files:**
- Modify: `crates/voltex_editor/src/panels/hierarchy.rs`

- [ ] **Step 1: Write the panel function**

`crates/voltex_editor/src/panels/hierarchy.rs` 의 내용을 다음으로 교체:

```rust
//! Hierarchy panel — scrollable list of entities, click to select.

use crate::session::EditorSession;
use crate::ui_context::UiContext;
use voltex_ecs::{Entity, Transform};

const ROW_HEIGHT: f32 = 20.0;

/// Draw a hierarchy panel at (x, y) of size (w, h).
///
/// Lists every entity that has a Transform component. Clicking a row sets
/// `session.selected`. Returns true if the selection changed this frame.
pub fn hierarchy_panel(
    ui: &mut UiContext,
    session: &mut EditorSession,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> bool {
    ui.begin_panel("Hierarchy", x, y, w, h);

    let entities: Vec<Entity> = session.world.query::<Transform>().map(|(e, _)| e).collect();
    let mut selection_changed = false;

    let content_height = entities.len() as f32 * ROW_HEIGHT;
    let scroll_id: u32 = 0x1E_5C_00_01;
    ui.begin_scroll_panel(scroll_id, x + 4.0, y + 24.0, w - 8.0, h - 28.0, content_height);

    for entity in &entities {
        let label = match session.world.get::<voltex_ecs::scene::Tag>(*entity) {
            Some(tag) => format!("[{}] {}", entity.id, tag.0),
            None => format!("[{}] (untagged)", entity.id),
        };
        let is_selected = session.selected == Some(*entity);
        let display = if is_selected {
            format!("* {}", label)
        } else {
            format!("  {}", label)
        };
        if ui.button(&display) {
            session.selected = Some(*entity);
            selection_changed = true;
        }
    }

    ui.end_scroll_panel();
    ui.end_panel();
    selection_changed
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build -p voltex_editor 2>&1 | tail -10`
Expected: `Finished`. 만약 `voltex_ecs::scene::Tag` 경로 에러가 나면 `voltex_ecs::Tag`(prelude에 re-export 되어있으면) 또는 위 경로가 맞는지 확인 후 적절히 수정.

- [ ] **Step 3: Add a smoke test**

`crates/voltex_editor/src/panels/hierarchy.rs` 맨 아래에 추가:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::UiContext;
    use voltex_ecs::Transform;

    #[test]
    fn hierarchy_runs_without_panic_on_empty_world() {
        let mut ui = UiContext::new(800.0, 600.0);
        let mut session = EditorSession::new(
            std::env::temp_dir()
                .join("voltex_hier_test.vscn")
                .to_string_lossy()
                .into_owned(),
        );
        ui.begin_frame(0.0, 0.0, false);
        let changed = hierarchy_panel(&mut ui, &mut session, 0.0, 0.0, 200.0, 400.0);
        ui.end_frame();
        assert!(!changed);
    }

    #[test]
    fn hierarchy_lists_spawned_entities() {
        let mut ui = UiContext::new(800.0, 600.0);
        let scene = std::env::temp_dir()
            .join("voltex_hier_list.vscn")
            .to_string_lossy()
            .into_owned();
        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", scene));

        let mut session = EditorSession::new(scene.clone());
        for _ in 0..3 {
            session.dispatch(crate::command::EditorCommand::Spawn).unwrap();
        }

        ui.begin_frame(0.0, 0.0, false);
        hierarchy_panel(&mut ui, &mut session, 0.0, 0.0, 200.0, 400.0);
        ui.end_frame();

        // 3 entities × 1 button per row → at least 3 draw commands of text type;
        // we mainly check no panic and queries match world state.
        let count = session.world.query::<Transform>().count();
        assert_eq!(count, 3);

        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", scene));
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p voltex_editor --lib panels::hierarchy 2>&1 | tail -10`
Expected: `test result: ok. 2 passed`

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_editor/src/panels/hierarchy.rs
git commit -m "feat(editor): hierarchy_panel widget lists entities with selection"
```

---

## Task 9: inspector_panel widget

**Files:**
- Modify: `crates/voltex_editor/src/panels/inspector.rs`

- [ ] **Step 1: Implement inspector_panel**

`crates/voltex_editor/src/panels/inspector.rs` 내용을 다음으로 교체:

```rust
//! Inspector panel — view/edit components of the selected entity.

use crate::command::EditorCommand;
use crate::session::EditorSession;
use crate::ui_context::UiContext;
use voltex_ecs::Transform;
use voltex_math::Vec3;

/// Persistent buffers for the inspector's text inputs. The inspector is stateless
/// across frames except for these strings (the source of truth is the World).
#[derive(Default)]
pub struct InspectorState {
    pub px: String,
    pub py: String,
    pub pz: String,
    pub rx: String,
    pub ry: String,
    pub rz: String,
    pub sx: String,
    pub sy: String,
    pub sz: String,
    pub tag: String,
    pub last_entity_id: Option<u32>,
}

impl InspectorState {
    /// Refresh the buffers from the world if the selected entity changed since
    /// last frame. Idempotent if selection is stable.
    fn sync_from(&mut self, session: &EditorSession) {
        let Some(entity) = session.selected else {
            self.last_entity_id = None;
            return;
        };
        if self.last_entity_id == Some(entity.id) {
            return;
        }
        self.last_entity_id = Some(entity.id);

        if let Some(t) = session.world.get::<Transform>(entity) {
            self.px = format!("{}", t.position.x);
            self.py = format!("{}", t.position.y);
            self.pz = format!("{}", t.position.z);
            self.rx = format!("{}", t.rotation.x);
            self.ry = format!("{}", t.rotation.y);
            self.rz = format!("{}", t.rotation.z);
            self.sx = format!("{}", t.scale.x);
            self.sy = format!("{}", t.scale.y);
            self.sz = format!("{}", t.scale.z);
        }
        self.tag = session
            .world
            .get::<voltex_ecs::scene::Tag>(entity)
            .map(|t| t.0.clone())
            .unwrap_or_default();
    }

    fn parsed_vec(&self, x: &str, y: &str, z: &str) -> Option<Vec3> {
        let x = x.parse::<f32>().ok()?;
        let y = y.parse::<f32>().ok()?;
        let z = z.parse::<f32>().ok()?;
        Some(Vec3::new(x, y, z))
    }
}

/// Draw the inspector panel. When any text_input commits a parseable change,
/// dispatch a SetTransform or SetTag command.
pub fn inspector_panel(
    ui: &mut UiContext,
    session: &mut EditorSession,
    state: &mut InspectorState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    state.sync_from(session);
    ui.begin_panel("Inspector", x, y, w, h);

    let Some(entity) = session.selected else {
        ui.text("(no entity selected)");
        ui.end_panel();
        return;
    };

    ui.text(&format!("Entity id={} gen={}", entity.id, entity.generation));

    let mut tag_changed = false;
    let mut transform_changed = false;
    let row_y = |i: f32| y + 48.0 + i * 22.0;

    if ui.text_input(0x1_5A_00_01, &mut state.tag, x + 6.0, row_y(0.0), w - 12.0) {
        tag_changed = true;
    }

    ui.text("position x/y/z:");
    if ui.text_input(0x1_5A_01_01, &mut state.px, x + 6.0, row_y(2.0), (w - 12.0) / 3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_01_02, &mut state.py, x + 6.0 + (w - 12.0)/3.0, row_y(2.0), (w - 12.0)/3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_01_03, &mut state.pz, x + 6.0 + 2.0*(w - 12.0)/3.0, row_y(2.0), (w - 12.0)/3.0) { transform_changed = true; }

    ui.text("rotation x/y/z:");
    if ui.text_input(0x1_5A_02_01, &mut state.rx, x + 6.0, row_y(4.0), (w - 12.0)/3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_02_02, &mut state.ry, x + 6.0 + (w - 12.0)/3.0, row_y(4.0), (w - 12.0)/3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_02_03, &mut state.rz, x + 6.0 + 2.0*(w - 12.0)/3.0, row_y(4.0), (w - 12.0)/3.0) { transform_changed = true; }

    ui.text("scale x/y/z:");
    if ui.text_input(0x1_5A_03_01, &mut state.sx, x + 6.0, row_y(6.0), (w - 12.0)/3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_03_02, &mut state.sy, x + 6.0 + (w - 12.0)/3.0, row_y(6.0), (w - 12.0)/3.0) { transform_changed = true; }
    if ui.text_input(0x1_5A_03_03, &mut state.sz, x + 6.0 + 2.0*(w - 12.0)/3.0, row_y(6.0), (w - 12.0)/3.0) { transform_changed = true; }

    if ui.button("Despawn") {
        let _ = session.dispatch(EditorCommand::Despawn { entity });
        session.selected = None;
        ui.end_panel();
        return;
    }

    if tag_changed {
        let _ = session.dispatch(EditorCommand::SetTag {
            entity,
            tag: state.tag.clone(),
        });
    }

    if transform_changed {
        let (p, r, s) = (
            state.parsed_vec(&state.px, &state.py, &state.pz),
            state.parsed_vec(&state.rx, &state.ry, &state.rz),
            state.parsed_vec(&state.sx, &state.sy, &state.sz),
        );
        if let (Some(p), Some(r), Some(s)) = (p, r, s) {
            let _ = session.dispatch(EditorCommand::SetTransform {
                entity,
                position: p,
                rotation: r,
                scale: s,
            });
        }
    }

    ui.end_panel();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UiContext;

    #[test]
    fn inspector_runs_with_no_selection() {
        let mut ui = UiContext::new(800.0, 600.0);
        let mut session = EditorSession::new(
            std::env::temp_dir()
                .join("voltex_insp_test.vscn")
                .to_string_lossy()
                .into_owned(),
        );
        let mut state = InspectorState::default();
        ui.begin_frame(0.0, 0.0, false);
        inspector_panel(&mut ui, &mut session, &mut state, 0.0, 0.0, 300.0, 400.0);
        ui.end_frame();
    }

    #[test]
    fn inspector_sync_loads_transform_into_buffers() {
        let scene = std::env::temp_dir()
            .join("voltex_insp_sync.vscn")
            .to_string_lossy()
            .into_owned();
        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", scene));

        let mut session = EditorSession::new(scene.clone());
        let e = session.dispatch(EditorCommand::Spawn).unwrap();
        session.dispatch(EditorCommand::SetTransform {
            entity: e,
            position: Vec3::new(4.5, 0.0, -2.0),
            rotation: Vec3::ZERO,
            scale: Vec3::new(1.0, 1.0, 1.0),
        }).unwrap();
        session.selected = Some(e);

        let mut state = InspectorState::default();
        state.sync_from(&session);
        assert_eq!(state.px, "4.5");
        assert_eq!(state.pz, "-2");

        let _ = std::fs::remove_file(&scene);
        let _ = std::fs::remove_file(format!("{}.log", scene));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p voltex_editor --lib panels::inspector 2>&1 | tail -10`
Expected: `test result: ok. 2 passed`. 만약 `4.5` 포맷이 `4.5` 가 아니라 `4.5` 가 아닌 다른 형태로 나오면 (예: `4.5000`) assert를 실제 출력에 맞게 조정하지 말고 `format!("{}", 4.5_f32)` 이 `"4.5"` 를 만든다는 사실을 확인 (Rust f32 Display는 "4.5" 반환).

- [ ] **Step 3: Commit**

```bash
git add crates/voltex_editor/src/panels/inspector.rs
git commit -m "feat(editor): inspector_panel edits Transform and Tag through commands"
```

---

## Task 10: Rewrite editor_demo to use EditorSession

**Files:**
- Modify: `examples/editor_demo/Cargo.toml`
- Rewrite: `examples/editor_demo/src/main.rs`

- [ ] **Step 1: Add ecs/math deps to editor_demo Cargo.toml**

`examples/editor_demo/Cargo.toml` 의 `[dependencies]` 섹션에 두 줄 추가:

```toml
voltex_ecs.workspace = true
voltex_math.workspace = true
```

- [ ] **Step 2: Replace main.rs with scene-editor implementation**

기존 `examples/editor_demo/src/main.rs` 를 통째로 교체. (Read the existing file once to copy the boilerplate for window/wgpu setup; replace only the AppState fields and the per-frame UI section.)

새 파일 내용:

```rust
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::GpuContext;
use voltex_editor::{
    UiContext, UiRenderer,
    EditorSession,
    command::EditorCommand,
    panels::{hierarchy::hierarchy_panel, inspector::{inspector_panel, InspectorState}},
};

struct EditorDemoApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    input: InputState,
    timer: GameTimer,
    ui: UiContext,
    ui_renderer: UiRenderer,
    session: EditorSession,
    inspector_state: InspectorState,
}

impl ApplicationHandler for EditorDemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Editor".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new(window.handle.clone());

        let ui = UiContext::new(gpu.config.width as f32, gpu.config.height as f32);
        let ui_renderer = UiRenderer::new(&gpu.device, &gpu.queue, gpu.surface_format, &ui.font);

        let scene_path = "scenes/editor_scene.vscn".to_string();
        std::fs::create_dir_all("scenes").ok();
        let session = EditorSession::new(scene_path);

        self.state = Some(AppState {
            window,
            gpu,
            input: InputState::new(),
            timer: GameTimer::new(60),
            ui,
            ui_renderer,
            session,
            inspector_state: InspectorState::default(),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: winit::event::KeyEvent {
                    physical_key: PhysicalKey::Code(key_code),
                    state: kstate,
                    ..
                },
                ..
            } => {
                let pressed = matches!(kstate, winit::event::ElementState::Pressed);
                state.input.set_key(key_code, pressed);
                if key_code == KeyCode::Escape && pressed {
                    event_loop.exit();
                }
            }
            WindowEvent::MouseInput { state: bstate, button, .. } => {
                if button == winit::event::MouseButton::Left {
                    state.input.mouse_left = matches!(bstate, winit::event::ElementState::Pressed);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.input.mouse_x = position.x as f32;
                state.input.mouse_y = position.y as f32;
            }
            WindowEvent::Resized(size) => {
                state.gpu.resize(size.width, size.height);
                state.ui.screen_width = size.width as f32;
                state.ui.screen_height = size.height as f32;
            }
            WindowEvent::RedrawRequested => {
                state.timer.tick();
                state.ui.begin_frame(state.input.mouse_x, state.input.mouse_y, state.input.mouse_left);

                // ── Toolbar ──
                state.ui.begin_panel("Toolbar", 0.0, 0.0, state.ui.screen_width, 32.0);
                if state.ui.button("Spawn") {
                    let _ = state.session.dispatch(EditorCommand::Spawn);
                }
                if state.ui.button("Save") {
                    let path = state.session.scene_path.clone();
                    let _ = state.session.dispatch(EditorCommand::SaveScene { path });
                }
                if state.ui.button("Load") {
                    let path = state.session.scene_path.clone();
                    let _ = state.session.dispatch(EditorCommand::LoadScene { path });
                    state.session.selected = None;
                }
                state.ui.end_panel();

                // ── Hierarchy (left) ──
                hierarchy_panel(
                    &mut state.ui,
                    &mut state.session,
                    0.0, 32.0,
                    280.0, state.ui.screen_height - 32.0,
                );

                // ── Inspector (right) ──
                inspector_panel(
                    &mut state.ui,
                    &mut state.session,
                    &mut state.inspector_state,
                    state.ui.screen_width - 320.0, 32.0,
                    320.0, state.ui.screen_height - 32.0,
                );

                state.ui.end_frame();

                let output = match state.gpu.surface.get_current_texture() {
                    Ok(o) => o,
                    Err(_) => { state.window.handle.request_redraw(); return; }
                };
                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("editor_demo") });
                {
                    let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("clear"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.08, g: 0.09, b: 0.10, a: 1.0 }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    state.ui_renderer.render(&mut rp, &state.gpu.queue, &state.ui.draw_list, state.ui.screen_width, state.ui.screen_height);
                }
                state.gpu.queue.submit(Some(encoder.finish()));
                output.present();
                state.window.handle.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = EditorDemoApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
```

⚠️ 위 코드는 wgpu/winit 보일러플레이트를 기존 `editor_demo` 의 시그니처에 맞춰 작성한 추정 버전이다. 실제로 **빌드를 시도하기 전에** 기존 `examples/editor_demo/src/main.rs` 의 보일러플레이트(예: `gpu.surface_format`, `gpu.resize`, `UiRenderer::render` 시그니처) 를 한 번 더 확인하고, 시그니처 불일치가 있으면 그쪽에 맞춰 호출부를 조정한다. **로직(EditorSession + 두 패널)은 위 형태를 유지**.

- [ ] **Step 3: Verify build**

Run: `cargo build -p editor_demo 2>&1 | tail -15`
Expected: `Finished`. 컴파일 에러가 발생하면 기존 `editor_demo/src/main.rs` 의 wgpu/UiRenderer 호출 시그니처에 맞춰 보일러플레이트만 수정하고 로직은 유지한다.

- [ ] **Step 4: Run the demo briefly (manual smoke check)**

Run: `cargo run -p editor_demo --release` 윈도우가 열리는지 확인. 즉시 닫아도 됨 (`Esc`).
Expected: 윈도우가 열리고 패널 3개가 보임 (Toolbar 위쪽, Hierarchy 왼쪽, Inspector 오른쪽). "Spawn" 버튼 클릭 시 Hierarchy 에 `[0] (untagged)` 행이 추가.

GUI 검증이 불가능한 환경(headless CI 등)이라면 이 단계는 **수동 검증 항목**으로 표시하고 skip — `cargo build` 성공만 확인.

- [ ] **Step 5: Verify the scene file gets written**

Run: 데모를 실행하여 "Spawn" 3번 + "Save" 클릭, 종료. 그 다음:

```bash
cat scenes/editor_scene.vscn
```

Expected: 3개 엔티티가 들어간 JSON. 파일에 `"entities"` 와 `"transform"` 키가 있어야 함.

- [ ] **Step 6: Commit**

```bash
git add examples/editor_demo/Cargo.toml examples/editor_demo/src/main.rs
git commit -m "feat(editor_demo): rewrite as scene editor using EditorSession"
```

---

## Task 11: Full workspace test gate

- [ ] **Step 1: Run all tests**

Run: `cargo test --workspace --lib 2>&1 | grep "test result" | sort | uniq -c`
Expected: 기존 485 + 새로 추가된 voltex_editor 테스트 (약 14개) → 모든 라인 `0 failed`.

- [ ] **Step 2: Update docs/STATUS.md**

`docs/STATUS.md` 의 **Phase 8-4: Immediate Mode UI (Editor)** 섹션 아래에 추가:

```markdown
### Phase 8-5: Editor Command Foundation
- voltex_editor: EditorCommand (Spawn/Despawn/SetTransform/SetTag/SaveScene/LoadScene)
- voltex_editor: EditorSession (World + ComponentRegistry + auto-save + jsonl log)
- voltex_editor: hierarchy_panel, inspector_panel (panels module)
- examples/editor_demo: 실제 씬 에디터 (Toolbar + Hierarchy + Inspector)
```

그리고 "테스트: 485개 전부 통과" 줄을 실제 카운트로 업데이트한다 (`cargo test --workspace --lib 2>&1 | grep -oE '[0-9]+ passed' | awk '{sum += $1} END {print sum}'` 로 총합 산출).

- [ ] **Step 3: Final commit**

```bash
git add docs/STATUS.md
git commit -m "docs: record Phase 8-5 editor command foundation"
```

---

## Definition of Done

전체 plan 완료 시 다음 5개 모두 충족해야 한다:

1. `cargo build --workspace` 성공.
2. `cargo test --workspace --lib` 모든 패키지 `0 failed`.
3. `cargo run -p editor_demo` 실행 → 윈도우에 Toolbar/Hierarchy/Inspector 3개 패널 표시.
4. "Spawn" 3회 + "Save" 후 `scenes/editor_scene.vscn` 가 유효 JSON 으로 디스크에 존재 (`python -m json.tool scenes/editor_scene.vscn` 통과).
5. `scenes/editor_scene.vscn.log` 가 jsonl 형식으로 매 커맨드를 한 줄씩 기록.

## Out of Scope (다음 plan에서 다룸)

- 3D 뷰포트 (render-to-texture, ECS 씬을 패널 안에 렌더)
- Asset Browser (파일 시스템 탐색)
- UTF-8 / 한글 폰트 지원 (현재 ASCII만 → 한글 Tag는 디스크에는 저장되지만 화면 표시는 □)
- `voltex_script` Lua API 확장 + `Entity.generation` 버그 수정
- 핫리로드: 외부 편집기가 `.vscn` 을 수정하면 에디터가 자동 reload
- Undo/Redo (커맨드 로그를 이용한 역연산)
