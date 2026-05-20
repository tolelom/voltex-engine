# M5: ECS 패턴 표준화 구현 계획

**Goal:** ECS 컴포넌트 등록과 시스템 정의 패턴을 표준화하여 AI/사용자 모두 단일 경로로 작업할 수 있게 한다.

**Architecture:** `Component` trait + 제네릭 `ComponentRegistry::register::<T>()` 도입. `System` trait + `FnMut(&mut World)` blanket impl은 이미 통일되어 있으므로 문서화로 표준화.

**Tech Stack:** Rust 1.90 / voltex_ecs. 외부 의존성 추가 없음 (proc-macro 도입은 yagni, 보류).

**Spec reference:** `docs/superpowers/specs/2026-05-20-engine-restructure-design.md` Layer 4, 성공기준 #4.

---

## Audit 결과 (2026-05-20)

### 컴포넌트 등록

| 경로 | 사용처 |
|---|---|
| `ComponentRegistry::register(name: &str, ser_fn, deser_fn)` | 등록의 유일한 경로 (raw fn pointer 기반) |
| `register_defaults()` | Transform + Tag만 자동 등록 |
| 제네릭 `register::<T>()` | **없음** |
| 게임 측 컴포넌트 등록 | 0건 (engine built-ins만 직렬화 대상) |

**문제점**:
1. 등록할 때 컴포넌트 타입과 직렬화 함수가 분리되어 있어 잘못 짝지을 위험
2. 컴포넌트 이름이 등록 호출자에 흩어짐 (단일 정의 위치 없음)
3. AI/사용자가 "Component를 추가하려면 어떻게?" 질문에 대해 일관된 답을 줄 수 없음

### 시스템 정의

| 패턴 | 정의 |
|---|---|
| `System` trait | `fn run(&mut self, world: &mut World)` |
| `FnMut(&mut World)` blanket impl | 클로저 자동 변환 |
| `Scheduler::add::<S: System>(s)` | 순차 실행 |
| `DependencyScheduler` + `SystemAccess` | 병렬 그룹 빌드 (read/write 의존성) |

**현재 사용 현황**:
- `Scheduler::new()` / `Scheduler::add` 호출하는 코드: **테스트 내부 외 0건** (예제/게임 모두 `world.query()` 직접 호출)
- `DependencyScheduler::new()` 호출하는 코드: **테스트 내부 외 0건**

**해석**: 시스템 정의 자체는 이미 1-trait 1-blanket impl로 통일되어 있다. 두 스케줄러가 공존하지만 어느 쪽도 외부에서 사용되지 않아 충돌 없음. 향후 사용 시 표준을 명시하면 됨.

---

## 표준화 결정

### 1. Component trait (신규)

```rust
// crates/voltex_ecs/src/component_registry.rs

/// 직렬화 가능한 컴포넌트가 구현하는 trait.
/// 이름과 ser/deser fn을 컴포넌트 타입 자체에 묶어 단일 정의 위치를 만든다.
pub trait Component: 'static + Sized {
    /// 직렬화 포맷에서 사용할 이름. 변경 시 기존 .vscn 파일 호환 깨짐.
    const NAME: &'static str;

    /// World에서 본 컴포넌트를 바이트로 직렬화. 컴포넌트가 없으면 None.
    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>>;

    /// 바이트를 디시리얼라이즈해 entity에 add.
    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()>;
}
```

### 2. 제네릭 register API

```rust
impl ComponentRegistry {
    /// 표준 경로: Component trait를 구현한 타입을 한 줄로 등록.
    pub fn register<T: Component>(&mut self) {
        self.entries.push(ComponentEntry {
            name: T::NAME.to_string(),
            serialize: T::serialize,
            deserialize: T::deserialize,
        });
    }

    /// Raw 경로: trait 구현이 불가능한 외부 타입을 위해 보존 (legacy/escape hatch).
    pub fn register_raw(&mut self, name: &str, ser: SerializeFn, deser: DeserializeFn) {
        self.entries.push(ComponentEntry { /* ... */ });
    }
}
```

**Backward compat 전략**:
- 기존 `register(name, ser, deser)` 시그니처를 갖는 메서드를 `register_raw`로 이름 변경.
- 새 제네릭 `register::<T>()`가 표준 경로가 됨.
- 호출자 영향: `register_defaults()` 내부만 수정. 외부에서 raw `register(...)` 직접 호출 사용처는 검색 결과 없음.

### 3. Transform/Tag 마이그레이션

`Transform`과 `Tag`에 `Component` impl 추가. 기존 free function (`serialize_transform` 등)은 trait impl로 흡수.

```rust
impl Component for Transform {
    const NAME: &'static str = "transform";
    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> { /* ... */ }
    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> { /* ... */ }
}

impl Component for Tag {
    const NAME: &'static str = "tag";
    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> { /* ... */ }
    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> { /* ... */ }
}
```

`register_defaults`는 새 경로 사용:

```rust
pub fn register_defaults(&mut self) {
    self.register::<Transform>();
    self.register::<crate::scene::Tag>();
}
```

### 4. 시스템 정의 표준 (문서화만)

코드 변경 없음. `CLAUDE.md` 공통 패턴 인덱스에 다음을 명시:

> **시스템 정의 표준**:
> - 시스템은 `fn(world: &mut World)` 함수 또는 클로저로 정의한다. blanket impl이 있어 자동으로 `System` trait를 만족한다.
> - 단일 스레드 순차 실행: `Scheduler::new().add(sys_a).add(sys_b).run_all(&mut world)`.
> - 의존성 기반 병렬 그룹: `DependencyScheduler` + `SystemAccess::new("name").read::<T>().write::<U>()`.
> - 외부 의존(렌더링/오디오 등)이 있는 시스템은 클로저 캡처보다 명시적 함수 + 인자 전달을 선호.

---

## 작업 순서 (단일 atomic commit)

M5는 단일 crate (voltex_ecs) 안에서 끝나므로 마일스톤 1개 = commit 1개.

- [ ] **Step 1**: `Component` trait 정의 + `register::<T>()` + `register_raw` 추가 (component_registry.rs)
- [ ] **Step 2**: Transform에 Component impl 추가 (transform.rs 또는 component_registry.rs 잔존)
- [ ] **Step 3**: Tag에 Component impl 추가 (scene.rs 또는 component_registry.rs 잔존)
- [ ] **Step 4**: `register_defaults` 새 경로 사용으로 변경
- [ ] **Step 5**: 기존 테스트 통과 확인 + 새 제네릭 경로 테스트 추가
- [ ] **Step 6**: voltex_ecs/CODEMAP.md 갱신 (Component trait 명시)
- [ ] **Step 7**: 루트 CLAUDE.md 공통 패턴 인덱스 갱신 (등록 + 시스템 정의)

각 step 후 `cargo build -p voltex_ecs` + `cargo test -p voltex_ecs --lib`.

---

## 검증 게이트

```bash
cargo build --workspace
cargo test --workspace --lib            # 760+ 통과
cargo build --examples
cargo clippy --workspace -- -D warnings
```

추가 검증:
- `grep -r "register_defaults" crates/` → 호출자 전부 유지 (signature 무변경)
- `grep -rn "Component for" crates/voltex_ecs/src/` → Transform + Tag impl 존재
- voltex_editor 의 `EditorSession::new` 변경 없이 동작

---

## 리스크 및 완화

| 리스크 | 영향 | 완화 |
|---|---|---|
| `register` → `register_raw` 이름 변경이 외부 호출 깨뜨림 | 컴파일 에러 | grep 결과 외부 호출 0건 확인됨. 그래도 `register_raw`만 노출하고 `register::<T>()`를 새로 채움 |
| Transform/Tag의 free fn 제거 시 다른 곳에서 직접 호출하던 곳 | 컴파일 에러 | free fn은 모두 module-private (`fn`, 아님 `pub fn`). 외부 호출 불가능 — 안전 |
| .vscn 직렬화 포맷 변경 | 기존 씬 깨짐 | `T::NAME` = 기존 문자열과 동일 ("transform", "tag"). 바이트 포맷 무변경 |

---

## Out of Scope

- `#[derive(Component)]` 매크로 도입 — proc-macro crate가 새로 필요. yagni 원칙으로 보류.
- `Scheduler`/`DependencyScheduler` 통합 — 두 스케줄러가 공존하지만 외부 사용처 0건이므로 통합 필요성 없음. 향후 실제 사용 시 결정.
- 게임/example 측 컴포넌트의 register::<T>() 마이그레이션 — 현재 등록 대상은 Transform/Tag 뿐이고, 게임 측 컴포넌트(RigidBody 등)는 직렬화 대상 미정.

---

## 후속 작업

M5 완료 후 M6 (voltex facade crate)으로 진행. M6에서 `voltex::prelude` 가 새 `Component` trait를 re-export하도록 추가.
