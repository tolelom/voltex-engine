# Cycle 12 (E3, flex) — AddComponent / RemoveComponent + Despawn 풀 undo

**작성일**: 2026-05-21
**범위**: 모든 mutation 을 undoable 하게 만든다. ComponentRegistry 의 raw byte API 를 활용해 임의 컴포넌트 추가/제거 + Despawn 의 entity·컴포넌트 풀 복원.
**선행 조건**: Cycle 11 (B3) 까지 완료. 본 cycle 은 critical path 가 아니지만 Cycle 13 (A, rustdoc) 전에는 끝나야 한다 (공개 API 가 굳어지기 전 마지막 변경 기회).
**로드맵 참조**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 12

---

## 1. 목표

현재 `EditorCommand` 는 Spawn/SetTransform/SetTag/AddChild/RemoveChild/SaveScene/LoadScene 의 mutation 6개만 undo 한다. Despawn 은 undo 시 history 를 clear 한다 (irreversible).

이 cycle 종료 시점에:

1. **`EditorCommand::AddComponent { entity, name, data }`** — registry 에 등록된 임의 컴포넌트를 byte payload 로 추가. inverse = RemoveComponent.
2. **`EditorCommand::RemoveComponent { entity, name }`** — 동일 이름의 컴포넌트를 제거. inverse = AddComponent (사전 snapshot bytes).
3. **`EntityAllocator::restore_at_generation(id, gen) → bool`** — 죽은 slot 을 원래 generation 으로 되살림.
4. **Despawn 의 풀 undo** — 사전 모든 registered 컴포넌트 snapshot + entity 슬롯 복원 → 컴포넌트 복원. undo history 더 이상 clear 하지 않음.
5. **Inspector 의 "Add Component" UI** — 등록된 컴포넌트 이름 버튼 → `AddComponent` dispatch (default_bytes 사용).

## 2. 비포함

- 컴포넌트별 GUI 자동 생성 (각 컴포넌트의 인스펙터 UI 는 v1.1+ 수동)
- 멀티 엔티티 batch undo
- Spawn ↔ Despawn 사이클이 generation 을 이상하게 만드는 corner case (예: deallocate → 다른 엔티티가 같은 슬롯 사용 → 첫 deallocate undo): `restore_at_generation` 이 false 반환, dispatch 가 undo 실패로 reported (silent skip, UI 가 disabled state 로 진입)
- 미등록 컴포넌트의 Despawn-undo 복원 (E2 정책과 동일: 등록 안 된 컴포넌트는 snapshot 에 없음, silent loss)
- `Component` trait 의 `remove` / `default_bytes` 메서드는 default impl 제공해 기존 impl 무수정 (backward compatible)

## 3. 설계

### 3.1 Component trait 확장

```rust
pub trait Component: 'static + Sized {
    const NAME: &'static str;
    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>>;
    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> Result<()>;

    /// Default impl: remove `Self` from world by TypeId. Returns true if it was present.
    /// Override if removing requires extra cleanup (rare).
    fn remove(world: &mut World, entity: Entity) -> bool {
        world.remove::<Self>(entity).is_some()
    }

    /// Bytes that deserialize into a "sensible default" of this component.
    /// Default impl returns empty `Vec` — overriders must provide bytes whose
    /// `deserialize` succeeds. Used by `EditorCommand::AddComponent` from
    /// Inspector UI buttons.
    fn default_bytes() -> Vec<u8> {
        Vec::new()
    }
}
```

#### Built-in `default_bytes` overrides

| Component | Bytes |
|---|---|
| `Transform` | 36 zero bytes for pos+rot, scale = (1,1,1) → little-endian f32 layout |
| `Tag` | empty string (0 bytes) — deserialize already handles |
| `MeshHandle`, `MaterialHandle` | 8 bytes of zero (= sentinel index 0, gen 0 — Handle::stale). Deserialize allows it; renderer skips stale handles. |
| `Bounds` | 24 bytes: AABB { min: (0,0,0), max: (0,0,0) } |
| `ScriptComponent` | `[u32::MAX as LE bytes (UNBOUND)] + 빈 path` → 4 bytes only |

These are documented in each impl block.

### 3.2 ComponentEntry 확장

```rust
pub struct ComponentEntry {
    pub name: String,
    pub serialize: SerializeFn,
    pub deserialize: DeserializeFn,
    pub remove: RemoveFn,          // NEW
    pub default_bytes: DefaultFn,  // NEW
}

pub type RemoveFn = fn(&mut World, Entity) -> bool;
pub type DefaultFn = fn() -> Vec<u8>;
```

`register::<T>()` 가 4 함수 포인터 모두 채움. `register_raw(name, ser, deser)` 는 remove / default_bytes 에 no-op (`|_, _| false`, `|| Vec::new()`) 를 채움 — 외부 타입은 Inspector 의 AddComponent 후보로 자동 노출되지 않음.

### 3.3 EntityAllocator::restore_at_generation

```rust
impl EntityAllocator {
    /// Re-activate a previously deallocated slot at the same generation it
    /// had before deallocation. Returns false if the slot is currently alive,
    /// out of range, or has moved past `target_generation`.
    ///
    /// Used by `EditorCommand::Despawn` undo to revive the exact same
    /// `Entity` handle (id + gen) the user had before despawn.
    pub fn restore_at_generation(&mut self, id: u32, target_generation: u32) -> bool {
        let Some(entry) = self.entries.get_mut(id as usize) else { return false; };
        if entry.alive { return false; }
        // After deallocate(), entry.generation == original + 1. So restoring
        // to `target_generation` requires entry.generation == target + 1.
        // If somebody reallocated and re-deallocated the slot in between,
        // entry.generation has moved further forward → restore impossible.
        if entry.generation != target_generation.wrapping_add(1) { return false; }
        entry.generation = target_generation;
        entry.alive = true;
        // Walk free_list and remove the matching id (preserve relative order
        // of other free slots so subsequent allocates are deterministic).
        if let Some(pos) = self.free_list.iter().position(|&x| x == id) {
            self.free_list.remove(pos);
        }
        self.alive_count += 1;
        true
    }
}
```

`World::restore_at_generation(id, gen)` 는 그대로 위임. 컴포넌트 storage 들은 entity slot 의 sparse-set 에서 이미 비어 있으므로 별도 cleanup 불필요 (despawn 시 모든 storage 가 remove_entity 했음).

### 3.4 EditorCommand 추가 + apply_command

```rust
pub enum EditorCommand {
    // ... 기존 8 ...
    AddComponent { entity: Entity, name: String, data: Vec<u8> },
    RemoveComponent { entity: Entity, name: String },
}

// apply_command branch
EditorCommand::AddComponent { entity, name, data } => {
    if !world.is_alive(*entity) { return Err(EditorError::EntityNotAlive(*entity)); }
    let entry = registry.find(name).ok_or_else(|| EditorError::Generic(
        format!("AddComponent: unknown component name `{}`", name)
    ))?;
    (entry.deserialize)(world, *entity, data).map_err(|e| EditorError::Parse(e.to_string()))?;
    Ok(*entity)
}
EditorCommand::RemoveComponent { entity, name } => {
    if !world.is_alive(*entity) { return Err(EditorError::EntityNotAlive(*entity)); }
    let entry = registry.find(name).ok_or_else(|| EditorError::Generic(
        format!("RemoveComponent: unknown component name `{}`", name)
    ))?;
    (entry.remove)(world, *entity);  // no-op-safe if component absent
    Ok(*entity)
}
```

`EditorError::Generic` variant 가 이미 존재 (M4 표준화).

### 3.5 Inverse 확장

```rust
enum Inverse {
    // ... 기존 5 ...
    /// Undo AddComponent: remove by name.
    RemoveComponentByName { entity: Entity, name: String },
    /// Undo RemoveComponent: re-add with prior bytes (None = no prior component).
    AddComponentByBytes { entity: Entity, name: String, data: Option<Vec<u8>> },
    /// Undo Despawn: restore entity slot + re-add every captured component.
    RestoreEntity { entity: Entity, components: Vec<(String, Vec<u8>)> },
}
```

#### capture_inverse 새 분기

```rust
EditorCommand::AddComponent { entity, name, .. } => {
    // Inverse is RemoveComponent (we don't care about pre-bytes — Add overwrites).
    Some(Inverse::RemoveComponentByName { entity: *entity, name: name.clone() })
}
EditorCommand::RemoveComponent { entity, name } => {
    // Capture prior bytes so undo can re-attach the exact same component value.
    let prior = self.registry.find(name).and_then(|entry| (entry.serialize)(&self.world, *entity));
    Some(Inverse::AddComponentByBytes { entity: *entity, name: name.clone(), data: prior })
}
EditorCommand::Despawn { entity } => {
    // Snapshot every registered component on the entity before despawn.
    let components: Vec<(String, Vec<u8>)> = self
        .registry
        .entries()
        .iter()
        .filter_map(|e| (e.serialize)(&self.world, *entity).map(|d| (e.name.clone(), d)))
        .collect();
    Some(Inverse::RestoreEntity { entity: *entity, components })
}
```

#### apply_inverse 새 분기

```rust
Inverse::RemoveComponentByName { entity, name } => {
    if let Some(entry) = self.registry.find(name) {
        (entry.remove)(&mut self.world, *entity);
    }
}
Inverse::AddComponentByBytes { entity, name, data } => {
    if let Some(entry) = self.registry.find(name) {
        if let Some(bytes) = data {
            let _ = (entry.deserialize)(&mut self.world, *entity, bytes);
        }
        // None branch: there was no prior component, so undoing RemoveComponent
        // means "leave it absent" — nothing to do.
    }
}
Inverse::RestoreEntity { entity, components } => {
    let restored = self.world.restore_at_generation(entity.id, entity.generation);
    if restored {
        for (name, bytes) in components {
            if let Some(entry) = self.registry.find(name) {
                let _ = (entry.deserialize)(&mut self.world, *entity, bytes);
            }
        }
    }
    // If restore failed (slot reused), silently skip. UI sees the undo
    // returned false and stays disabled.
}
```

### 3.6 dispatch 분기 변경

Despawn 이 더 이상 "history 삭제" 가 아닌 "정상 undoable". 기존 코드:

```rust
EditorCommand::Despawn { .. }
| EditorCommand::SaveScene { .. }
| EditorCommand::LoadScene { .. } => {
    self.undo_stack.clear();
    self.redo_stack.clear();
}
```

→ Despawn 만 떼어내서 정상 undoable 로 push. SaveScene/LoadScene 만 history clear.

새 변형 (AddComponent/RemoveComponent) 는 기존 undoable 그룹에 합류:

```rust
EditorCommand::SetTransform { .. }
| EditorCommand::SetTag { .. }
| EditorCommand::AddChild { .. }
| EditorCommand::RemoveChild { .. }
| EditorCommand::AddComponent { .. }
| EditorCommand::RemoveComponent { .. }
| EditorCommand::Despawn { .. } => {
    if let Some(inv) = pre_inverse {
        self.undo_stack.push((cmd.clone(), inv));
        self.redo_stack.clear();
    }
}
```

### 3.7 Inspector "Add Component" UI

`crates/voltex_editor/src/panels/inspector.rs` 에 추가. 현재 선택된 entity 에 대해:

```
[Add Component ▾]
  ├ + transform
  ├ + tag
  ├ + mesh_handle
  ├ + material_handle
  ├ + bounds
  └ + script
```

각 버튼 hover 시:
- 해당 컴포넌트가 entity 에 이미 있으면 버튼 disabled (gray-out)
- 클릭 시 `session.dispatch(EditorCommand::AddComponent { entity, name, data: default_bytes_for(name) })`

`Remove Component` 는 inspector 의 기존 컴포넌트 라인 옆 [×] 버튼 — 클릭 시 `RemoveComponent` dispatch.

v1 UI 한계: dropdown 이 아니라 컴포넌트 6개 버튼을 세로 나열 (simplest path). 등록 컴포넌트가 더 늘어나면 dropdown 으로 전환 — v1.1 작업.

## 4. 파일 변경 / 신규

| 파일 | 변경 |
|---|---|
| `crates/voltex_ecs/src/component_registry.rs` | Component trait + ComponentEntry 확장. 6 built-in 의 `default_bytes` 오버라이드. |
| `crates/voltex_ecs/src/entity.rs` | `restore_at_generation` 추가 + 단위 테스트 +3 |
| `crates/voltex_ecs/src/world.rs` | `restore_at_generation` 위임 |
| `crates/voltex_editor/src/command.rs` | AddComponent/RemoveComponent variants + apply_command branches + 테스트 +3 |
| `crates/voltex_editor/src/session.rs` | Inverse 3 variants + capture/apply 분기 + Despawn 그룹 이동 + 테스트 +5 |
| `crates/voltex_editor/src/panels/inspector.rs` | Add Component 버튼 그룹 + 테스트 +2 (로직 분리하여) |
| `crates/voltex_editor/CODEMAP.md` + `crates/voltex_ecs/CODEMAP.md` | API 갱신 |
| `docs/dev/STATUS.md` | Cycle 12 entry |
| `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` | Cycle 12 마커 `[✅]` |

## 5. 테스트 (+12 최소 목표, roadmap 명시 +12)

| # | 위치 | 시나리오 |
|---|---|---|
| 1 | `entity::tests::restore_at_generation_revives_dead_slot` | deallocate → restore → is_alive true with original gen |
| 2 | `entity::tests::restore_at_generation_rejects_alive_slot` | 살아있는 slot 에 restore → false, 상태 무변경 |
| 3 | `entity::tests::restore_at_generation_rejects_advanced_generation` | deallocate → 다른 entity 가 같은 슬롯 alloc → 둘 다 deallocate → 첫 entity restore → false |
| 4 | `command::tests::add_component_attaches_with_bytes` | AddComponent { Tag "X" bytes } → Tag::new("X") |
| 5 | `command::tests::remove_component_clears_existing` | Tag 추가 → RemoveComponent → world.get::<Tag> is None |
| 6 | `command::tests::add_unknown_component_returns_err` | 미등록 이름 → EditorError::Generic |
| 7 | `session::tests::add_component_undo_removes_it` | dispatch Add → undo → component absent + undo_depth 0 |
| 8 | `session::tests::remove_component_undo_restores_prior_bytes` | Tag("A") → RemoveComponent → undo → Tag("A") restored |
| 9 | `session::tests::despawn_undo_restores_entity_and_components` | Spawn + Tag + Transform → Despawn → undo → 동일 id+gen + Tag + Transform 복원 |
| 10 | `session::tests::despawn_undo_fails_silently_when_slot_reused` | Despawn → Spawn (같은 id 새 gen) → undo despawn → return false (또는 silent skip), 새 entity 생존 |
| 11 | `session::tests::despawn_no_longer_clears_history` | 이전 Spawn 의 undo_depth 가 Despawn 후에도 유지 |
| 12 | `panels::inspector::tests::add_component_button_dispatches_correct_payload` | UI helper 함수 단위 테스트 (e.g., `add_component_payload_for_name("tag")` 가 빈 vec 반환) |

## 6. Done 기준

- [x] `cargo build --workspace` 클린
- [x] `cargo test --workspace --lib` 통과 + 위 12 신규 테스트 추가
- [x] Despawn 후 undo 시 entity 의 id+generation 동일 + 모든 등록 컴포넌트 복원
- [x] AddComponent / RemoveComponent dispatch + undo + redo 일관성
- [x] STATUS / roadmap 갱신, CODEMAP 의 API 섹션 갱신

## 7. 세션 분할

| 세션 | 작업 |
|---|---|
| **1** | voltex_ecs: Component trait 확장 (`remove` + `default_bytes` default impl) + 6 built-in 오버라이드 + ComponentEntry 필드 2개 추가 + `EntityAllocator::restore_at_generation` + World 위임 + 테스트 6개 (3 entity + 3 component_registry) |
| **2** | voltex_editor: EditorCommand 2 variants + apply_command 분기 + Inverse 3 variants + capture/apply 분기 + Despawn 의 history-clear 제거 + Inspector "Add Component" UI + 테스트 6개 + CODEMAP/STATUS/roadmap |

## 8. 리스크

| # | 위험 | 완화 |
|---|---|---|
| 1 | `restore_at_generation` 의 generation 비교 wrapping_add → overflow corner case (u32::MAX 슬롯) | 테스트 케이스 추가 (deallocate → restore at u32::MAX-1 ok, u32::MAX-1 deallocate → 다른 alloc 으로 0 wrap → restore false) |
| 2 | `default_bytes` 가 잘못된 바이트 → AddComponent 가 deserialize 단계에서 fail | 6 built-in 각각 자체 테스트로 roundtrip 검증 |
| 3 | Despawn snapshot 이 미등록 컴포넌트 누락 → 사용자 컴포넌트가 undo 후 사라짐 | E2 정책과 동일 (silent loss). `register::<T>()` 호출 책임은 사용자. CODEMAP 에 명시 |
| 4 | Inspector 의 "Add Component" 가 이미 존재하는 컴포넌트에 add 호출 → deserialize 가 새 instance 로 덮어씀 | UI 측에서 사전 검사 (이미 있으면 버튼 disabled). 만약 dispatch 되더라도 deserialize 가 World::add 호출 → 덮어쓰기 OK |
| 5 | Inverse 의 byte payload 가 메모리 누적 — 큰 텍스처 컴포넌트가 있으면 undo_stack 비대 | v1 한계로 명시 (텍스처 같은 큰 컴포넌트는 typically Handle 만 컴포넌트에 저장하니 실제 영향 미미) |

## 9. 관련 문서

- `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 12
- `docs/superpowers/specs/2026-05-20-e2-play-stop-lifecycle-design.md` § 미등록 컴포넌트 정책 (E2 와 동일 룰 적용)
- `crates/voltex_ecs/src/component_registry.rs` § M5 표준화 — Component trait 정의
- `crates/voltex_editor/src/session.rs` § Phase 8-6 Undo/Redo 인프라
