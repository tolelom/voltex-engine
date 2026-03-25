# Scene Serialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add JSON scene serialization, binary scene format, and registration-based arbitrary component serialization to voltex_ecs.

**Architecture:** ComponentRegistry stores per-type serialize/deserialize functions. JSON and binary serializers use the registry to handle arbitrary components. Existing .vscn text format unchanged.

**Tech Stack:** Pure Rust, no external dependencies. Mini JSON writer/parser within voltex_ecs (not reusing voltex_renderer's json_parser to avoid dependency inversion).

---

### Task 1: Mini JSON Writer + Parser for voltex_ecs

**Files:**
- Create: `crates/voltex_ecs/src/json.rs`
- Modify: `crates/voltex_ecs/src/lib.rs`

- [ ] **Step 1: Write tests for JSON writer**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_null() {
        assert_eq!(json_write_null(), "null");
    }

    #[test]
    fn test_write_number() {
        assert_eq!(json_write_f32(3.14), "3.14");
        assert_eq!(json_write_f32(1.0), "1");
    }

    #[test]
    fn test_write_string() {
        assert_eq!(json_write_string("hello"), "\"hello\"");
        assert_eq!(json_write_string("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn test_write_array() {
        assert_eq!(json_write_array(&["1", "2", "3"]), "[1,2,3]");
    }

    #[test]
    fn test_write_object() {
        let pairs = vec![("name", "\"test\""), ("value", "42")];
        let result = json_write_object(&pairs);
        assert_eq!(result, r#"{"name":"test","value":42}"#);
    }

    #[test]
    fn test_parse_number() {
        match json_parse("42.5").unwrap() {
            JsonVal::Number(n) => assert!((n - 42.5).abs() < 1e-10),
            _ => panic!("expected number"),
        }
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(json_parse("\"hello\"").unwrap(), JsonVal::Str("hello".into()));
    }

    #[test]
    fn test_parse_array() {
        match json_parse("[1,2,3]").unwrap() {
            JsonVal::Array(a) => assert_eq!(a.len(), 3),
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn test_parse_object() {
        let val = json_parse(r#"{"x":1,"y":2}"#).unwrap();
        assert!(matches!(val, JsonVal::Object(_)));
    }

    #[test]
    fn test_parse_null() {
        assert_eq!(json_parse("null").unwrap(), JsonVal::Null);
    }

    #[test]
    fn test_parse_nested() {
        let val = json_parse(r#"{"a":[1,2],"b":{"c":3}}"#).unwrap();
        assert!(matches!(val, JsonVal::Object(_)));
    }
}
```

- [ ] **Step 2: Implement JSON writer and parser**

Implement in `crates/voltex_ecs/src/json.rs`:
- `JsonVal` enum: Null, Bool(bool), Number(f64), Str(String), Array(Vec), Object(Vec<(String, JsonVal)>)
- Writer helpers: `json_write_null`, `json_write_f32`, `json_write_string`, `json_write_array`, `json_write_object`
- `json_parse(input: &str) -> Result<JsonVal, String>` — minimal recursive descent parser
- `JsonVal::get(key)`, `as_f64()`, `as_str()`, `as_array()`, `as_object()` accessors

Register in lib.rs: `pub mod json;`

- [ ] **Step 3: Run tests, commit**

```bash
cargo test --package voltex_ecs -- json::tests -v
git add crates/voltex_ecs/src/json.rs crates/voltex_ecs/src/lib.rs
git commit -m "feat(ecs): add mini JSON writer and parser for scene serialization"
```

---

### Task 2: ComponentRegistry

**Files:**
- Create: `crates/voltex_ecs/src/component_registry.rs`
- Modify: `crates/voltex_ecs/src/lib.rs`

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{World, Transform};
    use voltex_math::Vec3;

    #[test]
    fn test_register_and_serialize() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        assert!(registry.find("transform").is_some());
        assert!(registry.find("tag").is_some());
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
    }

    #[test]
    fn test_roundtrip_transform() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));

        let entry = registry.find("transform").unwrap();
        let data = (entry.serialize)(&world, e).unwrap();

        let mut world2 = World::new();
        let e2 = world2.spawn();
        (entry.deserialize)(&mut world2, e2, &data).unwrap();

        let t = world2.get::<Transform>(e2).unwrap();
        assert!((t.position.x - 1.0).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Implement ComponentRegistry**

```rust
// crates/voltex_ecs/src/component_registry.rs
use crate::entity::Entity;
use crate::world::World;

pub type SerializeFn = fn(&World, Entity) -> Option<Vec<u8>>;
pub type DeserializeFn = fn(&mut World, Entity, &[u8]) -> Result<(), String>;

pub struct ComponentEntry {
    pub name: String,
    pub serialize: SerializeFn,
    pub deserialize: DeserializeFn,
}

pub struct ComponentRegistry {
    entries: Vec<ComponentEntry>,
}

impl ComponentRegistry {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    pub fn register(&mut self, name: &str, ser: SerializeFn, deser: DeserializeFn) {
        self.entries.push(ComponentEntry {
            name: name.to_string(), serialize: ser, deserialize: deser,
        });
    }

    pub fn find(&self, name: &str) -> Option<&ComponentEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn entries(&self) -> &[ComponentEntry] { &self.entries }

    pub fn register_defaults(&mut self) {
        // Transform: serialize as 9 f32s (pos.xyz, rot.xyz, scale.xyz)
        self.register("transform", serialize_transform, deserialize_transform);
        // Tag: serialize as UTF-8 string bytes
        self.register("tag", serialize_tag, deserialize_tag);
        // Parent handled specially (entity reference)
    }
}

fn serialize_transform(world: &World, entity: Entity) -> Option<Vec<u8>> {
    let t = world.get::<crate::Transform>(entity)?;
    let mut data = Vec::with_capacity(36);
    for &v in &[t.position.x, t.position.y, t.position.z,
                t.rotation.x, t.rotation.y, t.rotation.z,
                t.scale.x, t.scale.y, t.scale.z] {
        data.extend_from_slice(&v.to_le_bytes());
    }
    Some(data)
}

fn deserialize_transform(world: &mut World, entity: Entity, data: &[u8]) -> Result<(), String> {
    if data.len() < 36 { return Err("Transform data too short".into()); }
    let f = |off: usize| f32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]]);
    let t = crate::Transform {
        position: voltex_math::Vec3::new(f(0), f(4), f(8)),
        rotation: voltex_math::Vec3::new(f(12), f(16), f(20)),
        scale: voltex_math::Vec3::new(f(24), f(28), f(32)),
    };
    world.add(entity, t);
    Ok(())
}

fn serialize_tag(world: &World, entity: Entity) -> Option<Vec<u8>> {
    let tag = world.get::<crate::scene::Tag>(entity)?;
    Some(tag.0.as_bytes().to_vec())
}

fn deserialize_tag(world: &mut World, entity: Entity, data: &[u8]) -> Result<(), String> {
    let s = std::str::from_utf8(data).map_err(|_| "Invalid UTF-8 in tag")?;
    world.add(entity, crate::scene::Tag(s.to_string()));
    Ok(())
}
```

- [ ] **Step 3: Run tests, commit**

```bash
cargo test --package voltex_ecs -- component_registry -v
git add crates/voltex_ecs/src/component_registry.rs crates/voltex_ecs/src/lib.rs
git commit -m "feat(ecs): add ComponentRegistry for arbitrary component serialization"
```

---

### Task 3: JSON Scene Serialization

**Files:**
- Modify: `crates/voltex_ecs/src/scene.rs`

- [ ] **Step 1: Write tests**

```rust
#[test]
fn test_json_roundtrip() {
    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));
    world.add(e, Tag("player".into()));

    let json = serialize_scene_json(&world, &registry);
    assert!(json.contains("\"version\":1"));
    assert!(json.contains("\"player\""));

    let mut world2 = World::new();
    let entities = deserialize_scene_json(&mut world2, &json, &registry).unwrap();
    assert_eq!(entities.len(), 1);
    let t = world2.get::<Transform>(entities[0]).unwrap();
    assert!((t.position.x - 1.0).abs() < 1e-4);
}

#[test]
fn test_json_with_parent() {
    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    let mut world = World::new();
    let parent = world.spawn();
    let child = world.spawn();
    world.add(parent, Transform::new());
    world.add(child, Transform::new());
    add_child(&mut world, parent, child);

    let json = serialize_scene_json(&world, &registry);
    let mut world2 = World::new();
    let entities = deserialize_scene_json(&mut world2, &json, &registry).unwrap();
    assert!(world2.get::<Parent>(entities[1]).is_some());
}
```

- [ ] **Step 2: Implement serialize_scene_json and deserialize_scene_json**

Use the json module's writer/parser. Serialize components via registry.
Format: `{"version":1,"entities":[{"parent":null_or_idx,"components":{"transform":"base64_data","tag":"base64_data",...}}]}`
Use base64 encoding for component binary data in JSON (reuse decoder pattern or simple hex encoding).

- [ ] **Step 3: Run tests, commit**

---

### Task 4: Binary Scene Format

**Files:**
- Create: `crates/voltex_ecs/src/binary_scene.rs`
- Modify: `crates/voltex_ecs/src/lib.rs`

- [ ] **Step 1: Write tests**

```rust
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
    assert!(world2.get::<Parent>(entities[1]).is_some());
}

#[test]
fn test_binary_invalid_magic() {
    let data = vec![0u8; 20];
    let mut world = World::new();
    let registry = ComponentRegistry::new();
    assert!(deserialize_scene_binary(&mut world, &data, &registry).is_err());
}
```

- [ ] **Step 2: Implement binary format**

Header: `VSCN` + version(1) u32 LE + entity_count u32 LE
Per entity: parent_index i32 LE (-1 = no parent) + component_count u32 LE
Per component: name_len u16 LE + name bytes + data_len u32 LE + data bytes

- [ ] **Step 3: Run tests, commit**

---

### Task 5: Exports and Full Verification

- [ ] **Step 1: Update lib.rs exports**

```rust
pub mod json;
pub mod component_registry;
pub mod binary_scene;
pub use component_registry::ComponentRegistry;
pub use binary_scene::{serialize_scene_binary, deserialize_scene_binary};
// scene.rs already exports serialize_scene, deserialize_scene
// Add: pub use scene::{serialize_scene_json, deserialize_scene_json};
```

- [ ] **Step 2: Run full tests**

```bash
cargo test --package voltex_ecs -v
cargo build --workspace
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(ecs): complete JSON and binary scene serialization with component registry"
```
