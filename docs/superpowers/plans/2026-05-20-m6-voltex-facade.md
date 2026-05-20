# M6: voltex facade crate 구현 계획

**Goal:** 사용자/AI가 `voltex_*` 11개 crate를 개별 import하는 대신 단일 `voltex` crate를 통해 접근할 수 있게 한다.

**Architecture:** 새 crate `crates/voltex/` 추가. lib.rs는 모든 sub-crate를 모듈로 re-export. `prelude` 모듈에 자주 쓰는 타입만 신중히 선별. `App` 추상화는 yagni — 도입은 실제 필요 시점으로 보류.

**Tech Stack:** Rust workspace. 외부 의존성 추가 없음.

**Spec reference:** `docs/superpowers/specs/2026-05-20-engine-restructure-design.md` Layer 5, 성공기준 #5 + #6.

---

## Audit 결과 (2026-05-20)

### 잠재적 이름 충돌

| 이름 | 위치 | 충돌 가능성 |
|---|---|---|
| `Result<T>` | 8개 crate의 `<Crate>Error` alias | **prelude 포함 불가** — 개별 `voltex::ecs::Result` 형식으로 접근 |
| `error` | 모든 crate의 module | 모듈명이라 충돌 없음 |
| `Ray` | voltex_math::Ray (struct) / voltex_physics::ray (module, fn만) | math의 Ray만 prelude |
| `Plane` | voltex_renderer::frustum::Plane | prelude 미포함 |
| `Quat` | voltex_renderer::animation::Quat | 도메인별이라 미포함 |
| `Listener`, `Reverb`, `Echo` | voltex_audio | 미포함 |
| `Snapshot` | voltex_net | 미포함 (게임 도메인) |
| `Component` | voltex_ecs (M5 신규 trait) | 충돌 없음 |

### 예제 import 패턴 (현재)

`examples/triangle/src/main.rs` (현재):
```rust
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::{GpuContext, pipeline, vertex::Vertex};
```

목표 (M6 이후):
```rust
use voltex::prelude::*;                     // VoltexWindow, WindowConfig, InputState, GameTimer
use voltex::renderer::{GpuContext, pipeline, vertex::Vertex};
```

`pipeline`과 `vertex::Vertex`는 너무 도메인 특화라 prelude에 미포함. 명시적 경로 유지.

---

## 설계 결정

### 1. crate 구조

```
crates/voltex/
├── Cargo.toml           — 모든 voltex_* sub-crate를 workspace dep로 의존
├── CODEMAP.md
└── src/
    └── lib.rs           — re-export + prelude (~60줄 예상)
```

의존성 그래프상 voltex는 **L4** (최상위, 모든 crate에 의존).

### 2. lib.rs 내용

```rust
//! voltex — Rust 게임 엔진 facade crate.
//!
//! 모든 sub-crate를 모듈 경로로 노출하며, 자주 쓰는 타입은 `prelude`에 모아둔다.

pub use voltex_math as math;
pub use voltex_ecs as ecs;
pub use voltex_renderer as renderer;
pub use voltex_physics as physics;
pub use voltex_audio as audio;
pub use voltex_asset as asset;
pub use voltex_ai as ai;
pub use voltex_net as net;
pub use voltex_script as script;
pub use voltex_editor as editor;
pub use voltex_platform as platform;

pub mod prelude {
    // 수학 — 거의 모든 게임 코드에 등장
    pub use voltex_math::{Vec2, Vec3, Vec4, Mat4, AABB, Ray};

    // ECS — 게임 로직의 기본 단위
    pub use voltex_ecs::{
        Entity, World, Transform,
        Component, ComponentRegistry,
        Parent, Children,
    };

    // 플랫폼 — 메인 루프
    pub use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};

    // 에셋 — 핸들 패턴
    pub use voltex_asset::{Handle, Assets};
}
```

### 3. prelude 포함 기준

다음 모두를 만족하는 타입만 포함:
- 거의 모든 게임 코드에서 등장
- 이름 충돌이 없거나 prelude 사용자에게 직관적
- 도메인 specific하지 않음 (rendering/audio/physics 세부는 명시 import)

**미포함 (의도적)**:
- `Quat`: voltex_math에 없음. renderer::animation::Quat 존재하지만 도메인 특화 — `use voltex::renderer::Quat;` 명시
- `EditorCommand` / `EditorSession`: 에디터 작업자만 사용
- `Scheduler` / `DependencyScheduler`: 실제 사용처 없음 (M5 audit 결과)
- `RigidBody` / `Collider`: 물리 사용자만
- `GpuContext` / `Mesh`: 렌더링 사용자만

### 4. `App` 추상화 — 도입 보류

spec에 언급되었으나:
- 현재 모든 예제가 winit `ApplicationHandler`를 직접 구현 (각 예제마다 다른 패턴)
- "최소 추상화" 원칙 + yagni → 실제로 5개 이상의 예제가 동일한 boilerplate를 쓰고 있을 때 도입

M6에서는 facade re-export + prelude만. App은 별도 작업으로 분리.

### 5. examples/triangle 마이그레이션 (성공기준 #6)

spec 성공기준 #6: "`examples/triangle`이 `use voltex::prelude::*;` 1줄로 시작 가능".

현재 triangle은:
- platform 4종 (VoltexWindow, WindowConfig, InputState, GameTimer) → **prelude로 다 잡힘**
- renderer 3종 (GpuContext, pipeline, vertex::Vertex) → prelude 미포함이므로 `use voltex::renderer::{GpuContext, pipeline, vertex::Vertex};` 1줄로 흡수
- wgpu, winit → 외부, 그대로

기존 2개 voltex_* import → voltex facade 2개 import로 변경. 1줄로 줄어들지는 않지만 voltex crate 1개에만 의존.

---

## 작업 순서

- [ ] **Step 1**: M6 plan 작성 (이 문서)
- [ ] **Step 2**: `crates/voltex/Cargo.toml` 작성 — workspace 의존성 11개 활성화
- [ ] **Step 3**: `crates/voltex/src/lib.rs` 작성 — re-export + prelude
- [ ] **Step 4**: `crates/voltex/CODEMAP.md` 작성
- [ ] **Step 5**: 루트 `Cargo.toml` 수정 — workspace.members + workspace.dependencies
- [ ] **Step 6**: `cargo build -p voltex` + `cargo test -p voltex` 통과 확인 (테스트 0개여도 통과 OK)
- [ ] **Step 7**: `examples/triangle/Cargo.toml`을 voltex deps로 전환
- [ ] **Step 8**: `examples/triangle/src/main.rs`의 import를 facade 경로로 마이그레이션
- [ ] **Step 9**: 전체 `cargo build --workspace` + `cargo test --workspace --lib` 통과 (763개)
- [ ] **Step 10**: CLAUDE.md 갱신 — 의존성 그래프에 voltex L4 추가, 공통 패턴 인덱스에 facade 사용법 추가
- [ ] **Step 11**: STATUS.md 갱신 — M6 완료 표시
- [ ] **Step 12**: commit 3개 (plan / facade crate / triangle 마이그레이션) + push

---

## 검증 게이트

```bash
cargo build --workspace                        # voltex 포함 11+1 crate 빌드
cargo test --workspace --lib                   # 763개 통과
cargo build -p triangle                        # facade 경로로 빌드 통과
cargo doc -p voltex --no-deps                  # 문서 무경고 (성공기준 보조)
```

---

## 리스크 및 완화

| 리스크 | 영향 | 완화 |
|---|---|---|
| prelude의 이름 충돌이 사용자 코드 깨뜨림 | 컴파일 에러 | 사전 audit으로 충돌 가능 타입 모두 prelude 제외. 명시적 path 권장 |
| voltex crate 추가로 빌드 시간 증가 | 미미 (단순 re-export) | re-export만 — incremental compile에 영향 거의 없음 |
| triangle 마이그레이션이 다른 예제도 압박 | scope creep | M6에선 triangle 1개만. 나머지는 M7 또는 사용자 시점에 |
| App 미도입으로 spec 부분 미충족 | spec 완수도 ↓ | spec도 "최소 추상화" 명시. App 도입은 실제 필요 시점으로 분리 — 별도 작업 |

---

## Out of Scope

- `voltex::App` 추상화 — yagni 보류
- 나머지 예제 11개의 facade 마이그레이션 — 사용자가 새 코드 작성 시 자연스럽게 전환
- `voltex::prelude` 확장 — 실제 사용 패턴이 쌓인 후 결정
- `voltex_renderer`의 sub-crate 분해 (이미 spec out of scope)

---

## 후속 작업

M6 완료 후 M7 (최종 검증):
- `cargo doc` 무경고
- 다른 예제 1-2개 추가 facade 마이그레이션 (선택)
- 전체 CODEMAP 일관성 audit
- STATUS.md / DEFERRED.md 최종 정리
