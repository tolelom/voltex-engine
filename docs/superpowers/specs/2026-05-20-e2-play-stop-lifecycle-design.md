# Cycle 4 (E2) — Play/Stop 모드 + 시스템 라이프사이클 훅 설계

**작성일**: 2026-05-20
**상위 로드맵**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 4
**Sub-project**: E (Editor v1)
**세션 수**: 3
**의존**: Cycle 1 (C0a), Cycle 2 (C0b), Cycle 3 (E1) — 모두 완료

---

## 1. 목적

Play 버튼 → 현재 World 스냅샷 → 시뮬레이션 동작. Stop → 스냅샷에서 복원. 후속 cycle (S1 스크립팅, audio 등) 이 사용할 라이프사이클 훅 (PlayStart/PlayStop) 인프라 정의. editor_demo 에 RigidBody 떨어지는 데모 추가.

## 2. Scope / Out-of-scope

### 2.1 In-scope

- `voltex_editor::session::EditorMode` enum (Edit, Play)
- `EditorSession::set_mode(mode)` — Edit→Play 시 `serialize_scene_binary` 로 snapshot 저장 후 lifecycle 콜백 호출. Play→Edit 시 lifecycle 콜백 → `World::clear()` → snapshot 복원. 두 전환 모두 undo/redo 클리어.
- `EditorError::ModeMismatch { current: EditorMode }` variant 추가. Play 중에는 dispatch 시 모든 EditorCommand 가 이 에러로 거부.
- `voltex_editor::lifecycle` 신규 모듈 (`LifecycleEvent` enum, `LifecycleCallback` type alias)
- `EditorSession::add_lifecycle(callback)` API
- editor_demo 통합:
  - `voltex_physics` 의존성 추가
  - 시작 시 lifecycle 콜백 등록: PlayStart → cube 엔티티에 `RigidBody::dynamic(1.0)` + `Collider::Sphere { radius: 0.5 }` 부착. ground 엔티티에 `RigidBody::statik()` + `Collider::Box { half_extents }` 부착. PlayStop → 콜백은 no-op (snapshot 복원이 component 정리 처리)
  - Play 모드일 때 매 프레임 `physics_step(&mut session.world, &PhysicsConfig::default())` 호출
  - Toolbar 에 Play/Stop 토글 버튼. Play 모드 진입 시 다른 모든 버튼 (Spawn/Save/Load/Undo/Redo/Move/Rotate/Scale) 비활성화
  - 카메라 조작 (orbit) 은 두 모드 모두 활성
  - Gizmo 드래그 시도는 Play 중 무시
- voltex_editor 테스트 +11 (요구 ≥10)
- `crates/voltex_editor/CODEMAP.md` 갱신 (EditorMode, lifecycle, Play 모드 의미)

### 2.2 Out-of-scope (후속/v1+)

- Pause / Step (1 프레임)
- Play-mode-only 시스템 분리 (현재는 lifecycle 콜백 한 통로만 제공)
- 미등록 컴포넌트 런타임 경고 — CODEMAP 에 docs-only 로 안내 ("RigidBody/Collider 같은 미등록 컴포넌트는 Play→Stop 시 자동 소멸")
- 비주얼 Play indicator (Toolbar 버튼 색만으로 구분)
- Play 도중 카메라 상태 보존/리셋 (계속 그대로 유지)
- Inspector/Hierarchy 패널 Play 중 disabled 처리 — read 만 되고 dispatch 는 ModeMismatch 로 자동 거부되므로 자연스럽게 read-only 동작
- Inspector 슬라이더가 Play 중 dispatch 실패할 때의 사용자 피드백 — silent (반환값 무시)

## 3. 아키텍처

### 3.1 의존성 변경

```
voltex_editor (L3) — 신규: voltex_physics 의존성 추가? 아니면 editor_demo 에만?
```

**결정**: voltex_editor 자체는 voltex_physics 에 의존하지 않는다. editor_demo (example) 가 voltex_physics 의존성 추가. voltex_editor 의 lifecycle 인터페이스는 World 만 받으므로 physics-agnostic.

이유: voltex_editor 가 voltex_physics 에 의존하면 L3 → L2 cross 가 늘어나고, 다른 게임 (editor_demo 외) 이 voltex_editor 만 쓰면서 physics 없이 동작하는 경우를 막는다.

### 3.2 EditorMode + snapshot 기본 동작

```rust
// session.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode { Edit, Play }

pub struct EditorSession {
    pub world: World,
    pub registry: ComponentRegistry,
    pub selected: Option<Entity>,
    pub scene_path: String,
    pub log_path: String,
    pub mode: EditorMode,
    play_snapshot: Option<Vec<u8>>,
    lifecycles: Vec<LifecycleCallback>,
    undo_stack: Vec<...>,
    redo_stack: Vec<...>,
}

impl EditorSession {
    pub fn new(scene_path: impl Into<String>) -> Self {
        // 기존 필드 +
        mode: EditorMode::Edit,
        play_snapshot: None,
        lifecycles: Vec::new(),
    }

    pub fn set_mode(&mut self, target: EditorMode) {
        if self.mode == target { return; }
        match (self.mode, target) {
            (EditorMode::Edit, EditorMode::Play) => {
                // 1. 스냅샷
                self.play_snapshot = Some(
                    voltex_ecs::serialize_scene_binary(&self.world, &self.registry)
                );
                // 2. mode 전환
                self.mode = EditorMode::Play;
                // 3. lifecycle PlayStart 콜백 호출
                self.fire_lifecycle(LifecycleEvent::PlayStart);
                // 4. undo/redo 클리어
                self.undo_stack.clear();
                self.redo_stack.clear();
            }
            (EditorMode::Play, EditorMode::Edit) => {
                // 1. lifecycle PlayStop 콜백 호출 (World 정리는 다음 단계에서)
                self.fire_lifecycle(LifecycleEvent::PlayStop);
                // 2. World 비우고 snapshot 으로 복원
                self.world.clear();
                if let Some(data) = self.play_snapshot.take() {
                    let _ = voltex_ecs::deserialize_scene_binary(
                        &mut self.world, &data, &self.registry,
                    );
                }
                // 3. mode 전환
                self.mode = EditorMode::Edit;
                // 4. undo/redo 클리어
                self.undo_stack.clear();
                self.redo_stack.clear();
            }
            _ => unreachable!(),
        }
    }

    fn fire_lifecycle(&mut self, event: LifecycleEvent) {
        let mut callbacks = std::mem::take(&mut self.lifecycles);
        for cb in &mut callbacks {
            cb(event, &mut self.world);
        }
        self.lifecycles = callbacks;
    }
}
```

**핵심**: snapshot 은 ComponentRegistry 에 등록된 컴포넌트만 보존한다 (Transform/Tag/MeshHandle/MaterialHandle/Bounds). Play 중 추가된 RigidBody/Collider 는 미등록 → snapshot 에 들어가지 않으므로 Stop 시 World::clear → deserialize 과정에서 자동 사라진다.

### 3.3 ModeMismatch dispatch 정책

```rust
// error.rs
pub enum EditorError {
    Generic(String),
    EntityNotAlive(Entity),
    Io(String),
    Parse(String),
    /// Play 모드에서 dispatch 가 거부됨 (v1 정책: Play 중에는 World 가 수정되지 않음).
    ModeMismatch { current: crate::session::EditorMode },
}

// session.rs dispatch
pub fn dispatch(&mut self, cmd: EditorCommand) -> Result<Entity, EditorError> {
    if self.mode != EditorMode::Edit {
        return Err(EditorError::ModeMismatch { current: self.mode });
    }
    // 기존 로직
}
```

순환 의존 회피: `error.rs` 가 `session.rs::EditorMode` 를 참조해야 한다. `EditorMode` 를 `lifecycle.rs` (또는 별도 mod) 로 옮기는 것 vs error 가 session 을 참조하는 것. 후자가 단순. Rust 에서 같은 crate 내 모듈은 자유 참조 가능.

만약 cycle import 문제가 생기면: `EditorMode` 를 `error.rs` 옆 또는 `lifecycle.rs` 에 두고 둘 다 import. 구현 시 확인.

### 3.4 Lifecycle 모듈 (신규)

```rust
// lifecycle.rs
use voltex_ecs::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    PlayStart,
    PlayStop,
}

/// 라이프사이클 이벤트 콜백. EditorSession::add_lifecycle 로 등록.
pub type LifecycleCallback = Box<dyn FnMut(LifecycleEvent, &mut World) + 'static>;
```

`EditorSession::add_lifecycle`:

```rust
impl EditorSession {
    pub fn add_lifecycle<F>(&mut self, callback: F)
    where
        F: FnMut(LifecycleEvent, &mut World) + 'static,
    {
        self.lifecycles.push(Box::new(callback));
    }
}
```

콜백은 등록 순서대로 호출. 콜백이 panic 하면 전체 호출 중단 (정책 단순). PlayStart 의 모든 콜백 실행 완료 후에야 dispatch 가 ModeMismatch 가 되도록 순서 정확히: 콜백 실행 시점에 mode 는 이미 Play 로 설정됨 → 콜백 내부에서 dispatch 시도 시 ModeMismatch (현실적으로 콜백은 World 직접 mutate 하므로 무관).

### 3.5 editor_demo 통합

#### Cargo.toml
```toml
voltex_physics.workspace = true
```

#### setup
```rust
use voltex::physics::{Collider, PhysicsConfig, RigidBody, physics_step};

let mut session = EditorSession::new(...);
// ... seed entities ...

// Stash entity ids for lifecycle callback
let cube1_id = cube1_e;
let cube2_id = cube2_e;
let cube3_id = cube3_e;
let ground_id = ground_e;

session.add_lifecycle(move |event, world| {
    match event {
        LifecycleEvent::PlayStart => {
            for &cube in &[cube1_id, cube2_id, cube3_id] {
                if world.is_alive(cube) {
                    world.add(cube, RigidBody::dynamic(1.0));
                    world.add(cube, Collider::Sphere { radius: 0.5 });
                }
            }
            if world.is_alive(ground_id) {
                world.add(ground_id, RigidBody::statik());
                world.add(ground_id, Collider::Box { half_extents: Vec3::new(5.0, 0.05, 5.0) });
            }
        }
        LifecycleEvent::PlayStop => {
            // No-op — snapshot restore handles cleanup.
        }
    }
});

let physics_config = PhysicsConfig::default();
```

#### per-frame
```rust
if state.session.mode == EditorMode::Play {
    voltex::physics::physics_step(&mut state.session.world, &state.physics_config);
}
```

#### Toolbar
```rust
let in_play = state.session.mode == EditorMode::Play;

if in_play {
    if state.ui.button("Stop") {
        state.session.set_mode(EditorMode::Edit);
        state.session.selected = None;  // 선택 보존 정책: Stop 시 해제
    }
} else {
    if state.ui.button("Play") {
        state.session.set_mode(EditorMode::Play);
    }
    if state.ui.button("Spawn") { ... }
    if state.ui.button("Save") { ... }
    if state.ui.button("Load") { ... }
    if state.ui.button("Undo") { state.session.undo(); }
    if state.ui.button("Redo") { state.session.redo(); }
    if state.ui.button("Move") { state.gizmo_state.set_mode(GizmoMode::Translate); }
    if state.ui.button("Rotate") { state.gizmo_state.set_mode(GizmoMode::Rotate); }
    if state.ui.button("Scale") { state.gizmo_state.set_mode(GizmoMode::Scale); }
}
```

#### Gizmo 드래그 Play 중 차단
```rust
if !in_play && state.left_dragging && !state.gizmo_dragging && ... {
    state.gizmo_state.try_begin_drag(...)
}
```

(gizmo update_drag 도 마찬가지로 in_play 시 skip)

#### Picking Play 중 비활성 (선택만 보존)
스펙상 명시 안 됨 — Play 중 클릭으로 selection 변경 허용. Inspector 가 새 entity 정보 보여줄 수 있음.

## 4. 데이터 흐름

### 4.1 Edit → Play
```
사용자 Play 버튼
  ↓
session.set_mode(Play)
  ↓ serialize_scene_binary(world, registry) → Vec<u8>
  ↓ play_snapshot = Some(bytes)
  ↓ mode = Play
  ↓ for cb in lifecycles: cb(PlayStart, &mut world)
     → 데모 콜백: cube 들에 RigidBody + Collider 부착
  ↓ undo_stack.clear(), redo_stack.clear()
이후 매 프레임:
  if mode == Play:
    physics_step(&mut world, &config)  // 큐브가 떨어짐, 충돌
  BasicRenderer::render(...)           // 변경된 위치로 그림
```

### 4.2 Play → Edit (Stop)
```
사용자 Stop 버튼
  ↓
session.set_mode(Edit)
  ↓ for cb in lifecycles: cb(PlayStop, &mut world)
     → 데모 콜백: no-op (RigidBody/Collider 는 다음 단계에서 어차피 사라짐)
  ↓ world.clear()
  ↓ deserialize_scene_binary(&mut world, snapshot)
     → Transform/Tag/MeshHandle/MaterialHandle/Bounds 복원
     → RigidBody/Collider 는 미등록이므로 복원되지 않음 (정확히 원하는 동작)
  ↓ mode = Edit
  ↓ play_snapshot = None
  ↓ undo/redo 클리어
다음 프레임: 큐브들 원위치
```

### 4.3 Play 중 dispatch 시도
```
사용자 Inspector 슬라이더 / Toolbar 버튼 등
  ↓
session.dispatch(SomeCmd)
  ↓ mode != Edit → return Err(EditorError::ModeMismatch { current: Play })
호출자는 Result 무시 (UI 가 이미 버튼 disabled — 추가 피드백 없음)
```

## 5. 모듈 구조 변경

```
crates/voltex_editor/src/
├── error.rs       # + ModeMismatch variant
├── session.rs     # + mode/play_snapshot/lifecycles 필드, set_mode, add_lifecycle, dispatch 가드
├── lifecycle.rs   # 신규: LifecycleEvent, LifecycleCallback
└── lib.rs         # pub mod lifecycle; pub use lifecycle::*;
```

session.rs 가 ~520줄 (현재) + ~150줄 신규 = ~670줄 → 600줄 한도 근처. 분리 필요 시 `session/mod.rs` + `session/play.rs` 로 split. 일단 한 파일로 작성하고 600 초과시 분리.

## 6. 에러 처리

- `set_mode` 자체는 무에러 (snapshot 직렬화는 panic 안 함 — registered 컴포넌트 deser/ser 가 잘못된 데이터 반환해도 buffer 에 빈 슬롯으로 적재)
- `dispatch` 가 Play 모드에서 `Err(ModeMismatch)` 반환
- snapshot deserialize 실패 시: silent (이미 비워진 World 그대로) — 다음 dispatch 가 EntityNotAlive 등으로 자연스럽게 실패. v1 수용.

## 7. 테스트 (≥10 — 11개)

| # | 파일 | 케이스 |
|---|---|---|
| 1 | session.rs | `set_mode_edit_to_play_stores_snapshot_and_changes_mode` |
| 2 | session.rs | `set_mode_play_to_edit_restores_world_state` |
| 3 | session.rs | `set_mode_same_mode_is_noop` |
| 4 | session.rs | `play_to_edit_drops_components_added_during_play` (RigidBody-like 미등록 컴포넌트는 사라짐) |
| 5 | session.rs | `dispatch_in_play_mode_returns_mode_mismatch` |
| 6 | session.rs | `entering_play_clears_undo_redo_history` |
| 7 | session.rs | `exiting_play_clears_undo_redo_history` |
| 8 | session.rs | `snapshot_preserves_hierarchy` (parent/children) |
| 9 | session.rs | `snapshot_preserves_render_handles` (MeshHandle + MaterialHandle + Bounds) |
| 10 | lifecycle.rs | `play_start_callback_invoked` (콜백 호출 + World 인자 전달) |
| 11 | lifecycle.rs | `play_stop_callback_invoked_and_can_inspect_world` |

추가 가능: 여러 콜백 등록 → 순서대로 호출 — Session 2 에 1개 더 추가 가능.

## 8. 시각 검증 (수동)

editor_demo:
- 시작 시 4개 entity (ground + 3 cube) 정지 상태
- Play 버튼 클릭 → 큐브 3개가 중력으로 떨어지고 ground 위에 멈춤 (Cube2 는 Cube1 의 child 이므로 부모-자식 변환은 어떻게 동작할까... v1 단순화: physics_step 이 자식 Transform 도 직접 mutate, parent-child 관계는 사실상 무의미해짐. 시각 검증으로 OK)
- Stop 버튼 클릭 → 큐브들이 원래 위치로 즉시 복귀
- Play 중 Spawn/Save/Load 등 버튼은 보이지 않거나 disable

## 9. 세션 분할

### Session 1: EditorMode + snapshot core (4 tasks)
- `EditorMode` enum, `play_snapshot` 필드, `set_mode` 기본 동작 (lifecycle 빼고)
- `error.rs::ModeMismatch` variant
- 단위 테스트 7개 (#1, 2, 3, 4, 6, 7, 8)
- 빌드/테스트 깨끗

### Session 2: lifecycle callbacks + ModeMismatch dispatch (3 tasks)
- `lifecycle.rs` 신규
- `EditorSession::add_lifecycle`, `fire_lifecycle`
- `set_mode` 가 lifecycle 호출하도록
- `dispatch` 가 ModeMismatch 반환하도록
- 단위 테스트 4개 (#5, 9, 10, 11)

### Session 3: editor_demo physics demo + UI (2 tasks)
- editor_demo Cargo.toml + voltex_physics 의존성
- 시작 시 lifecycle 등록 (cube → RigidBody/Collider, ground → static + box collider)
- 메인 루프 physics_step in Play
- Toolbar Play/Stop 버튼 + 다른 버튼 disable in Play
- Gizmo drag in Play 차단
- CODEMAP.md 갱신, STATUS.md cycle 4 section, 로드맵 `[✅]`

## 10. 위험 / 미지수

### 10.1 미등록 컴포넌트 silent drop

Play 중 사용자가 직접 World 에 미등록 컴포넌트 (예: Health, AI state) 추가 시 Stop 에서 사라짐. v1 수용: CODEMAP 에 명시. 게임 측은 `ComponentRegistry::register::<MyComponent>()` 로 등록하면 보존됨.

### 10.2 Hierarchy 보존

`serialize_scene_binary` 가 `Parent` 컴포넌트 인덱스로 직렬화 후 deserialize 시 add_child 호출 — 이미 테스트됨 (voltex_ecs::binary_scene tests). Cycle 4 spec 의 테스트 #8 가 round-trip 확인.

### 10.3 Selection / Inspector 상태

- `session.selected` 는 entity id+generation. Stop 후 동일 entity 가 복원되지만 EntityAllocator 가 동일 id+generation 을 보장하지 않을 수 있음 → selected 가 stale entity 가리킬 수 있음.
- 정책: Stop 시 `session.selected = None` (editor_demo 의 UI 측에서 처리).

### 10.4 physics 가 child Transform 직접 mutate

데모의 cube2 가 cube1 의 child. physics_step 이 cube2 의 Transform 을 직접 갱신하면 hierarchy 의 parent-relative 의미가 깨진다. v1 단순화: 데모는 그냥 떨어지는 것만 보여줌. parent-child + physics 통합은 future work.

### 10.5 fire_lifecycle 의 borrow

`self.lifecycles` 와 `&mut self.world` 동시 접근 → take/restore 패턴 (std::mem::take). 콜백이 add_lifecycle 을 재귀 호출하면 take 된 빈 Vec 에 push → 원래 callbacks 복귀 후 덮어쓰지 않게 주의. 보호: take 한 callbacks 와 새로 추가된 callbacks 를 merge:

```rust
fn fire_lifecycle(&mut self, event: LifecycleEvent) {
    let mut callbacks = std::mem::take(&mut self.lifecycles);
    for cb in &mut callbacks {
        cb(event, &mut self.world);
    }
    // Re-attach old callbacks; preserve any that were registered during firing.
    let added_during_fire = std::mem::take(&mut self.lifecycles);
    callbacks.extend(added_during_fire);
    self.lifecycles = callbacks;
}
```

## 11. 변경 영향도

| Crate / Module | 변경 |
|---|---|
| voltex_editor::error | ModeMismatch variant 추가 |
| voltex_editor::session | EditorMode + snapshot + lifecycle + dispatch 가드 |
| voltex_editor::lifecycle | 신규 모듈 |
| voltex_editor::lib | pub mod lifecycle 등록 |
| voltex_ecs / voltex_renderer / voltex_renderer_ecs / voltex_physics | 변경 없음 |
| examples/editor_demo | Cargo.toml + voltex_physics, lifecycle 콜백 등록, physics_step, Toolbar 분기, gizmo drag 차단 |
| 다른 examples | 변경 없음 |

## 12. Done 기준 (로드맵 § Cycle 4)

- [ ] Play → cube 떨어짐 → Stop → 원위치 (시각 검증)
- [ ] 스냅샷 라운드트립이 ComponentRegistry 의 모든 등록 컴포넌트 보존 (테스트 #4, #8, #9)
- [ ] voltex_editor 테스트 +10 이상 (11 작성)
- [ ] 라이프사이클 훅 인터페이스 문서화 (CODEMAP + 이 spec)
- [ ] `cargo build --workspace` 깨끗
- [ ] `cargo test --workspace --lib` 849+ 통과 (838 + 11)
- [ ] STATUS.md, 로드맵 cycle 4 `[✅]` + done commit hash
