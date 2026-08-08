# Voltex Leap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 2D 사이드뷰 플랫포머 도그푸딩 게임 — 엔진 2D 스택(orthographic, Sprite, SpriteAnimation, Particle, Lifetime) 실전 검증 + 새 마찰 로그.

**Architecture:** `games/voltex_leap` crate. 게임 로직 100% Rust, lib.rs 하위 모듈(level/physics2d/components/game_state)이 헤드리스 테스트 대상, main.rs가 RuntimeApp 배선. Depths(`games/voltex_depths`)의 구조/패턴을 미러하되 이동 모델은 2D 중력 플랫포머.

**Tech Stack:** voltex facade + voltex_runtime, dev-dep voltex_capture(+voltex_image은 gen_assets용).

## Global Constraints

- 게임 로직 100% Rust. Lua/에디터 저작 비범위.
- 엔진 crate 즉석 수정 금지 — 마찰은 `docs/dogfood-friction-2.md`에 기록 (진행을 실제로 막을 때만 예외).
- workspace build/clippy(`-D warnings`)/test 그린 유지. 파일 600줄 한도.
- 게임 평면 = XY (X 수평, Y 수직↑), 전 엔티티 z=0. TILE = 1.0 world unit.
- ASCII 맵: 행 0 = 최상단. `world_y = (rows-1-row) as f32 * TILE`. 셀 중심 = `(gx as f32 + 0.5, gy_world as f32 + 0.5)` 아님 — **셀 좌하단 기준 + 0.5 오프셋** 즉 중심 = `(gx + 0.5, world_row + 0.5)`.
- 각 태스크 완료 시 atomic commit. 마찰 발견 즉시 friction-2 로그에 추가 (커밋은 태스크 커밋에 동승 가능).

---

### Task 1: Crate 스캐폴드

**Files:**
- Create: `games/voltex_leap/Cargo.toml`, `games/voltex_leap/src/lib.rs`, `games/voltex_leap/src/main.rs`
- Modify: 루트 `Cargo.toml` workspace members에 `games/voltex_leap` 추가 (depths 항목 옆)

**Interfaces:**
- Produces: 빈 lib(모듈 선언은 후속 태스크가 추가), main은 부팅 스텁.

- [ ] **Step 1: depths 항목 확인 후 Cargo.toml 작성**

```toml
[package]
name = "voltex_leap"
version = "0.1.0"
edition = "2021"
license.workspace = true
default-run = "voltex_leap"

[dependencies]
voltex.workspace = true
voltex_runtime.workspace = true
env_logger = { workspace = true }

[dev-dependencies]
voltex_capture.workspace = true

[build-dependencies]
```

(`[build-dependencies]` 빈 섹션은 쓰지 말 것 — 생략. gen_assets가 `voltex_image`/`voltex_audio`를 쓰면 일반 deps에 추가하되 facade `voltex::image`/`voltex::audio` 경유가 우선.)

`src/lib.rs`:
```rust
//! Voltex Leap — 2D 사이드뷰 플랫포머. 도그푸딩 게임 2편.
//! 설계: docs/superpowers/specs/2026-08-09-voltex-leap-game-design.md
```

`src/main.rs`:
```rust
fn main() {
    println!("voltex_leap: wiring lands in Task 8");
}
```

- [ ] **Step 2: 빌드 확인** — `cargo build -p voltex_leap` PASS
- [ ] **Step 3: Commit** — `feat(leap): scaffold voltex_leap game crate`

---

### Task 2: level.rs — ASCII 맵 파싱 + 좌표계

**Files:**
- Create: `games/voltex_leap/src/level.rs` (+ lib.rs에 `pub mod level;`)
- 참고: `games/voltex_depths/src/level.rs` (파싱 스타일)

**Interfaces:**
- Produces:
  - `pub const TILE: f32 = 1.0;`
  - `pub struct LevelMap { pub cols: usize, pub rows: usize, solids: Vec<bool>, pub spikes: Vec<(usize, usize)>, pub coins: Vec<(usize, usize)>, pub walkers: Vec<(usize, usize)>, pub player_spawn: (usize, usize), pub door: (usize, usize) }` — 좌표는 전부 **그리드 (gx, gy)** 이며 gy는 **월드 방향(0 = 최하단)** 으로 저장.
  - `pub fn parse(rows: &[&str]) -> Result<LevelMap, String>` — 문자: `#`솔리드, `^`가시, `o`코인, `E`워커, `P`스폰(정확히 1), `D`문(정확히 1), `.`/공백 빈칸. 행 길이 불일치/P·D 개수 오류는 Err.
  - `pub fn is_solid(&self, gx: i32, gy: i32) -> bool` — 맵 밖 x는 solid(벽 취급), 맵 밖 y는 빈칸(위로 점프/아래로 낙사 허용).
  - `pub fn cell_center(gx: usize, gy: usize) -> voltex::prelude::Vec3` — `Vec3::new(gx as f32 * TILE + 0.5, gy as f32 * TILE + 0.5, 0.0)`.
  - `pub fn grid_at(pos_x: f32, pos_y: f32) -> (i32, i32)` — floor 나눗셈.

- [ ] **Step 1: 실패 테스트 작성** (level.rs 하단 `#[cfg(test)]`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &[&str] = &[
        "................",
        "..P...o....E..D.",
        "####..####..####",
    ];

    #[test]
    fn parses_entities_into_world_oriented_grid() {
        let m = LevelMap::parse(MAP).unwrap();
        assert_eq!((m.cols, m.rows), (16, 3));
        // ASCII 행 1(중단) → 월드 gy 1. P는 (2,1).
        assert_eq!(m.player_spawn, (2, 1));
        assert_eq!(m.door, (14, 1));
        assert_eq!(m.coins, vec![(6, 1)]);
        assert_eq!(m.walkers, vec![(11, 1)]);
        // ASCII 최하단 행 → 월드 gy 0.
        assert!(m.is_solid(0, 0));
        assert!(!m.is_solid(4, 0)); // 갭
    }

    #[test]
    fn out_of_bounds_x_is_solid_but_y_is_open() {
        let m = LevelMap::parse(MAP).unwrap();
        assert!(m.is_solid(-1, 0));
        assert!(m.is_solid(m.cols as i32, 0));
        assert!(!m.is_solid(2, -1));
        assert!(!m.is_solid(2, m.rows as i32));
    }

    #[test]
    fn rejects_bad_maps() {
        assert!(LevelMap::parse(&["##", "#"]).is_err()); // 행 길이
        assert!(LevelMap::parse(&["..", ".."]).is_err()); // P 없음
        assert!(LevelMap::parse(&["PP", "D."]).is_err()); // P 2개
    }

    #[test]
    fn coordinate_round_trip() {
        let c = cell_center(3, 2);
        assert_eq!(grid_at(c.x, c.y), (3, 2));
        assert_eq!(grid_at(-0.1, 0.0), (-1, 0)); // floor, truncation 아님
    }
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p voltex_leap --lib` FAIL (미구현)
- [ ] **Step 3: 구현** — parse는 rows를 역순 순회하며 world gy로 저장. solids는 `Vec<bool>` row-major(gy*cols+gx). `grid_at`은 `pos.div_euclid(TILE)` 계열(`(pos_x / TILE).floor() as i32`).
- [ ] **Step 4: 테스트 통과 확인**
- [ ] **Step 5: Commit** — `feat(leap): ASCII level parsing + world-oriented grid coords`

---

### Task 3: physics2d.rs — 중력 + AABB 타일 충돌

**Files:**
- Create: `games/voltex_leap/src/physics2d.rs` (+ lib.rs 선언)

**Interfaces:**
- Consumes: `LevelMap::is_solid`, `TILE`, `grid_at`.
- Produces:
  - `pub const GRAVITY: f32 = -30.0;`
  - `pub struct MoveResult { pub pos: voltex::prelude::Vec3, pub vel: voltex::prelude::Vec3, pub grounded: bool, pub hit_wall: bool }`
  - `pub fn move_and_collide(map: &LevelMap, pos: Vec3, vel: Vec3, half: [f32; 2], dt: f32) -> MoveResult` — 축 분리: X 이동→해소, Y 이동→해소. 관통 시 접촉면에 스냅 + 해당 축 vel 0. `grounded` = 아래 접촉으로 y-vel이 0이 된 프레임 또는 발밑 ε(0.02) 내 솔리드.
- 알고리즘 (각 축): 이동 후 AABB `[pos-half, pos+half]`가 겹치는 셀 범위를 `grid_at`로 순회, 솔리드 셀과 겹치면 이동 방향 반대로 셀 경계에 스냅. dt가 커도 한 프레임 이동량이 TILE 미만이라는 전제(플레이어 속도 × 1/60 ≪ 1)로 스윕 생략 — 전제는 doc comment로 명시.

- [ ] **Step 1: 실패 테스트 작성**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::LevelMap;
    use voltex::prelude::Vec3;

    const MAP: &[&str] = &[
        "#....#",
        "#.P..#",
        "######",
    ];
    fn map() -> LevelMap { LevelMap::parse_no_door(MAP).unwrap() }
    // parse_no_door: 테스트 전용 — D 없이 파싱 허용하는 cfg(test) 헬퍼를
    // level.rs에 추가 (parse 내부를 공유하고 P/D 검증만 스킵).

    const HALF: [f32; 2] = [0.35, 0.45];

    #[test]
    fn falls_and_lands_on_floor() {
        let m = map();
        let mut pos = Vec3::new(2.5, 2.0, 0.0);
        let mut vel = Vec3::ZERO;
        let mut grounded = false;
        for _ in 0..120 {
            vel.y += GRAVITY * (1.0 / 60.0);
            let r = move_and_collide(&m, pos, vel, HALF, 1.0 / 60.0);
            pos = r.pos; vel = r.vel; grounded = r.grounded;
        }
        // 바닥 윗면 y=1.0 위에 반높이만큼 떠서 정지.
        assert!((pos.y - (1.0 + HALF[1])).abs() < 1e-3, "pos.y={}", pos.y);
        assert_eq!(vel.y, 0.0);
        assert!(grounded);
    }

    #[test]
    fn wall_blocks_horizontal_motion() {
        let m = map();
        let r = move_and_collide(
            &m, Vec3::new(4.4, 1.5, 0.0), Vec3::new(10.0, 0.0, 0.0), HALF, 0.1);
        assert!((r.pos.x - (5.0 - HALF[0])).abs() < 1e-3);
        assert_eq!(r.vel.x, 0.0);
        assert!(r.hit_wall);
    }

    #[test]
    fn ceiling_stops_upward_velocity() {
        let m = map();
        let r = move_and_collide(
            &m, Vec3::new(2.5, 2.4, 0.0), Vec3::new(0.0, 10.0, 0.0), HALF, 0.1);
        assert!((r.pos.y - (3.0 - HALF[1])).abs() < 1e-3);
        assert_eq!(r.vel.y, 0.0);
        assert!(!r.grounded);
    }

    #[test]
    fn grounded_without_vertical_motion() {
        let m = map();
        let standing = Vec3::new(2.5, 1.0 + HALF[1], 0.0);
        let r = move_and_collide(&m, standing, Vec3::new(1.0, 0.0, 0.0), HALF, 0.016);
        assert!(r.grounded);
    }
}
```

- [ ] **Step 2: 실패 확인**
- [ ] **Step 3: 구현** (+ level.rs에 `parse_no_door` cfg(test) 헬퍼)
- [ ] **Step 4: 통과 확인**
- [ ] **Step 5: Commit** — `feat(leap): 2D gravity + axis-separated tile collision`

---

### Task 4: components.rs + 플레이어 컨트롤

**Files:**
- Create: `games/voltex_leap/src/components.rs` (+ lib.rs 선언)

**Interfaces:**
- Produces:
  - `pub struct Player { pub vel: Vec3, pub grounded: bool, pub facing: f32 /* +1/-1 */ }` (Default: vel ZERO, grounded false, facing 1.0)
  - `pub struct Walker { pub dir: f32, pub speed: f32 }` / `pub struct Coin;` / `pub struct DoorMarker;` / `pub struct Spike;` — 전부 마커/소상태. Component trait 구현 불필요(씬 직렬화 비범위).
  - `pub const PLAYER_HALF: [f32; 2] = [0.35, 0.45];`
  - `pub const PLAYER_SPEED: f32 = 5.0;` `pub const JUMP_VELOCITY: f32 = 12.0;`
  - `pub fn player_control(map: &LevelMap, world: &mut World, move_x: f32, jump_pressed: bool, dt: f32) -> PlayerEvents` — Tag "player" 엔티티에 적용. `PlayerEvents { pub jumped: bool, pub landed: bool }` (파티클/SFX 트리거용 — grounded false→true 전이 = landed).

- [ ] **Step 1: 실패 테스트 작성** (components.rs 내)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{cell_center, LevelMap};
    use voltex::ecs::Tag;
    use voltex::prelude::*;

    const MAP: &[&str] = &[
        "......",
        "..P...",
        "######",
    ];

    fn setup() -> (LevelMap, World, Entity) {
        let m = LevelMap::parse_no_door(MAP).unwrap();
        let mut w = World::new();
        let p = w.spawn_bundle((
            Transform::from_position(cell_center(2, 1)),
            Player::default(),
            Tag("player".into()),
        ));
        (m, w, p)
    }

    #[test]
    fn walks_right_and_faces_direction() {
        let (m, mut w, p) = setup();
        for _ in 0..30 { player_control(&m, &mut w, 1.0, false, 1.0 / 60.0); }
        let t = w.get::<Transform>(p).unwrap();
        assert!(t.position.x > 2.5 + 1.0);
        assert_eq!(w.get::<Player>(p).unwrap().facing, 1.0);
        player_control(&m, &mut w, -1.0, false, 1.0 / 60.0);
        assert_eq!(w.get::<Player>(p).unwrap().facing, -1.0);
    }

    #[test]
    fn jump_only_when_grounded_and_reports_events() {
        let (m, mut w, p) = setup();
        // 착지까지 안정화
        for _ in 0..60 { player_control(&m, &mut w, 0.0, false, 1.0 / 60.0); }
        assert!(w.get::<Player>(p).unwrap().grounded);
        let ev = player_control(&m, &mut w, 0.0, true, 1.0 / 60.0);
        assert!(ev.jumped);
        // 공중에서 재점프 불가
        let ev2 = player_control(&m, &mut w, 0.0, true, 1.0 / 60.0);
        assert!(!ev2.jumped);
        // 낙하 후 착지 이벤트
        let mut landed = false;
        for _ in 0..120 {
            landed |= player_control(&m, &mut w, 0.0, false, 1.0 / 60.0).landed;
        }
        assert!(landed);
    }
}
```

- [ ] **Step 2: 실패 확인** → **Step 3: 구현** (`player_control`: vel.x = move_x*PLAYER_SPEED, vel.y += GRAVITY*dt, jump시 vel.y = JUMP_VELOCITY, move_and_collide 적용, facing은 move_x != 0일 때 부호) → **Step 4: 통과** 
- [ ] **Step 5: Commit** — `feat(leap): player components + platformer control`

---

### Task 5: 워커 AI + 접촉 사망 판정

**Files:**
- Create: `games/voltex_leap/src/enemy.rs` (+ lib.rs 선언)

**Interfaces:**
- Consumes: `Walker`, `Spike`, `PLAYER_HALF`, level API.
- Produces:
  - `pub const WALKER_HALF: [f32; 2] = [0.35, 0.35];`
  - `pub fn walker_system(map: &LevelMap, world: &mut World, dt: f32)` — dir*speed 수평 이동(move_and_collide). **벽에 막히거나(전방 hit_wall) 다음 발판이 비면(전방 아래 셀 !solid) dir 반전.**
  - `pub fn lethal_contact(world: &World) -> bool` — 플레이어 AABB(PLAYER_HALF)가 Walker AABB 또는 Spike 셀(가시 그리드의 셀 AABB, 높이 절반 0.3)과 겹치면 true. Spike 위치는 스폰된 엔티티(Transform+Spike) 기준.

- [ ] **Step 1: 실패 테스트 작성**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::*;
    use crate::level::{cell_center, LevelMap};
    use voltex::ecs::Tag;
    use voltex::prelude::*;

    const MAP: &[&str] = &[
        "......",
        ".E....",
        "###...",  // x=3부터 낭떠러지
    ];

    #[test]
    fn walker_turns_at_ledge_and_wall() {
        let m = LevelMap::parse_no_door(MAP).unwrap();
        let mut w = World::new();
        let e = w.spawn_bundle((
            Transform::from_position(cell_center(1, 1)),
            Walker { dir: 1.0, speed: 2.0 },
        ));
        // 오른쪽 낭떠러지(발판 끝)에서 반전
        for _ in 0..240 { walker_system(&m, &mut w, 1.0 / 60.0); }
        // 왕복하되 발판(0..=2) 위에 머문다
        let x = w.get::<Transform>(e).unwrap().position.x;
        assert!(x > 0.0 && x < 3.0, "x={x}");
        // 순회 동안 dir이 한 번은 음수였어야 함 (반전 증거)
        // → 별도 확인: 지금 위치에서 60틱 더 돌리면 벽(x=0)에서도 반전해 진행
        let before = x;
        for _ in 0..60 { walker_system(&m, &mut w, 1.0 / 60.0); }
        assert_ne!(before, w.get::<Transform>(e).unwrap().position.x);
    }

    #[test]
    fn player_dies_on_walker_or_spike_overlap() {
        let mut w = World::new();
        w.spawn_bundle((
            Transform::from_position(Vec3::new(2.0, 1.5, 0.0)),
            Player::default(),
            Tag("player".into()),
        ));
        assert!(!lethal_contact(&w));
        w.spawn_bundle((
            Transform::from_position(Vec3::new(2.3, 1.5, 0.0)),
            Walker { dir: 1.0, speed: 0.0 },
        ));
        assert!(lethal_contact(&w));
    }
}
```

- [ ] **Step 2: 실패 확인** → **Step 3: 구현** → **Step 4: 통과**
- [ ] **Step 5: Commit** — `feat(leap): patrol walker AI + lethal contact`

---

### Task 6: game_state.rs — phase 전이 + 코인/문 + 스폰/재시작

**Files:**
- Create: `games/voltex_leap/src/game_state.rs` (+ lib.rs 선언)
- 참고: `games/voltex_depths/src/game_state.rs` (start_game/world.clear 패턴, HUD)

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, PartialEq, Debug)] pub enum GamePhase { Title, Playing, Dead, Clear }`
  - `pub struct CoinCount(pub u32);` — 리소스 대신 플레이어 엔티티에 부착.
  - `pub fn spawn_level(world: &mut World, map: &LevelMap)` — world.clear() 후 타일(솔리드 셀당 Sprite::solid 엔티티), 가시/코인/워커/문/플레이어 스폰. 스프라이트 배선(텍스처 핸들)은 main이 스폰 후 입히므로 여기선 solid 색만: 타일 회색 `[0.45,0.45,0.5,1]`, 가시 빨강 `[0.9,0.2,0.2,1]`, 코인 노랑 `[1.0,0.85,0.2,1]`, 문 초록 `[0.2,0.9,0.4,1]`, 워커 보라 `[0.7,0.3,0.9,1]`, 플레이어 흰색(텍스처가 입혀짐). 태그: player만 Tag("player").
  - `pub struct StepOutcome { pub coins_gained: u32, pub died: bool, pub cleared: bool }`
  - `pub fn playing_step(map: &LevelMap, world: &mut World, dt: f32) -> StepOutcome` — walker_system → lethal_contact → 코인 수집(플레이어 중심-코인 중심 거리 < 0.6 → 코인 despawn + CoinCount 증가) → 문 도달(거리 < 0.6 → cleared). **플레이어 이동은 여기 없음** (action system이 input으로 수행 — depths와 동일 분리).
  - `pub fn draw_hud(ui: &mut voltex::editor::UiContext, world: &World, phase: GamePhase)` — 코인 수 + phase별 안내 텍스트 (Title: "SPACE to start", Dead: "R to retry", Clear: "CLEAR! R to restart"). depths draw_hud 스타일.

- [ ] **Step 1: 실패 테스트 작성**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::*;
    use crate::level::LevelMap;
    use voltex::prelude::*;

    const MAP: &[&str] = &[
        "......",
        ".P.o.D",
        "######",
    ];

    #[test]
    fn spawn_populates_and_respawn_resets() {
        let m = LevelMap::parse(MAP).unwrap();
        let mut w = World::new();
        spawn_level(&mut w, &m);
        assert!(w.find_by_tag("player").is_some());
        assert_eq!(w.query::<Coin>().count(), 1);
        // 코인 하나 지우고 재스폰하면 복구
        let c = w.query::<Coin>().map(|(e, _)| e).next().unwrap();
        w.despawn(c);
        spawn_level(&mut w, &m);
        assert_eq!(w.query::<Coin>().count(), 1);
    }

    #[test]
    fn coin_pickup_and_door_clear() {
        let m = LevelMap::parse(MAP).unwrap();
        let mut w = World::new();
        spawn_level(&mut w, &m);
        let p = w.find_by_tag("player").unwrap();
        // 코인 위로 순간이동 → 수집
        w.get_mut::<Transform>(p).unwrap().position = crate::level::cell_center(3, 1);
        let out = playing_step(&m, &mut w, 1.0 / 60.0);
        assert_eq!(out.coins_gained, 1);
        assert_eq!(w.query::<Coin>().count(), 0);
        assert_eq!(w.get::<CoinCount>(p).unwrap().0, 1);
        // 문 위로 → 클리어
        w.get_mut::<Transform>(p).unwrap().position = crate::level::cell_center(5, 1);
        assert!(playing_step(&m, &mut w, 1.0 / 60.0).cleared);
    }

    #[test]
    fn walker_contact_reports_death() {
        let m = LevelMap::parse(MAP).unwrap();
        let mut w = World::new();
        spawn_level(&mut w, &m);
        let p = w.find_by_tag("player").unwrap();
        let pos = w.get::<Transform>(p).unwrap().position;
        w.spawn_bundle((Transform::from_position(pos), Walker { dir: 1.0, speed: 0.0 }));
        assert!(playing_step(&m, &mut w, 1.0 / 60.0).died);
    }
}
```

- [ ] **Step 2: 실패 확인** → **Step 3: 구현** → **Step 4: 통과** (draw_hud는 단위 테스트 비대상 — depths와 동일 수용)
- [ ] **Step 5: Commit** — `feat(leap): level spawn, coin/door/death step, phase types`

---

### Task 7: gen_assets — 절차 생성 SFX + 스프라이트시트 PNG

**Files:**
- Create: `games/voltex_leap/src/bin/gen_assets.rs`, 생성물 `games/voltex_leap/assets/*.{wav,png}` 커밋
- 참고: `games/voltex_depths/src/bin/gen_sfx.rs` (신스 패턴), `voltex::image::encode_rgba8`

**Interfaces:**
- Produces (assets/): `sfx_jump.wav, sfx_coin.wav, sfx_death.wav, sfx_clear.wav, bgm.wav`, `player_idle.png`(2프레임 가로), `player_run.png`(4프레임 가로), `player_jump.png`(1프레임), `coin.png`(4프레임 가로).
- **애니메이션 상태별 텍스처 분리 이유(마찰 로그 대상)**: `SpriteAnimation`은 시트의 `0..frame_count` 프리픽스만 재생 — 시작 프레임 오프셋/행 선택이 없어 한 시트에 idle/run/jump를 담으면 상태 전환이 불가. 상태마다 텍스처를 갈아끼우는 워크어라운드 채택.
- 스프라이트 픽셀아트는 코드로 그린 단순 도형(캡슐 몸통 + 다리 위상 변화, 코인은 회전 타원). 프레임 32×32.

- [ ] **Step 1: gen_assets.rs 작성** — wav는 depths gen_sfx의 톤 신스 함수 이식(사각/사인 스윕), png는 `Vec<u8>` RGBA 채워 `encode_rgba8(w, h, &pixels)` 후 fs::write.
- [ ] **Step 2: 실행** — `cargo run -p voltex_leap --bin gen_assets` → assets/ 9개 파일 생성 확인
- [ ] **Step 3: 디코드 회귀 테스트** — lib 테스트에 assets/ PNG를 `voltex::image::parse_png`로 읽어 크기 단언 (idle 64×32, run 128×32, jump 32×32, coin 128×32):

```rust
#[test]
fn generated_spritesheets_decode_with_expected_dims() {
    for (file, w) in [("player_idle.png", 64), ("player_run.png", 128),
                      ("player_jump.png", 32), ("coin.png", 128)] {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/");
        let bytes = std::fs::read(format!("{path}{file}")).unwrap();
        let img = voltex::image::parse_png(&bytes).unwrap();
        assert_eq!((img.width, img.height), (w, 32), "{file}");
    }
}
```
(parse_png 반환 타입 필드명은 구현 시 실제 API 확인 후 맞출 것)
- [ ] **Step 4: Commit** — `feat(leap): procedural sfx/bgm + spritesheet generation`

---

### Task 8: main.rs 풀 배선

**Files:**
- Modify: `games/voltex_leap/src/main.rs`
- 참고: `games/voltex_depths/src/main.rs` (전체 구조), `crates/voltex_runtime/src/config.rs` (`with_orthographic`)

**Interfaces:**
- Consumes: 전 모듈. ClipId 상수 = with_audio_files 순서 (`JUMP=0, COIN=1, DEATH=2, CLEAR=3, BGM=4`).

배선 내용 (depths 구조 미러, 차이만 명시):
1. **Config**: `with_window_title("Voltex Leap").with_window_size(1280, 720).with_orthographic(12.0).with_camera(Vec3::new(0.0, 0.0, 10.0), 0.0, 0.0).with_hud(true).with_asset_root(...).with_audio_files(...).disable_physics().disable_scripts()`.
2. **텍스처**: 부트 직후 `app` 렌더 에셋에 `add_texture_png`로 4장 로드해 핸들 확보. (RuntimeApp에서 RenderAssets 접근 경로는 구현 시 확인 — 접근 불가면 **마찰 기록** 후 AssetPath(TexturePath) 경로 or set-up 콜백 우회.)
3. **입력**: `bind_axis("move_x", KeyD, KeyA)` + Space 점프/시작, R 재시작.
4. **카메라 follow**: `set_camera_fn` — `cam.position = Vec3::new(player.x.clamp(cam_min_x, cam_max_x), level_center_y, 10.0)`, yaw 0, pitch 0. clamp 범위 = 레벨 폭 − 직교 뷰 반폭.
5. **action system**: phase 머신 (Title—Space→ spawn_level+BGM+Playing / Playing→ player_control 호출 + 점프/착지 이벤트에 SFX·먼지 파티클 / Dead·Clear—R→ Title). depths처럼 `Rc<RefCell<GamePhase>>` 공유.
6. **user system**: Playing 중 `playing_step` + `update_sprite_animations(world, dt)` 호출 — coins_gained>0 → SFX+스파클 파티클, died→Dead, cleared→Clear phase 전환. **플레이어 애니메이션 상태 스위치**: grounded&&|move_x|>0 → run 텍스처+4프레임, grounded 정지 → idle 2프레임, 공중 → jump 1프레임. 상태 바뀔 때만 Sprite.texture/SpriteAnimation 교체. facing에 따라 `uv_rect` 좌우 반전(`[u1,v0,u0,v1]`) — **렌더러가 역순 uv를 지원 안 하면 마찰 기록** 후 반전 생략.
7. **HUD**: `set_hud_fn` → `draw_hud`.
8. **파티클**: `spawn_particle(world, pos, color, Particle { ... })` + Lifetime은 Particle 내장 수명 사용 (Particle API 확인 후 필드 채움).
9. main 하단 `#[cfg(test)]`: MAP 파싱 구조 단언 (depths 미러):

```rust
#[test]
fn main_map_parses() {
    let m = LevelMap::parse(MAP).unwrap();
    assert!(m.coins.len() >= 3);
    assert!(m.walkers.len() >= 2);
}
```

- [ ] **Step 1: MAP 설계** — 폭 ~48, 높이 ~10. 시작 발판 → 갭 점프 2회 → 가시 구간 → 워커 순찰 발판 2곳 → 상단 코인 3+ → 문. 
- [ ] **Step 2: 배선 구현** (위 1~9)
- [ ] **Step 3: 빌드+테스트** — `cargo build -p voltex_leap && cargo test -p voltex_leap`
- [ ] **Step 4: 스모크 런** — `cargo run -p voltex_leap` 6초 (panic/validation error 없음 확인, 화면 검증은 Task 10)
- [ ] **Step 5: Commit** — `feat(leap): full runtime wiring (input/camera/anim/audio/HUD)`

---

### Task 9: 헤드리스 풀 플레이스루 테스트

**Files:**
- Create: `games/voltex_leap/tests/playthrough.rs`
- 참고: `games/voltex_depths/tests/playthrough.rs` (tick 순서 미러 원칙)

**Interfaces:** lib 공개 API만 사용 (RuntimeApp 불필요).

- [ ] **Step 1: 테스트 작성** — main.rs의 실제 tick 순서를 미러한 헬퍼 `fn tick(map, world, input: SimInput, dt) -> StepOutcome` (player_control → playing_step). 시나리오:
  1. `full_clear_run`: 스폰 → 스크립트된 입력 시퀀스(우측 이동 + 갭 앞 점프)로 문까지 도달 → cleared. 입력 시퀀스는 프레임 구간별 `(move_x, jump)` 테이블. **레벨을 실제 클리어 가능함을 증명하는 테스트** — 통과할 때까지 MAP 또는 시퀀스 조정.
  2. `death_by_spike_and_restart`: 가시로 걸어가 died → spawn_level 재호출 → 코인/워커 복구 + 플레이어 스폰 위치 복귀.
  3. `coin_collection_counts`: 경로 상 코인 최소 1개 수집돼 CoinCount 증가.
- [ ] **Step 2: 통과 확인** — `cargo test -p voltex_leap --test playthrough`
- [ ] **Step 3: Commit** — `test(leap): headless full playthrough (clear + death/restart)`

---

### Task 10: GPU 시각 회귀

**Files:**
- Create: `games/voltex_leap/tests/visual.rs`
- 참고: `games/voltex_depths/tests/visual.rs` (비율 기반 색 단언, HeadlessGpu skip 패턴)

- [ ] **Step 1: 테스트 작성** — `HeadlessGpu` 확보(불가 시 skip), `spawn_level`로 월드 구성, `voltex_capture::render_world(gpu, 640, 360, &mut world, cam_pos, 0.0, 0.0)` → 픽셀에서 타일 회색/가시 빨강/문 초록 존재 비율 단언 + 스크린샷 `docs/visual/leap_level.png` 저장. **render_world는 perspective 카메라 — orthographic 미지원이면 마찰 기록** 후 perspective 샷으로 단언.
- [ ] **Step 2: 통과 확인** (GPU 머신) → **Step 3: Commit** — `test(leap): GPU level visual regression + screenshot`

---

### Task 11: 마찰 로그 + 문서 마무리

**Files:**
- Create: `docs/dogfood-friction-2.md` (구현 중 계속 누적 — 이 태스크에서 머리말/등급 정리)
- Modify: `docs/STATUS.md` (신규 섹션), `CHANGELOG.md` (v1.0.111), 루트 `Cargo.toml` version, `README.md` 배지(있으면)

- [ ] **Step 1: friction-2 머리말** — 1편 로그 형식 미러: 항목 테이블(하려던 것/마찰/우회/제안/P등급) + 요약.
- [ ] **Step 2: STATUS/CHANGELOG** — 게임 개요, 검증 결과, 마찰 건수 요약.
- [ ] **Step 3: 최종 검증** — `cargo build --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
- [ ] **Step 4: Commit** — `docs(leap): STATUS/CHANGELOG + dogfood friction log 2 (v1.0.111)`

---

## Self-Review 결과

- 스펙 커버리지: 뷰/레벨 저작/플레이어/적/하자드/코인/문/파티클/오디오/텍스처/HUD/검증 4종 → T2~T10 매핑 확인. 에디터 저작 비범위 유지.
- 타입 일관성: `PLAYER_HALF`/`MoveResult`/`StepOutcome`/`GamePhase` 시그니처 태스크 간 일치 확인.
- 미확정 API(parse_png 반환 필드, RenderAssets 접근, Particle 필드)는 구현 시 확인을 명시 — 마찰 후보로 지정.
