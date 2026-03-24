# Phase 3b: Scene Graph + Serialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 부모-자식 트랜스폼 계층으로 씬 그래프를 구축하고, 로컬→월드 트랜스폼 자동 전파, 간단한 텍스트 포맷으로 씬 저장/로드를 구현한다.

**Architecture:** voltex_ecs에 `Parent(Entity)`, `Children(Vec<Entity>)` 컴포넌트를 추가하고, 계층 관리 함수를 `hierarchy.rs`에 구현한다. World 트랜스폼 전파는 루트에서 리프까지 top-down 순회. 씬 직렬화는 커스텀 텍스트 포맷(`.vscn`)으로 Transform/Parent/Children/커스텀 태그를 저장한다.

**Tech Stack:** Rust 1.94, voltex_math (Vec3, Mat4), voltex_ecs (World, Entity, Transform, SparseSet)

**Spec:** `docs/superpowers/specs/2026-03-24-voltex-engine-design.md` Phase 3 (3-2. 씬 그래프)

---

## File Structure

```
crates/voltex_ecs/src/
├── lib.rs              # 모듈 re-export 업데이트
├── hierarchy.rs        # Parent, Children 컴포넌트 + 계층 관리 함수 (NEW)
├── world_transform.rs  # WorldTransform + 전파 함수 (NEW)
├── scene.rs            # 씬 직렬화/역직렬화 (NEW)
examples/
└── hierarchy_demo/     # 씬 그래프 데모 (NEW)
    ├── Cargo.toml
    └── src/
        └── main.rs
```

---

## Task 1: 계층 컴포넌트 + 관리 함수

**Files:**
- Create: `crates/voltex_ecs/src/hierarchy.rs`
- Modify: `crates/voltex_ecs/src/lib.rs`

Parent(Entity)와 Children(Vec<Entity>) 컴포넌트. 계층을 조작하는 free function들.

- [ ] **Step 1: hierarchy.rs 작성**

```rust
// crates/voltex_ecs/src/hierarchy.rs
use crate::{Entity, World};

/// 부모 엔티티를 가리키는 컴포넌트
#[derive(Debug, Clone, Copy)]
pub struct Parent(pub Entity);

/// 자식 엔티티 목록 컴포넌트
#[derive(Debug, Clone)]
pub struct Children(pub Vec<Entity>);

/// entity를 parent의 자식으로 추가
pub fn add_child(world: &mut World, parent: Entity, child: Entity) {
    // child에 Parent 컴포넌트 설정
    world.add(child, Parent(parent));

    // parent의 Children에 child 추가
    if let Some(children) = world.get_mut::<Children>(parent) {
        if !children.0.contains(&child) {
            children.0.push(child);
        }
    } else {
        world.add(parent, Children(vec![child]));
    }
}

/// entity를 부모에서 분리 (parent-child 관계 제거)
pub fn remove_child(world: &mut World, parent: Entity, child: Entity) {
    // parent의 Children에서 child 제거
    if let Some(children) = world.get_mut::<Children>(parent) {
        children.0.retain(|&e| e != child);
    }

    // child의 Parent 제거
    world.remove::<Parent>(child);
}

/// entity와 모든 자손을 재귀적으로 삭제
pub fn despawn_recursive(world: &mut World, entity: Entity) {
    // 자식들 먼저 수집 (borrow 문제 회피)
    let children: Vec<Entity> = world.get::<Children>(entity)
        .map(|c| c.0.clone())
        .unwrap_or_default();

    for child in children {
        despawn_recursive(world, child);
    }

    // 부모에서 자신 제거
    if let Some(parent) = world.get::<Parent>(entity).map(|p| p.0) {
        if let Some(children) = world.get_mut::<Children>(parent) {
            children.0.retain(|&e| e != entity);
        }
    }

    world.despawn(entity);
}

/// 루트 엔티티 목록 반환 (Parent가 없는 엔티티)
pub fn roots(world: &World) -> Vec<Entity> {
    // Transform가 있지만 Parent가 없는 엔티티가 루트
    world.query::<crate::Transform>()
        .filter(|(entity, _)| world.get::<Parent>(*entity).is_none())
        .map(|(entity, _)| entity)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transform;
    use voltex_math::Vec3;

    #[test]
    fn test_add_child() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();
        world.add(parent, Transform::new());
        world.add(child, Transform::new());

        add_child(&mut world, parent, child);

        assert_eq!(world.get::<Parent>(child).unwrap().0, parent);
        assert_eq!(world.get::<Children>(parent).unwrap().0.len(), 1);
        assert_eq!(world.get::<Children>(parent).unwrap().0[0], child);
    }

    #[test]
    fn test_add_multiple_children() {
        let mut world = World::new();
        let parent = world.spawn();
        let c1 = world.spawn();
        let c2 = world.spawn();
        world.add(parent, Transform::new());
        world.add(c1, Transform::new());
        world.add(c2, Transform::new());

        add_child(&mut world, parent, c1);
        add_child(&mut world, parent, c2);

        assert_eq!(world.get::<Children>(parent).unwrap().0.len(), 2);
    }

    #[test]
    fn test_remove_child() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();
        world.add(parent, Transform::new());
        world.add(child, Transform::new());

        add_child(&mut world, parent, child);
        remove_child(&mut world, parent, child);

        assert!(world.get::<Parent>(child).is_none());
        assert_eq!(world.get::<Children>(parent).unwrap().0.len(), 0);
    }

    #[test]
    fn test_despawn_recursive() {
        let mut world = World::new();
        let root = world.spawn();
        let child = world.spawn();
        let grandchild = world.spawn();
        world.add(root, Transform::new());
        world.add(child, Transform::new());
        world.add(grandchild, Transform::new());

        add_child(&mut world, root, child);
        add_child(&mut world, child, grandchild);

        despawn_recursive(&mut world, root);

        assert!(!world.is_alive(root));
        assert!(!world.is_alive(child));
        assert!(!world.is_alive(grandchild));
    }

    #[test]
    fn test_roots() {
        let mut world = World::new();
        let r1 = world.spawn();
        let r2 = world.spawn();
        let child = world.spawn();
        world.add(r1, Transform::new());
        world.add(r2, Transform::new());
        world.add(child, Transform::new());

        add_child(&mut world, r1, child);

        let root_list = roots(&world);
        assert_eq!(root_list.len(), 2);
        assert!(root_list.contains(&r1));
        assert!(root_list.contains(&r2));
        assert!(!root_list.contains(&child));
    }

    #[test]
    fn test_no_duplicate_child() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();
        world.add(parent, Transform::new());
        world.add(child, Transform::new());

        add_child(&mut world, parent, child);
        add_child(&mut world, parent, child); // 중복 추가

        assert_eq!(world.get::<Children>(parent).unwrap().0.len(), 1);
    }
}
```

- [ ] **Step 2: lib.rs 업데이트**

```rust
// crates/voltex_ecs/src/lib.rs
pub mod entity;
pub mod sparse_set;
pub mod world;
pub mod transform;
pub mod hierarchy;

pub use entity::{Entity, EntityAllocator};
pub use sparse_set::SparseSet;
pub use world::World;
pub use transform::Transform;
pub use hierarchy::{Parent, Children, add_child, remove_child, despawn_recursive, roots};
```

- [ ] **Step 3: 테스트 통과 확인**

Run: `cargo test -p voltex_ecs`
Expected: 기존 25 + hierarchy 6 = 31개 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_ecs/
git commit -m "feat(ecs): add Parent/Children hierarchy with add_child, remove_child, despawn_recursive"
```

---

## Task 2: WorldTransform 전파

**Files:**
- Create: `crates/voltex_ecs/src/world_transform.rs`
- Modify: `crates/voltex_ecs/src/lib.rs`

로컬 Transform에서 월드 행렬을 계산하여 WorldTransform에 저장. 루트에서 리프까지 top-down 순회로 부모의 월드 행렬을 자식에 곱한다.

- [ ] **Step 1: world_transform.rs 작성**

```rust
// crates/voltex_ecs/src/world_transform.rs
use voltex_math::Mat4;
use crate::{Entity, World, Transform};
use crate::hierarchy::{Parent, Children};

/// 계산된 월드 트랜스폼 (로컬 * 부모 월드)
#[derive(Debug, Clone, Copy)]
pub struct WorldTransform(pub Mat4);

impl WorldTransform {
    pub fn identity() -> Self {
        Self(Mat4::IDENTITY)
    }
}

/// 모든 엔티티의 WorldTransform을 갱신한다.
/// 루트(Parent 없는)부터 시작하여 자식으로 전파.
pub fn propagate_transforms(world: &mut World) {
    // 루트 엔티티 수집
    let root_entities: Vec<Entity> = world.query::<Transform>()
        .filter(|(e, _)| world.get::<Parent>(*e).is_none())
        .map(|(e, _)| e)
        .collect();

    for root in root_entities {
        propagate_entity(world, root, Mat4::IDENTITY);
    }
}

fn propagate_entity(world: &mut World, entity: Entity, parent_world: Mat4) {
    let local = match world.get::<Transform>(entity) {
        Some(t) => t.matrix(),
        None => return,
    };
    let world_matrix = parent_world * local;

    world.add(entity, WorldTransform(world_matrix));

    // 자식들 수집 (borrow 회피)
    let children: Vec<Entity> = world.get::<Children>(entity)
        .map(|c| c.0.clone())
        .unwrap_or_default();

    for child in children {
        propagate_entity(world, child, world_matrix);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::add_child;
    use voltex_math::{Vec3, Vec4};

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn test_root_world_transform() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(5.0, 0.0, 0.0)));

        propagate_transforms(&mut world);

        let wt = world.get::<WorldTransform>(e).unwrap();
        let p = wt.0 * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(p.x, 5.0));
    }

    #[test]
    fn test_child_inherits_parent() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();

        world.add(parent, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        world.add(child, Transform::from_position(Vec3::new(0.0, 5.0, 0.0)));
        add_child(&mut world, parent, child);

        propagate_transforms(&mut world);

        // child의 월드 위치: parent(10,0,0) + child(0,5,0) = (10,5,0)
        let wt = world.get::<WorldTransform>(child).unwrap();
        let p = wt.0 * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(p.x, 10.0), "x: {}", p.x);
        assert!(approx_eq(p.y, 5.0), "y: {}", p.y);
    }

    #[test]
    fn test_three_level_hierarchy() {
        let mut world = World::new();
        let root = world.spawn();
        let mid = world.spawn();
        let leaf = world.spawn();

        world.add(root, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        world.add(mid, Transform::from_position(Vec3::new(0.0, 2.0, 0.0)));
        world.add(leaf, Transform::from_position(Vec3::new(0.0, 0.0, 3.0)));

        add_child(&mut world, root, mid);
        add_child(&mut world, mid, leaf);

        propagate_transforms(&mut world);

        // leaf 월드 위치: (1, 2, 3)
        let wt = world.get::<WorldTransform>(leaf).unwrap();
        let p = wt.0 * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(p.x, 1.0));
        assert!(approx_eq(p.y, 2.0));
        assert!(approx_eq(p.z, 3.0));
    }

    #[test]
    fn test_parent_scale_affects_child() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();

        world.add(parent, Transform::from_position_scale(
            Vec3::ZERO,
            Vec3::new(2.0, 2.0, 2.0),
        ));
        world.add(child, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        add_child(&mut world, parent, child);

        propagate_transforms(&mut world);

        // parent 스케일 2x → child(1,0,0)이 (2,0,0)으로
        let wt = world.get::<WorldTransform>(child).unwrap();
        let p = wt.0 * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(p.x, 2.0), "x: {}", p.x);
    }
}
```

- [ ] **Step 2: lib.rs 업데이트**

```rust
// lib.rs에 추가:
pub mod world_transform;
pub use world_transform::{WorldTransform, propagate_transforms};
```

- [ ] **Step 3: 테스트 통과 확인**

Run: `cargo test -p voltex_ecs`
Expected: 31 + 4 = 35개 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_ecs/
git commit -m "feat(ecs): add WorldTransform propagation through parent-child hierarchy"
```

---

## Task 3: 씬 직렬화 (.vscn 포맷)

**Files:**
- Create: `crates/voltex_ecs/src/scene.rs`
- Modify: `crates/voltex_ecs/src/lib.rs`

간단한 텍스트 포맷으로 씬을 저장/로드. 포맷:

```
# Voltex Scene v1
entity 0
  transform 1.0 2.0 3.0 | 0.0 0.5 0.0 | 1.0 1.0 1.0
  tag solar_system

entity 1
  parent 0
  transform 5.0 0.0 0.0 | 0.0 0.0 0.0 | 0.5 0.5 0.5
  tag planet
```

규칙:
- `entity N` — 엔티티 시작 (N은 파일 내 로컬 인덱스)
- `  transform px py pz | rx ry rz | sx sy sz` — Transform 컴포넌트
- `  parent N` — 부모 엔티티의 로컬 인덱스
- `  tag name` — 문자열 태그 (선택적, 디버깅용)

- [ ] **Step 1: scene.rs 작성**

```rust
// crates/voltex_ecs/src/scene.rs
use crate::{Entity, World, Transform};
use crate::hierarchy::{Parent, Children, add_child};
use voltex_math::Vec3;

/// 디버깅/식별용 태그 컴포넌트
#[derive(Debug, Clone)]
pub struct Tag(pub String);

/// World의 씬 데이터를 .vscn 텍스트로 직렬화
pub fn serialize_scene(world: &World) -> String {
    let mut output = String::from("# Voltex Scene v1\n");

    // Transform를 가진 모든 엔티티 수집
    let entities: Vec<(Entity, Transform)> = world.query::<Transform>()
        .map(|(e, t)| (e, *t))
        .collect();

    // Entity → 로컬 인덱스 매핑
    let entity_to_idx: std::collections::HashMap<Entity, usize> = entities.iter()
        .enumerate()
        .map(|(i, (e, _))| (*e, i))
        .collect();

    for (idx, (entity, transform)) in entities.iter().enumerate() {
        output.push_str(&format!("\nentity {}\n", idx));

        // Transform
        output.push_str(&format!(
            "  transform {} {} {} | {} {} {} | {} {} {}\n",
            transform.position.x, transform.position.y, transform.position.z,
            transform.rotation.x, transform.rotation.y, transform.rotation.z,
            transform.scale.x, transform.scale.y, transform.scale.z,
        ));

        // Parent
        if let Some(parent) = world.get::<Parent>(*entity) {
            if let Some(&parent_idx) = entity_to_idx.get(&parent.0) {
                output.push_str(&format!("  parent {}\n", parent_idx));
            }
        }

        // Tag
        if let Some(tag) = world.get::<Tag>(*entity) {
            output.push_str(&format!("  tag {}\n", tag.0));
        }
    }

    output
}

/// .vscn 텍스트를 파싱하여 World에 엔티티를 생성
pub fn deserialize_scene(world: &mut World, source: &str) -> Vec<Entity> {
    let mut entities: Vec<Entity> = Vec::new();
    let mut current_entity: Option<Entity> = None;
    // 로컬 인덱스 → (parent_local_idx) 매핑 (나중에 해석)
    let mut parent_map: Vec<Option<usize>> = Vec::new();

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with("entity ") {
            let entity = world.spawn();
            entities.push(entity);
            current_entity = Some(entity);
            parent_map.push(None);
            continue;
        }

        let entity = match current_entity {
            Some(e) => e,
            None => continue,
        };

        if line.starts_with("transform ") {
            if let Some(transform) = parse_transform(&line[10..]) {
                world.add(entity, transform);
            }
        } else if line.starts_with("parent ") {
            if let Ok(parent_idx) = line[7..].trim().parse::<usize>() {
                let idx = entities.len() - 1;
                parent_map[idx] = Some(parent_idx);
            }
        } else if line.starts_with("tag ") {
            let tag_name = line[4..].trim().to_string();
            world.add(entity, Tag(tag_name));
        }
    }

    // parent 관계 설정
    for (child_idx, parent_idx_opt) in parent_map.iter().enumerate() {
        if let Some(parent_idx) = parent_idx_opt {
            if *parent_idx < entities.len() && child_idx < entities.len() {
                add_child(world, entities[*parent_idx], entities[child_idx]);
            }
        }
    }

    entities
}

fn parse_transform(s: &str) -> Option<Transform> {
    // "px py pz | rx ry rz | sx sy sz"
    let parts: Vec<&str> = s.split('|').collect();
    if parts.len() != 3 {
        return None;
    }

    let pos = parse_vec3(parts[0].trim())?;
    let rot = parse_vec3(parts[1].trim())?;
    let scale = parse_vec3(parts[2].trim())?;

    Some(Transform {
        position: pos,
        rotation: rot,
        scale,
    })
}

fn parse_vec3(s: &str) -> Option<Vec3> {
    let nums: Vec<f32> = s.split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    if nums.len() == 3 {
        Some(Vec3::new(nums[0], nums[1], nums[2]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::roots;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn test_serialize_single_entity() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));
        world.add(e, Tag("test".into()));

        let text = serialize_scene(&world);
        assert!(text.contains("entity 0"));
        assert!(text.contains("transform 1 2 3"));
        assert!(text.contains("tag test"));
    }

    #[test]
    fn test_serialize_with_parent() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();
        world.add(parent, Transform::new());
        world.add(child, Transform::new());
        add_child(&mut world, parent, child);

        let text = serialize_scene(&world);
        assert!(text.contains("parent"));
    }

    #[test]
    fn test_roundtrip() {
        let mut world = World::new();
        let root = world.spawn();
        let child = world.spawn();
        world.add(root, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        world.add(root, Tag("root_node".into()));
        world.add(child, Transform::from_position(Vec3::new(0.0, 5.0, 0.0)));
        world.add(child, Tag("child_node".into()));
        add_child(&mut world, root, child);

        let text = serialize_scene(&world);

        // 새 World에 역직렬화
        let mut world2 = World::new();
        let entities = deserialize_scene(&mut world2, &text);

        assert_eq!(entities.len(), 2);

        // Transform 확인
        let t0 = world2.get::<Transform>(entities[0]).unwrap();
        assert!(approx_eq(t0.position.x, 10.0));

        let t1 = world2.get::<Transform>(entities[1]).unwrap();
        assert!(approx_eq(t1.position.y, 5.0));

        // 부모-자식 관계 확인
        let parent_comp = world2.get::<Parent>(entities[1]).unwrap();
        assert_eq!(parent_comp.0, entities[0]);

        // Tag 확인
        let tag0 = world2.get::<Tag>(entities[0]).unwrap();
        assert_eq!(tag0.0, "root_node");
    }

    #[test]
    fn test_deserialize_roots() {
        let scene = "\
# Voltex Scene v1

entity 0
  transform 0 0 0 | 0 0 0 | 1 1 1

entity 1
  parent 0
  transform 1 0 0 | 0 0 0 | 1 1 1

entity 2
  transform 5 0 0 | 0 0 0 | 1 1 1
";
        let mut world = World::new();
        let entities = deserialize_scene(&mut world, scene);
        assert_eq!(entities.len(), 3);

        let root_list = roots(&world);
        assert_eq!(root_list.len(), 2); // entity 0 and entity 2
    }
}
```

- [ ] **Step 2: lib.rs 업데이트**

```rust
// lib.rs에 추가:
pub mod scene;
pub use scene::{Tag, serialize_scene, deserialize_scene};
```

- [ ] **Step 3: 테스트 통과 확인**

Run: `cargo test -p voltex_ecs`
Expected: 35 + 4 = 39개 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_ecs/
git commit -m "feat(ecs): add scene serialization/deserialization (.vscn format)"
```

---

## Task 4: hierarchy_demo 예제

**Files:**
- Create: `examples/hierarchy_demo/Cargo.toml`
- Create: `examples/hierarchy_demo/src/main.rs`
- Modify: `Cargo.toml` (워크스페이스에 추가)

씬 그래프를 시각적으로 보여주는 데모. 태양계 모델: 태양(회전) → 행성(공전+자전) → 위성(공전). 씬을 .vscn 파일로 저장하고 다시 로드하는 기능 포함.

- [ ] **Step 1: 워크스페이스 + Cargo.toml**

workspace members에 `"examples/hierarchy_demo"` 추가.

```toml
# examples/hierarchy_demo/Cargo.toml
[package]
name = "hierarchy_demo"
version = "0.1.0"
edition = "2021"

[dependencies]
voltex_math.workspace = true
voltex_platform.workspace = true
voltex_renderer.workspace = true
voltex_ecs.workspace = true
wgpu.workspace = true
winit.workspace = true
bytemuck.workspace = true
pollster.workspace = true
env_logger.workspace = true
log.workspace = true
```

- [ ] **Step 2: main.rs 작성**

이 파일은 many_cubes를 기반으로 하되, ECS 계층을 사용한다. 핵심 변경:

1. 태양계 구축: 태양(중앙) → 행성 3개(공전) → 위성 1개(행성의 자식)
2. 매 프레임: Transform의 rotation.y를 dt만큼 증가시킨 뒤 `propagate_transforms()` 호출
3. 렌더링: `world.query::<WorldTransform>()`으로 월드 행렬을 직접 사용
4. S키: 씬을 `scene.vscn`로 저장, L키: `scene.vscn` 로드

구현은 many_cubes의 dynamic UBO 패턴을 따른다. 핵심 차이:
- Transform 대신 WorldTransform의 행렬을 uniform.model로 사용
- 엔티티별 회전은 Transform.rotation.y를 증가시킴
- propagate_transforms로 월드 행렬 계산

파일을 작성하기 전에 반드시 `examples/many_cubes/src/main.rs`를 읽고 dynamic UBO 패턴을 따를 것.

씬 구축:
```
Sun: position(0,0,0), scale(2,2,2), rotation.y += dt*0.2
├── Planet1: position(6,0,0), scale(0.5,0.5,0.5), rotation.y += dt*1.0
│   └── Moon: position(1.5,0,0), scale(0.3,0.3,0.3), rotation.y += dt*2.0
├── Planet2: position(10,0,0), scale(0.7,0.7,0.7), rotation.y += dt*0.6
└── Planet3: position(14,0,0), scale(0.4,0.4,0.4), rotation.y += dt*0.3
```

S키 저장: `voltex_ecs::serialize_scene(&world)`를 `scene.vscn`에 쓰기.
L키 로드: `scene.vscn`을 읽어 `voltex_ecs::deserialize_scene()`으로 World 재구축.

- [ ] **Step 3: 빌드 + 테스트 확인**

Run: `cargo build --workspace`
Run: `cargo test --workspace`

- [ ] **Step 4: 실행 확인**

Run: `cargo run -p hierarchy_demo`
Expected: 태양 주위로 행성이 공전, 행성 주위로 위성 공전. S키로 씬 저장, L키로 로드.

- [ ] **Step 5: 커밋**

```bash
git add Cargo.toml examples/hierarchy_demo/
git commit -m "feat: add hierarchy_demo with solar system scene graph and save/load"
```

---

## Phase 3b 완료 기준 체크리스트

- [ ] `cargo build --workspace` 성공
- [ ] `cargo test --workspace` — 모든 테스트 통과
- [ ] 부모-자식 트랜스폼 전파 정상 (3단계 계층)
- [ ] 씬 직렬화 roundtrip: 저장 → 로드 → 동일 결과
- [ ] `cargo run -p hierarchy_demo` — 태양계 렌더링, 계층적 회전, S/L 저장/로드
- [ ] 기존 예제 (triangle, model_viewer, many_cubes) 여전히 동작
