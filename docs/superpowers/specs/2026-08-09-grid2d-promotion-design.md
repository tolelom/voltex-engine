# voltex_physics::grid2d — 2D 타일 충돌 승격 설계 (friction-2 #4)

날짜: 2026-08-09
입력: `docs/dogfood-friction-2.md` #4 (P2), seed: `games/voltex_leap/src/physics2d.rs`

## 판정

friction #4의 "엔진 영역인가" 질문에 대한 판정: **타일 그리드 vs AABB 축 분리
해소는 승격, 캐릭터 튜닝(중력 상수, 점프 속도)은 게임 영역으로 잔류.**
근거: 타일 기반 2D 게임이면 장르 무관(플랫포머/탑다운/소코반) 전부 필요한
로직이고, leap 구현이 이미 테스트 4개로 형태를 증명했다. 스코프는 leap이
실제 쓴 만큼만 — 스윕/슬로프/원웨이 플랫폼은 다음 사용자가 나타날 때.

## API (`crates/voltex_physics/src/grid2d.rs`)

```rust
/// XY 평면 AABB를 타일 그리드에 대해 축 분리(X→Y) 이동·해소.
/// `is_solid(gx, gy)`가 그리드 정의 — 맵 타입에 비의존 (경계 정책 포함
/// 호출자 소유). 전제: 한 프레임 이동량 < tile (스윕 없음, doc 명시).
pub struct MoveResult2D {
    pub pos: Vec3,       // z 불변
    pub vel: Vec3,       // 충돌 축 성분 0
    pub grounded: bool,  // 하향 접촉 또는 발밑 프로브(tile*0.02) 히트
    pub hit_wall: bool,  // X 축 충돌
}

pub fn move_and_collide_2d(
    is_solid: impl Fn(i32, i32) -> bool,
    tile: f32,
    pos: Vec3,
    vel: Vec3,
    half: [f32; 2],
    dt: f32,
) -> MoveResult2D
```

- 구현은 leap physics2d 이식 (EPS flush 처리, 발밑 프로브 포함). 프로브
  깊이는 `tile * 0.02` (leap의 고정 0.02를 tile 비례로 일반화).
- `voltex_physics`는 에러 타입 없는 crate — 순수 함수 모듈로 정합.
- facade: `voltex::physics::grid2d` 자동 노출 (모듈 re-export 구조).

## leap 마이그레이션

`physics2d.rs`는 `GRAVITY` 상수 + `move_and_collide` 래퍼(LevelMap →
클로저 어댑터, `MoveResult` 재export)로 축소. 호출부(components/enemy)
시그니처 불변 — 동작 동일성은 기존 leap 테스트 4개가 회귀 가드.

## 검증

- grid2d 단위 테스트: leap physics2d 4개 시나리오 이식(낙하 착지/벽/천장/
  정지 접지) + 경계 정책이 클로저 소유임을 보이는 케이스(음수 좌표 open/
  solid 두 정책).
- leap 전 스위트(단위+플레이스루) 그린 = 동작 동일성.
- module_wiring 가드 통과, CODEMAP(physics) 갱신, friction-2 #4 취소선.
- workspace build/clippy(-D warnings)/test 그린. v1.0.113.

## 비범위

스윕 CCD, 슬로프, 원웨이 플랫폼, 타일 크기 비균일, 3D 그리드. #5(타일맵
배치 렌더)/#6(z-rot)은 다음 2D 게임 등장 시 재평가 (n=1 YAGNI 보류 판정).
