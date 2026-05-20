# survivor_game ECS/Physics 재작성 구현 계획

**Goal:** `Vec<Player>`/`Vec<Enemy>`/`Vec<Projectile>` 기반 구조를 voltex_ecs World + voltex_physics Collider 기반으로 재작성한다. 엔진 자체의 실전 검증.

**Architecture:** game.rs는 World를 보유. 모든 게임 상태는 components. 게임 로직은 World를 받는 `fn` 시스템. 충돌은 voltex_physics::detect_collisions로 라우팅. wave는 ECS와 무관한 작은 state 구조 그대로 유지.

**Tech Stack:** voltex facade. wgpu/winit boilerplate는 무변경 (렌더 루프만 World 쿼리 기반으로 교체).

**Spec reference:** STATUS.md 다음 작업 후보 #6, M1-M7 재구조화의 실전 검증 단계.

---

## 현재 구조 (재작성 전)

```
src/
├── main.rs       (880줄) — winit + wgpu boilerplate + 렌더 루프
├── game.rs       (135줄) — GameState { Vec<Player>, Vec<Enemy>, Vec<Projectile>, WaveSystem }
├── player.rs     ( 51줄) — Player { position, hp, fire_cooldown, invincible_timer, ... }
├── enemy.rs      ( 26줄) — Enemy { position, speed, alive }
├── projectile.rs ( 26줄) — Projectile { position, velocity, lifetime }
├── wave.rs       ( 66줄) — WaveSystem (spawn 타이밍 + edge position)
├── arena.rs      (186줄) — 정적 아레나 모델/머티리얼 (변경 없음)
└── camera.rs     ( 39줄) — QuarterViewCamera (변경 없음)
```

**문제**:
- 엔진의 World/Component/Collider를 전혀 사용하지 않음 → 엔진 실전 검증 부재
- 거리 기반 수동 collision (`dist < 0.55`) → voltex_physics 미사용
- 컴포넌트 단위 직렬화 불가능 → save/load 미지원

---

## 재작성 후 구조

```
src/
├── main.rs        (~700줄) — winit + wgpu + World 쿼리 기반 렌더 루프
├── game.rs        (~150줄) — World + Wave + score 보유. update/init/restart api
├── components.rs  (NEW, ~100줄) — Player/Enemy/Projectile/Velocity/Health/FireCooldown/InvincibleTimer/Lifetime/MoveSpeed
├── systems.rs     (NEW, ~200줄) — input/chase/motion/lifetime/collision/damage 시스템
├── wave.rs        ( 66줄) — 변경 없음 (이미 ECS 무관)
├── arena.rs       (186줄) — 변경 없음
├── camera.rs      ( 39줄) — 변경 없음
└── (player.rs/enemy.rs/projectile.rs 삭제)
```

총 줄 수는 비슷하거나 살짝 늘어남. 핵심은 엔진 사용 + 컴포넌트화.

---

## 컴포넌트 설계

```rust
// src/components.rs

// 마커 (tag-style, empty struct)
pub struct PlayerTag;
pub struct EnemyTag;
pub struct ProjectileTag;

// 데이터
#[derive(Clone, Copy)] pub struct Velocity(pub Vec3);
#[derive(Clone, Copy)] pub struct MoveSpeed(pub f32);
#[derive(Clone, Copy)] pub struct Health { pub hp: i32, pub max_hp: i32 }
#[derive(Clone, Copy)] pub struct FireCooldown(pub f32);
#[derive(Clone, Copy)] pub struct InvincibleTimer(pub f32);
#[derive(Clone, Copy)] pub struct Lifetime(pub f32);
```

**Collider** (voltex_physics::Collider): 모든 player/enemy/projectile entity에 `Collider::Sphere { radius }` 부착.

**ECS 직렬화는 생략**: 게임 인 게임만, 저장 불필요. 컴포넌트는 Component trait 구현하지 않음 (그래도 World에 저장 가능).

---

## 시스템 설계

```rust
// src/systems.rs

pub fn player_input_system(world: &mut World, input: &InputState, dt: f32) {
    // PlayerTag + Transform + MoveSpeed → Transform 갱신 (WASD)
    // FireCooldown 감소
    // InvincibleTimer 감소
}

pub fn enemy_chase_system(world: &mut World, player_pos: Vec3, dt: f32) {
    // EnemyTag + Transform + MoveSpeed → player 방향 이동
}

pub fn projectile_motion_system(world: &mut World, dt: f32) {
    // ProjectileTag + Transform + Velocity → 위치 갱신
}

pub fn lifetime_system(world: &mut World, dt: f32) -> Vec<Entity> {
    // Lifetime 감소, 0 이하인 entity 수집해 despawn
}

pub fn collision_system(world: &mut World) -> CollisionEvents {
    // voltex_physics::detect_collisions 호출
    // (Projectile vs Enemy) → 양쪽 despawn, score++
    // (Enemy vs Player, InvincibleTimer == 0) → Health--, InvincibleTimer = 1.0
}

pub struct CollisionEvents {
    pub enemies_killed: u32,
    pub player_hit: bool,
}
```

**컬렉션 패턴**: query2/3로 Entity 모아 별도 Vec에 수집 → 두 번째 패스에서 World 변형 (borrow conflict 회피).

---

## game.rs 신규 구조

```rust
pub struct Game {
    pub world: World,
    pub player_entity: Entity,
    pub wave: WaveSystem,
    pub score: u32,
    pub game_over: bool,
}

impl Game {
    pub fn new() -> Self { /* spawn player, init wave */ }
    pub fn restart(&mut self) { /* clear world, respawn player */ }

    pub fn update(&mut self, input: &InputState, dt: f32) -> FrameEvents {
        if self.game_over { return FrameEvents::default(); }

        let player_pos = self.player_pos();
        player_input_system(&mut self.world, input, dt);
        projectile_motion_system(&mut self.world, dt);
        enemy_chase_system(&mut self.world, player_pos, dt);

        // wave spawn → spawn_enemy(world, pos) helper
        let alive = self.alive_enemy_count();
        for pos in self.wave.update(dt, alive) {
            spawn_enemy(&mut self.world, pos);
        }

        // lifetime
        for e in lifetime_system(&mut self.world, dt) {
            self.world.despawn(e);
        }

        // collision
        let events = collision_system(&mut self.world);
        self.score += events.enemies_killed * 100;

        // game over check (player health)
        if let Some(h) = self.world.get::<Health>(self.player_entity) {
            if h.hp <= 0 { self.game_over = true; }
        }

        FrameEvents { /* projected from events */ }
    }

    // Query helpers (main.rs 렌더용)
    pub fn player_pos(&self) -> Vec3 { /* ... */ }
    pub fn player_health(&self) -> (i32, i32) { /* ... */ }
    pub fn player_invincible(&self) -> f32 { /* ... */ }
    pub fn alive_enemy_count(&self) -> usize { /* ... */ }
    pub fn projectile_positions(&self) -> Vec<Vec3> { /* ... */ }
    pub fn enemy_positions(&self) -> Vec<Vec3> { /* ... */ }
    pub fn try_shoot(&mut self, aim: Vec3) -> bool { /* spawn projectile if cooldown ready */ }
}
```

main.rs 인터페이스 거의 동일 — 렌더 루프는 `state.game.player_pos()` 같은 query helper만 사용. 직접 Vec 인덱싱 제거.

---

## main.rs 렌더 변경 요약

### Before
```rust
let num_projectiles = state.game.projectiles.len();
let num_enemies = state.game.enemies.len();
for i in 0..num_entities {
    let model = if i < ARENA_ENTITIES { state.models[i] }
                else if i == ARENA_ENTITIES { player_model }
                else if i < base_enemy_idx {
                    let p = &state.game.projectiles[i - ARENA_ENTITIES - 1];
                    /* ... */
                } else {
                    let e = &state.game.enemies[i - base_enemy_idx];
                    /* ... */
                };
}
```

### After
```rust
let projectile_positions = state.game.projectile_positions();
let enemy_positions = state.game.enemy_positions();
let num_projectiles = projectile_positions.len();
let num_enemies = enemy_positions.len();
// 동일한 인덱싱 로직, Vec<Vec3>에서 읽기
```

기존 index 기반 model/material 생성 로직은 유지. 데이터 출처만 `Vec<Player>/Vec<Enemy>` → World query 결과 Vec.

---

## Cargo.toml 변경

```toml
[dependencies]
voltex = { workspace = true }              # facade
voltex_physics.workspace = true             # detect_collisions, Collider
wgpu.workspace = true
winit.workspace = true
bytemuck.workspace = true
pollster.workspace = true
env_logger.workspace = true
log.workspace = true
```

기존 `voltex_math/platform/renderer/editor/audio` 5개 → `voltex + voltex_physics` 2개.

import 변경:
```rust
use voltex::prelude::*;                                    // Vec3, Mat4, World, Entity, Transform, ...
use voltex::renderer::{ /* 렌더링 타입들 */ };
use voltex::editor::{UiContext, UiRenderer};
use voltex::audio::{AudioClip, AudioSystem};
use voltex_physics::{Collider, detect_collisions};         // physics는 facade에 voltex::physics로 있음
```

---

## 작업 순서

- [ ] **Step 1**: 본 plan 작성
- [ ] **Step 2**: `src/components.rs` 신규
- [ ] **Step 3**: `src/systems.rs` 신규 (collision은 voltex_physics 사용)
- [ ] **Step 4**: `src/game.rs` 재작성 — World 보유 + spawn 헬퍼 + query helper
- [ ] **Step 5**: `src/player.rs`, `src/enemy.rs`, `src/projectile.rs` 삭제 + `src/main.rs` 갱신 + `Cargo.toml` 변경
- [ ] **Step 6**: `cargo build -p survivor_game`, `cargo test --workspace --lib` 통과 + commit/push

각 step 후 점진적 빌드 검증. 한 commit으로 묶을지 step별로 묶을지는 빌드 가능성 따라 결정.

---

## 검증 게이트

```bash
cargo build -p survivor_game             # 빌드 통과
cargo test --workspace --lib             # 765+ 통과 (게임은 lib test 없음, regression 없는지 확인)
cargo run -p survivor_game               # 실제 실행 (수동)
```

**행동 동등성 체크**:
- WASD 이동, 마우스 좌클릭 발사, 적 추격, 적-총알 충돌 사라짐, 적-플레이어 충돌 HP 감소, wave 진행, 게임 오버 + R 재시작

---

## 리스크 및 완화

| 리스크 | 영향 | 완화 |
|---|---|---|
| voltex_physics::detect_collisions가 BVH 빌드를 매 프레임 함 — overhead | 성능 ↓ (게임에서 수십 entity면 무시 가능) | 게임 규모상 무시. 차후 incremental BVH로 최적화 |
| Sphere radius가 기존 `dist < 0.55` 등 매직 넘버와 정확히 매칭 안 됨 | 게임 밸런스 다름 | 기존 매직 넘버 그대로 라디우스로 변환: player 0.5 / enemy 0.4 / projectile 0.15 |
| query2/3가 Vec를 alloc → 매 프레임 alloc 발생 | minor perf | 게임 규모상 무시. 차후 cached query로 최적화 |
| InvincibleTimer 처리 후 borrow conflict | 컴파일 에러 | 첫 번째 패스에서 Entity + 새 값을 모아 Vec에 수집, 두 번째 패스에서 적용 |
| projectile vs enemy 양쪽 despawn 시 동시 despawn 중복 | 두 번 despawn 호출 | HashSet으로 dedup |

---

## Out of Scope

- 컴포넌트 직렬화/세이브 로드 — 게임 인 게임 한정
- DependencyScheduler 사용 — 명시 함수 호출이 충분 (시스템 5개 정도)
- Rigid body 시뮬레이션 — 본 게임은 kinematic. 충돌 검출만 사용
- 새 적 종류, 무기 종류 — 동등 행동 재현이 목표
- main.rs 600줄 분리 — 예제이므로 spec 한도 비대상

---

## 후속 작업

ECS 재작성 후 자연스럽게 노출될 다음 작업 후보:
- 게임 상태를 .vscn으로 save/load (Component trait 구현 시)
- 적 종류 추가 시 컴포넌트 조합으로 즉시 확장 가능
- Lua 스크립트에서 spawn (engine_api 확장)
- Editor에서 직접 wave 파라미터 튜닝
