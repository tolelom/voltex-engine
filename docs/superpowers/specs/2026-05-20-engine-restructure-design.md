# Voltex 엔진 AI 친화 재구조화 설계

**작성일**: 2026-05-20
**목표 마일스톤**: 게임 제작 가능 수준 도달 전, 코드베이스 구조를 AI 어시스턴트와의 협업에 최적화

## 배경

Voltex는 11개 crate, 760개 테스트, 13개 예제를 갖춘 Rust 게임 엔진이다.
렌더링/물리/오디오/네트워킹/AI/스크립트/에디터 핵심 시스템이 거의 완성된 상태이지만,
사용자가 AI 어시스턴트(Claude 등)와 함께 게임을 제작하기 시작했을 때
구조적으로 어떤 문제가 발생할지 미리 식별하고 대응할 필요가 있다.

본 설계는 "실제 게임 제작 단계 진입 전"에 코드베이스를 AI가 자율적으로
탐색·이해·수정할 수 있는 구조로 정비하는 것을 목적으로 한다.

## 현재 구조 Audit 결과 (2026-05-20)

| 항목 | 상태 |
|---|---|
| crate별 메타데이터 (CODEMAP.md/README.md) | **0개** — 전부 없음 |
| 800줄+ 파일 | 4개 (vorbis 1493, gltf 982, jpg 959, deferred_pipeline 905) |
| 500-800줄 파일 | 11개 (widgets, navmesh_builder, world, solver, dock, gjk, wasapi, ttf_parser, texture, state, navmesh) |
| 에러 처리 라이브러리 | `thiserror`/`anyhow` 미사용 — crate별 자체 방식 |
| voltex_renderer 파일 수 | 약 80개 — 비대 (렌더링 + 이미지 파싱 혼재) |
| 최상위 facade crate | 없음 — 사용자가 11개 crate를 직접 import |

### AI 작업 관점에서의 구체적 영향

1. 새 crate 진입 시 `lib.rs`부터 추측 시작 — 컨텍스트 낭비
2. `vorbis.rs` 1493줄은 단일 Read(2000줄 한도) 가능하나, 동시에 다른 파일까지 보기 어려움
3. 에러 타입이 crate마다 달라 통합 에러 처리 불가
4. `voltex_renderer`가 비대해서 변경 영향도 추적 어려움
5. 일관성 없는 패턴 — 같은 일을 crate마다 다르게 처리할 위험

## 목표 및 성공 기준

### 핵심 목표

AI 어시스턴트가 코드베이스에서 자율적으로 탐색·이해·수정할 수 있는 구조를 만든다.

### 성공 기준

모두 달성해야 한다:

1. 모든 crate가 `CODEMAP.md`를 가진다 (책임, API, 의존성, 변경 시 주의점 명시)
2. 모든 `.rs` 파일이 **600줄 이하**
3. 모든 crate가 통일된 에러 타입 사용 (`thiserror` 기반)
4. ECS 컴포넌트/시스템 등록 패턴 표준화
5. 최상위 `voltex` facade crate 존재 (단일 진입점)
6. 테스트 760개 전부 통과 (regression 0)
7. 모든 examples 빌드/실행 정상

### 측정 가능한 AI 작업 효율 지표

- "X 기능을 찾아라" → CODEMAP에서 위치 1회 조회 가능
- 파일 1개 Read로 컨텍스트 잡힘 (현재 vorbis.rs는 부분 Read 필요)
- 에러 처리 코드가 어느 crate에서든 동일 패턴

## 아키텍처 (5개 레이어)

### Layer 1: 메타데이터 (CODEMAP)

각 crate 루트에 `CODEMAP.md` 추가. 고정 템플릿:

```markdown
# voltex_<crate>

## 책임 (한 줄)
이 crate가 해결하는 문제

## 공개 API 진입점
- `Type` — 한 줄 설명 (src/file.rs)
- `function()` — 한 줄 설명

## 모듈 구조
- mod_a — 책임
- mod_b — 책임

## 의존성
- 사용하는 voltex crate: voltex_math
- 외부: wgpu, winit

## 변경 시 주의점
- 이 crate를 만지면 깨질 수 있는 것

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 통합 테스트: tests/ (있다면)
- 실행: `cargo test -p voltex_<crate>`
```

루트 `CLAUDE.md`에는 추가:
- **crate 의존 그래프** (텍스트 DAG)
- **도메인 용어집** (Entity, Component, System, Handle 등)
- **공통 패턴 인덱스** (에러, 컴포넌트, 시스템, 직렬화)
- **변경 워크플로** (수정 → 테스트 → CODEMAP 갱신)

### Layer 2: 파일 분리 (>600줄)

**원칙**: 단일 파일이 600줄을 넘으면 디렉토리로 분리.
`mod.rs`에서 `pub use`로 기존 API 재노출 → 외부 호출자 무수정.

**대상 (15개)**:

800줄+ (M2 우선 처리):
- `voltex_audio/src/vorbis.rs` (1493)
- `voltex_renderer/src/gltf.rs` (982)
- `voltex_renderer/src/jpg.rs` (959)
- `voltex_renderer/src/deferred_pipeline.rs` (905)

500-800줄 (M3):
- `voltex_editor/src/widgets.rs` (745)
- `voltex_ai/src/navmesh_builder.rs` (651)
- `voltex_ecs/src/world.rs` (589)
- `voltex_physics/src/solver.rs` (574)
- `voltex_editor/src/dock.rs` (558)
- `voltex_physics/src/gjk.rs` (524)
- `voltex_audio/src/wasapi.rs` (521)
- `voltex_editor/src/ttf_parser.rs` (518)
- `voltex_renderer/src/texture.rs` (514)
- `voltex_script/src/state.rs` (505)
- `voltex_ai/src/navmesh.rs` (497)

**예시 분리**:

```
src/vorbis.rs (1493줄)
   → src/vorbis/
       ├── mod.rs           (pub use; ~50줄)
       ├── decoder.rs       (~400줄)
       ├── codebook.rs      (~400줄)
       ├── floor.rs         (~300줄)
       └── residue.rs       (~300줄)
```

**제약**: 외부 공개 API 시그니처 무변경. `pub use`로 동일 경로 유지.

### Layer 3: 에러 타입 표준화

각 crate에 `error.rs` 추가, `thiserror` 사용:

```rust
#[derive(Debug, thiserror::Error)]
pub enum RendererError {
    #[error("shader compile failed: {0}")]
    Shader(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, RendererError>;
```

**규칙**:
- crate별로 `<Crate>Error` 단일 enum
- 공개 함수는 모두 `Result<T, <Crate>Error>` 반환
- crate 간 에러 전파는 `#[from]`으로 자동 변환
- `Result<T, String>` 같은 임시 패턴 전부 제거

**의존성 그래프 기준 마이그레이션 순서** (in → out):

```
voltex_math (no deps)
   ↓
voltex_platform
   ↓
voltex_asset
   ↓
voltex_audio, voltex_ai, voltex_net, voltex_script
   ↓
voltex_ecs
   ↓
voltex_renderer (depends on math, asset)
   ↓
voltex_physics (depends on math, ecs)
   ↓
voltex_editor (depends on most)
```

### Layer 4: ECS 패턴 표준화

**audit 후 확정**하되, 예상 패턴:

- **컴포넌트 등록**: `ComponentRegistry::register::<T>()` 단일 경로
- **시스템 정의**: `System` trait 또는 `fn(world: &mut World)` 둘 중 하나로 통일
- **컴포넌트 매크로** (선택적): `#[derive(Component)]` 도입 검토

이 부분은 implementation plan에서 실제 코드 audit 후 확정.

**audit 항목**:
- 현재 어떤 식으로 컴포넌트가 등록되는가?
- System trait이 있는가, 함수 포인터인가?
- 두 가지 이상 패턴이 공존하는가?
- 표준화 시 마이그레이션해야 할 위치는 어디인가?

### Layer 5: voltex facade crate

새 crate `crates/voltex/` 추가.

```rust
// crates/voltex/src/lib.rs
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
    pub use voltex_math::{Vec3, Vec4, Mat4, Quat, /* ... */};
    pub use voltex_ecs::{World, Entity, /* ... */};
    pub use voltex_platform::{VoltexWindow, GameTimer};
    // 자주 쓰는 것만 신중히 선별
}

pub struct App {
    // GameLoop + World + GpuContext 묶음 (최소 추상화)
}
```

사용자 코드:

```rust
use voltex::prelude::*;
// 또는 명시적으로
use voltex::renderer::PbrMaterial;
```

**원칙**:
- 과도한 추상화 금지 — 단순 re-export + 최소 facade
- `App` 구조는 `voltex_platform::GameLoop` + `voltex_ecs::World` + `voltex_renderer::GpuContext`를 묶는 정도
- `prelude`는 신중히 선별 (이름 충돌 회피)

## 작업 순서 (마일스톤)

각 마일스톤 끝에서 **테스트 760개 전부 통과 + examples 빌드 확인**을 검증 게이트로 한다.

| # | 마일스톤 | 작업 | 위험도 | 예상 |
|---|---|---|---|---|
| M1 | CODEMAP 레이어 | 11개 crate × CODEMAP.md, 루트 CLAUDE.md 보강 | 0 (코드 무변경) | 1일 |
| M2 | 파일 분리 (800+) | vorbis, gltf, jpg, deferred_pipeline 4개 분리 | 중 | 2일 |
| M3 | 파일 분리 (500-800) | 나머지 11개 분리 | 낮음 | 1-2일 |
| M4 | 에러 표준화 | crate별 `<Crate>Error` 도입 + 의존성 순 마이그레이션 | 중 (API 변경) | 2-3일 |
| M5 | ECS 패턴 audit + 표준화 | 현재 패턴 분석 → 통일안 결정 → 마이그레이션 | 중 | 1-2일 |
| M6 | voltex facade | 새 crate + prelude + App 추상화 | 낮음 | 1일 |
| M7 | 최종 검증 | examples 일부 새 facade로 마이그레이션, CODEMAP 갱신 | 낮음 | 1일 |

**총 9-12일**. 마일스톤마다 commit + push, 사용자 검토 가능.

### 순서 근거

- M1 먼저 — 이후 모든 작업의 AI 컨텍스트 효율을 즉시 향상
- M2/M3 — 큰 파일 정리 후 다음 작업 안전
- M4 — 에러 통일 후 facade에서 단일 Error 타입 노출 가능
- M5 — ECS는 게임 코드 전반에 영향, 표준화 늦추면 마이그레이션 비용 ↑
- M6 facade — 위 모든 게 정리된 후 깔끔히 묶기
- M7 검증 — examples로 새 구조 실제 사용성 확인

### 롤백 전략

- 각 마일스톤이 독립 commit (또는 series of commits)
- 문제 시 해당 마일스톤만 revert
- 마일스톤 진행 중 빌드 깨짐 허용, 종료 시점에는 무조건 통과

## 검증 및 테스트 전략

### 공통 검증 게이트 (모든 마일스톤 종료 시)

```bash
cargo build --workspace
cargo test --workspace --lib         # 760개 통과
cargo build --examples               # 13개 example
cargo clippy --workspace -- -D warnings
```

### 마일스톤별 추가 검증

| 마일스톤 | 추가 검증 |
|---|---|
| M1 | 11개 CODEMAP.md 존재, 루트 CLAUDE.md에 의존성 그래프 포함 |
| M2/M3 | `find crates -name "*.rs" \| xargs wc -l \| sort -rn` → 600 초과 0개 |
| M4 | `grep -r "Result<.*, String>" crates/` → 0건, 각 crate에 `<Crate>Error` 존재 |
| M5 | 모든 컴포넌트가 `ComponentRegistry::register` 경로 통과, examples 정상 동작 |
| M6 | `examples/triangle`이 `use voltex::prelude::*;` 1줄로 시작 가능 |
| M7 | examples 1개 이상이 새 facade로 마이그레이션, `cargo doc` 무경고 |

### Regression 방지 원칙

- 파일 분리 시 외부 공개 API 시그니처 변경 금지 (`pub use` 동일 경로 유지)
- 에러 마이그레이션은 crate 단위로 atomic commit (혼합 상태 금지)
- 각 마일스톤은 머지 가능한 commit 1-3개로 끝남

## 명시적 비포함 (Out of Scope)

본 spec은 다음을 다루지 않는다 — 별도 sub-project로 진행:

- 게임 템플릿/스캐폴딩 (새 게임 부트스트랩)
- 에디터 viewport gizmo, raycast 선택
- survivor_game ECS/Physics 재작성
- 사용자 문서/튜토리얼
- 성능 벤치마크
- `voltex_renderer` 자체의 sub-crate 분해 (예: voltex_image 추출) — 본 spec에서는 파일 분리까지만, crate 분해는 검토 후 차후

## Open Questions / 결정 필요 사항

implementation plan 작성 시 결정:

1. **Layer 4 ECS 표준화의 구체적 범위**: 매크로 도입 여부, 시스템 등록 방식 등 — 실제 코드 audit 후 결정
2. **prelude 포함 항목**: 이름 충돌 우려 (예: `Vec3` 등) — 선별 기준 명확화 필요
3. **`App` 추상화의 구체적 API**: 어디까지 묶을지 — 최소 추상화 원칙 유지하되 실용성 확보

## 리스크

| 리스크 | 영향 | 완화 |
|---|---|---|
| 파일 분리 중 internal API 노출 누락 | 빌드 실패 | 분리 직후 `cargo build` 확인, 누락 시 추가 `pub(crate)` |
| 에러 마이그레이션 누락된 호출자 | 컴파일 에러 | 의존성 순서로 진행, 한 crate 끝나면 빌드 |
| ECS 패턴 변경이 게임 코드 깨뜨림 | 테스트/example 실패 | 패턴 결정 시 가장 호환 가능한 안 선택, examples로 검증 |
| facade re-export 이름 충돌 | API 사용성 저하 | prelude 신중히 선별, 충돌 시 명시적 import 권장 |

## 관련 문서

- `docs/STATUS.md` — 현재 작업 상태
- `docs/DEFERRED.md` — 미뤄진 항목
- `docs/superpowers/specs/2026-03-24-voltex-engine-design.md` — 전체 엔진 설계 (원본)
- 본 spec 이후 작성될 `docs/superpowers/plans/2026-05-20-engine-restructure-*.md` — 마일스톤별 plan
