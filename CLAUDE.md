# Voltex Engine

Rust 게임 엔진 프로젝트. wgpu 28.0 + winit 0.30 기반.

## 작업 현황
- `docs/STATUS.md` — 완료된 Phase, crate 구조, 테스트 현황, 다음 작업
- `docs/DEFERRED.md` — 간소화/미뤄진 항목 목록

## 스펙
- `docs/superpowers/specs/2026-03-24-voltex-engine-design.md` — 전체 엔진 설계
- `docs/superpowers/specs/2026-05-20-engine-restructure-design.md` — AI 친화 재구조화 설계

## 구현 계획
- `docs/superpowers/plans/` — 각 Phase별 상세 구현 계획

## 빌드/테스트
```bash
cargo build --workspace
cargo test --workspace
```

## 규칙
- Cargo path: `export PATH="$HOME/.cargo/bin:$PATH"` (Windows bash)
- wgpu 28.0 API: `immediate_size`, `multiview_mask`, `TexelCopyTextureInfo` 등 28.0 전용 필드 사용
- WGSL vec3 alignment: Rust struct에서 vec3 뒤에 padding 필요 (16바이트 정렬)
- max_bind_groups=4 (group 0~3). 리소스를 합쳐서 4개 이내로 유지
- Dynamic UBO 패턴: per-entity uniform은 aligned staging buffer + dynamic offset 사용
- 기존 예제(triangle, model_viewer 등)는 mesh_shader.wgsl(Blinn-Phong) 사용 — PBR 변경에 영향 없음

## Crate 의존성 그래프

각 crate의 자세한 책임/API는 `crates/<crate>/CODEMAP.md` 참고.

```
L0 (no voltex deps):
  voltex_math       — Vec/Mat/AABB/Ray
  voltex_asset      — Handle, AssetStorage, AssetLoader, FileWatcher
  voltex_net        — UDP, Packet, Reliable, Snapshot, Interpolation

L1 (depends on math only):
  voltex_platform   → voltex_math   (winit window/input/timer)
  voltex_ecs        → voltex_math, voltex_asset   (World, Entity, Component, Scheduler, Handle 컴포넌트)
  voltex_audio      → voltex_math   (WAV/OGG, WASAPI, 3D, mixer)

L2:
  voltex_ai         → voltex_math, voltex_ecs        (NavMesh, A*, steering)
  voltex_physics    → voltex_math, voltex_ecs        (Collider, RigidBody, Raycast)
  voltex_script     → voltex_math, voltex_ecs        (Lua 5.4, sandbox)
  voltex_renderer   → voltex_math, voltex_platform   (wgpu, PBR, RT, post-process)

L3:
  voltex_renderer_ecs → voltex_math, voltex_ecs, voltex_asset, voltex_renderer
                        (RenderAssets, BasicRenderer, collect_render_items — ECS-driven 렌더링 브릿지)
  voltex_editor       → voltex_math, voltex_ecs, voltex_renderer
                        (IMGUI, dock, viewport, inspector, TTF)

L4 (facade):
  voltex            → 모든 sub-crate를 모듈로 re-export + prelude
                      (사용자 진입점, 신규/외부 코드는 여기부터 import)
```

**변경 영향도**: L0 변경은 거의 모든 crate에 영향 (특히 voltex_math). L3 변경은 영향 적음. 변경 전 의존 그래프를 따라 영향 범위 확인.

## 도메인 용어집

- **Entity**: ECS의 식별자. `(id: u32, generation: u32)` 쌍. 재사용 안전.
- **Component**: 데이터 단위. Transform, Tag, Parent 등. `SparseSet<T>`에 저장.
- **System**: 게임 로직 단위. `Scheduler`가 순차 실행. (의존성 기반 병렬은 `DependencyScheduler`)
- **World**: 모든 엔티티/컴포넌트를 담는 컨테이너 (`voltex_ecs::World`).
- **Archetype**: signature 기반 entity 그룹 저장소 (`voltex_ecs::Archetype`).
- **Handle<T>**: 에셋 핸들. generation 기반 안전 참조 (`voltex_asset`).
- **Collider**: 충돌 형상. Sphere/Box/Capsule/ConvexHull/Mesh.
- **RigidBody**: 강체. 질량 + 속도 + 각속도 + 마찰 + sleep 상태.
- **NavMesh**: 폴리곤 기반 내비메시 (`voltex_ai`).
- **DockTree**: 에디터 도킹 트리 (binary split layout) (`voltex_editor`).
- **EditorCommand**: 에디터 작업 단위. Spawn/Despawn/SetTransform 등. EditorSession에 dispatch.
- **CODEMAP.md**: 각 crate 루트의 메타데이터 파일. 책임/API/의존성/주의점.

## 공통 패턴 인덱스

### 에러 처리 (M4 표준화 완료)
모든 crate가 `voltex_<crate>::<Crate>Error` + `Result<T>` alias 사용 (thiserror 기반).
- 각 crate `src/error.rs` 에 `<Crate>Error` enum + `Generic(String)` variant + `#[from] std::io::Error`
- `From<String> for <Crate>Error`, `From<&str> for <Crate>Error` 자동 변환 impl 제공
- crate 간 에러 전파: `?` 연산자 + `Generic` variant로 String 메시지 흡수 (`.into()` 또는 `.to_string().into()`)
- 에러 타입이 존재하는 crate: asset, net, ecs, audio, ai, script, renderer, editor
- 에러 타입이 없는 crate (Result<T,String> 미사용): math, platform, physics
- voltex_editor: `EditorError`가 기존 `CommandError`를 흡수 (Generic/EntityNotAlive/Io/Parse variants, PartialEq 유지)

### 컴포넌트 등록 (M5 표준화 완료)
직렬화 가능한 컴포넌트는 `voltex_ecs::Component` trait를 구현하고 `ComponentRegistry::register::<T>()` 로 등록한다.

```rust
use voltex_ecs::{Component, ComponentRegistry, World, Entity};

#[derive(Debug, Clone)]
struct Health(u32);

impl Component for Health {
    const NAME: &'static str = "health";  // 변경 시 기존 씬 파일 호환 깨짐
    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let h = world.get::<Health>(entity)?;
        Some(h.0.to_le_bytes().to_vec())
    }
    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> voltex_ecs::Result<()> {
        let v = u32::from_le_bytes(data[..4].try_into().map_err(|_| "Health too short")?);
        world.add(entity, Health(v));
        Ok(())
    }
}

let mut registry = ComponentRegistry::new();
registry.register::<Health>();           // 표준 경로
registry.register_defaults();            // Transform + Tag (engine built-in)
```

- **Escape hatch**: trait 구현이 불가능한 외부 타입은 `registry.register_raw(name, ser, deser)` 사용.
- `register_defaults()` 는 Transform, Tag 두 built-in을 한 번에 등록.
- 등록 안 된 컴포넌트는 `serialize_scene_json` / `serialize_scene_binary` 결과에 포함되지 않는다.

### 시스템 정의 (M5 표준화 완료)
시스템은 `fn(world: &mut World)` 함수 또는 동등한 클로저로 정의한다. `voltex_ecs::System` blanket impl로 자동으로 trait 만족.

```rust
use voltex_ecs::{Scheduler, World, Transform};

fn move_system(world: &mut World) {
    for (_, t) in world.query::<Transform>().collect::<Vec<_>>() {
        // ...
    }
}

let mut scheduler = Scheduler::new();
scheduler.add(move_system);                      // 함수 포인터
scheduler.add(|w: &mut World| { /* ... */ });    // 클로저도 OK
scheduler.run_all(&mut world);
```

- **순차 실행**: `Scheduler::new().add(sys_a).add(sys_b).run_all(&mut world)`.
- **병렬 그룹 빌드**: `DependencyScheduler` + `SystemAccess::new("name").read::<T>().write::<U>()` 로 충돌 분석 후 그룹화. 현재는 그룹 빌드까지만 제공 (실제 병렬 실행은 future work).
- 클로저 캡처보다 명시적 함수 + 인자 전달을 선호 (test/재사용 용이).

### 직렬화
- JSON: `voltex_ecs::serialize_scene_json` / `deserialize_scene_json`
- VSCN binary: `voltex_ecs::serialize_scene_binary` / `deserialize_scene_binary`
- VNAV (navmesh): `voltex_ai`의 navmesh 직렬화

### 에디터 작업
- 사용자 액션은 `EditorCommand`로 표현
- `EditorSession::dispatch(command)` 호출
- World 직접 변형은 가급적 피하고 command 경유 (jsonl 로그 + 자동 저장 혜택)

### Facade crate 사용 (M6 표준화 완료)
신규/외부 코드는 `voltex` facade crate를 단일 진입점으로 사용한다.

```rust
use voltex::prelude::*;                       // Vec3, World, Entity, Transform, ...
use voltex::renderer::{GpuContext, pipeline}; // 도메인 특화는 명시 경로
use voltex::physics::{Collider, RigidBody};
```

- **prelude 포함**: Vec2/3/4, Mat4, AABB, Ray, Entity, World, Transform, Component, ComponentRegistry, Parent, Children, VoltexWindow, WindowConfig, InputState, GameTimer, Handle, Assets
- **prelude 미포함 (의도적)**: GpuContext, RigidBody, Collider, LuaState, EditorSession 등 도메인 특화 타입 — 명시 경로 사용
- 각 sub-crate의 `Result<T>` alias는 이름 충돌로 prelude 제외 — `voltex::ecs::Result` 형식 명시
- 기존 코드는 `voltex_<crate>::Type` 경로 그대로 사용해도 무방 (facade는 추가 레이어, 강제 아님)

## 변경 워크플로 (AI 작업 기준)

1. **위치 파악**: 수정할 기능이 어느 crate에 있는지 `CLAUDE.md` 의존성 그래프 → 해당 `CODEMAP.md` → 모듈 구조에서 찾기
2. **변경 영향 점검**: `CODEMAP.md`의 "변경 시 주의점" 확인
3. **수정**: 단일 책임 원칙, 600줄 한도 (v1.0.2 에서 4 파일 재분리 완료, 회귀 발생 시 분리)
4. **빌드 확인**: `cargo build -p <crate>` (해당 crate만) 후 `cargo build --workspace`
5. **테스트 확인**: `cargo test -p <crate> --lib` 후 `cargo test --workspace --lib`
6. **CODEMAP 갱신**: 공개 API/모듈 구조가 바뀌었으면 해당 `CODEMAP.md` 갱신
7. **Commit**: 단일 책임 단위로 commit (모듈 분리/에러 마이그레이션은 atomic)
