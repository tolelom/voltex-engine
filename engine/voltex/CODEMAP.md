# voltex

## 책임 (한 줄)
Facade crate — 13개 `voltex_*` sub-crate를 모듈로 노출하고 자주 쓰는 타입을 `prelude`에 모은다.

## 공개 API 진입점
- 모듈 re-export: `math`, `ecs`, `renderer`, `renderer_ecs`, `physics`, `audio`, `asset`, `ai`, `net`, `script`, `editor`, `platform`, `runtime` (src/lib.rs)
- `prelude` 모듈 — 자주 쓰는 타입 묶음 (src/lib.rs)

## 모듈 구조
- 단일 파일 `src/lib.rs` (~60줄, re-export + prelude)

## prelude 포함 타입
- `voltex_math`: Vec2, Vec3, Vec4, Mat4, AABB, Ray
- `voltex_ecs`: Entity, World, Transform, Component, ComponentRegistry, Parent, Children
- `voltex_platform`: VoltexWindow, WindowConfig, InputState, GameTimer
- `voltex_asset`: Handle, Assets

## prelude 의도적 미포함
- 도메인 특화 타입 (`GpuContext`, `RigidBody`, `Collider`, `LuaState`, `EditorSession` 등) — 명시 경로 사용
- 각 crate의 `Result<T>` alias — 이름 충돌 회피 (`voltex::ecs::Result` 식 명시)
- `Quat` (voltex_math에 없음, renderer::animation::Quat 명시 import)

## 의존성
- voltex crate: 모든 sub-crate (L0~L3 전체)
- 외부 crate: 없음

## 변경 시 주의점
- prelude에 새 타입 추가 시: 다른 crate의 같은 이름과 충돌하지 않는지 확인
- sub-crate의 공개 API를 prelude에 들이려면 호환성 영향이 워크스페이스 전체로 확산
- App 추상화는 의도적으로 미도입 (yagni). 도입 시 spec/plan에 별도 명시

## 테스트
- 단위 테스트: src/lib.rs (prelude/모듈 경로 컴파일 검증 2개)
- 실행: `cargo test -p voltex`
