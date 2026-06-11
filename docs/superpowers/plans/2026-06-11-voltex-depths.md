# Voltex Depths Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 탑다운 트윈스틱 던전 클리어 게임 `games/voltex_depths` — 타이틀→플레이→클리어/게임오버 루프 + SFX/BGM + 빌드 배포까지. 엔진 도그푸딩이 목적이며 마찰은 `docs/dogfood-friction.md`에 기록한다.

**Architecture:** ASCII 그리드 → 엔티티/NavMesh를 만드는 순수 `level` 모듈 위에, `combat`(투사체/피격/사망), `enemy`(추적자/사수/보스 AI), `game_state`(문/HUD/phase) 시스템을 얹고 `main.rs`가 RuntimeApp에 배선한다. 게임 상태 공유는 World 리소스(클리어 시 소멸 OK인 것) + `Rc<RefCell<>>` 클로저 캡처(phase/map/카메라 VP — World.clear() 생존 필요)로 나눈다. 신규 엔진 기능 0이 원칙; 막히면 friction 기록 후 최소 우회.

**Tech Stack:** voltex facade (prelude, ai, physics, runtime), voltex_capture (시각 회귀), Rust 올인 (Lua/씬 파일 미사용).

**스펙:** `docs/superpowers/specs/2026-06-11-voltex-depths-game-design.md`

---

## 확인된 API (구현 시 그대로 사용 — 재확인 불필요)

```rust
// voltex_ai (voltex::ai)
NavAgent { target: Option<Vec3>, speed: f32, path: Vec<Vec3>, current_waypoint: usize, reached: bool }; NavAgent::new(speed)
nav_agent_system(world: &mut World, navmesh: &NavMesh, dt: f32)
NavMeshBuilder { cell_size, cell_height, agent_height, agent_radius, max_slope }  // .build(&[Vec3], &[u32]) -> NavMesh
find_path(navmesh: &NavMesh, start: Vec3, goal: Vec3) -> Option<Vec<Vec3>>

// voltex_runtime
RuntimeApp::set_camera_fn(FnMut(&mut Camera, &World, f32));  // Camera { position, yaw, pitch, fov_y, aspect, .. } + view_projection() -> Mat4
RuntimeApp::set_collision_fn(FnMut(&[ContactPoint], &mut World, f32));
RuntimeApp::set_hud_fn(FnMut(&mut UiContext, &World));
RuntimeApp::add_action_system(FnMut(&mut World, &InputState, &InputMap, f32));
RuntimeApp::add_system(FnMut(&mut World, f32));
RuntimeConfig::with_audio_files(Vec<PathBuf>)  // enable_audio + hot-reload watch
Timer::once(secs) / Timer::repeating(secs); .tick(dt) -> &mut Self; .just_finished(); .finished()
Lifetime::new(secs)  // 만료 시 despawn_recursive (tick 마지막 단계)
AudioPath(pub String)  // 경로 → clip_id 자동 리졸브. AudioSource 없으면 자동 부착, 있으면 clip_id만 rebind(플래그 보존)

// voltex_audio
AudioSource::new(0u32) → 필드 { playing, looping, volume, spatial, .. }; .play()/.stop()

// voltex_ecs
world.spawn_bundle((A, B, ..)) -> Entity; world.insert_bundle(e, (..));
world.insert_resource(T) / resource::<T>() -> Option<&T> / resource_mut::<T>() / resource_or_insert_with
world.send_event(E); world.drain_events::<E>() -> Vec<E>
world.find_by_tag("player") -> Option<Entity>; world.query::<T>() -> impl Iterator<Item=(Entity, &T)>
world.get::<T>(e) / get_mut::<T>(e) / add(e, T) / despawn_recursive(e) / is_alive(e) / clear()

// voltex_physics
Collider::Sphere { radius: f32 }  // 컴포넌트로 직접 부착
ContactPoint { entity_a, entity_b, normal, depth, point_on_a, point_on_b }  // set_collision_fn으로 전달

// voltex_renderer_ecs (prelude)
PrimitiveMesh::cube(size) / sphere_default(radius) / plane(size); MaterialDesc::color([f32;4])

// voltex_platform
input.mouse_position() -> (f64, f64); input.is_key_just_pressed(KeyCode); input.is_mouse_button_pressed(MouseButton)
app.input_map.bind_axis("move_x", KeyCode::KeyD, KeyCode::KeyA) / bind_key / bind_mouse; map.vector("move_x","move_y", input) -> Vec2

// voltex_math
Mat4::look_at / perspective / inverse() -> Option<Mat4> / mul_vec4(Vec4) (Mat4 * Vec4 연산자도 있음)
Vec3::normalize_or_zero()

// voltex_editor UiContext (HUD)
ui.draw_text(text, x, y, [u8;4]); ui.draw_rect(x, y, w, h, [u8;4]); ui.screen_width / ui.screen_height
```

게임플레이 상수: `CELL = 2.0`(셀당 월드 유닛), 플레이어 y=0.5, 투사체 y=0.6. 창 1280x720 고정(리사이즈 시 조준 오차는 known limitation).

---

### Task 1: 스캐폴드 + 마찰 로그

**Files:**
- Create: `games/voltex_depths/Cargo.toml`, `games/voltex_depths/voltex.toml`, `games/voltex_depths/src/main.rs`, `games/voltex_depths/assets/.gitkeep`, `docs/dogfood-friction.md`
- Modify: `Cargo.toml` (워크스페이스 루트 — members에 `"games/voltex_depths"` 추가, `"games/voltex_survivor"` 줄 38 옆)

- [ ] **Step 1: 파일 생성**

`games/voltex_depths/Cargo.toml` (voltex_survivor 것을 본떠):
```toml
[package]
name = "voltex_depths"
version = "0.1.0"
edition = "2021"
license.workspace = true

[dependencies]
voltex.workspace = true
voltex_runtime.workspace = true
env_logger = { workspace = true }
```

`games/voltex_depths/voltex.toml`:
```toml
[project]
name = "voltex_depths"
version = "0.1.0"

[build]
assets = ["assets/**/*"]
window_title = "Voltex Depths"
window_size = [1280, 720]
```
(`scene`/`scripts` 키 없이 manifest가 통과하는지가 첫 friction 후보 — 실패하면 기록하고 더미 키로 우회)

`src/main.rs` (빈 부트, 컴파일 확인용):
```rust
use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult};

fn main() -> RuntimeResult<()> {
    env_logger::init();
    RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Depths")
            .with_window_size(1280, 720),
    )?
    .run()
}
```

`docs/dogfood-friction.md`:
```markdown
# 도그푸딩 마찰 로그 — voltex_depths

게임 제작 중 발견한 엔진 워크플로 마찰. 항목: 하려던 것 / 없거나 어색했던 것 / 우회 / 제안(P0~P2).
엔진 즉석 수정은 진행을 실제로 막을 때만. 이 로그가 v1.1 로드맵 입력이 된다.

| # | 하려던 것 | 마찰 | 우회 | 제안 | P |
|---|---|---|---|---|---|
```

- [ ] **Step 2: 빌드 확인** — Run: `cargo build -p voltex_depths`. Expected: 성공.
- [ ] **Step 3: Commit** — `git add -A && git commit -m "feat(depths): scaffold voltex_depths game crate"`

---

### Task 2: level.rs — ASCII 맵 파싱 + 좌표 변환

**Files:**
- Create: `games/voltex_depths/src/level.rs`
- Modify: `games/voltex_depths/src/main.rs` (`mod level;` 추가)

- [ ] **Step 1: 실패하는 테스트 작성** (`level.rs` 하단 `#[cfg(test)]`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const T_MAP: &[&str] = &[
        "#########",
        "#...#...#",
        "#.P.D.c.#",
        "#...#.s.#",
        "#########",
    ];

    #[test]
    fn parse_extracts_cells_and_spawns() {
        let m = LevelMap::parse(T_MAP).unwrap();
        assert_eq!((m.w, m.h), (9, 5));
        assert_eq!(m.cell(0, 0), Cell::Wall);
        assert_eq!(m.cell(1, 1), Cell::Floor);
        assert_eq!(m.cell(4, 2), Cell::Door);
        assert_eq!(m.player_spawn, (2, 2));
        assert_eq!(m.enemies, vec![(6, 2, EnemyKind::Chaser), (6, 3, EnemyKind::Shooter)]);
        assert_eq!(m.doors, vec![(4, 2)]);
    }

    #[test]
    fn world_pos_roundtrips_with_cell_of() {
        let m = LevelMap::parse(T_MAP).unwrap();
        let p = m.world_pos(6, 2, 0.5);
        assert_eq!(m.cell_of(p), (6, 2));
        // 맵 중심이 원점: 가운데 셀(4,2)은 x=0, z=0
        let c = m.world_pos(4, 2, 0.0);
        assert!(c.x.abs() < 1e-5 && c.z.abs() < 1e-5);
    }

    #[test]
    fn parse_rejects_map_without_player() {
        assert!(LevelMap::parse(&["###", "#.#", "###"]).is_err());
    }
}
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p voltex_depths level`. Expected: 컴파일 실패 (LevelMap 미정의).
- [ ] **Step 3: 구현**

```rust
//! ASCII 그리드 → 레벨 데이터. 전부 순수 (GPU/World 무관).
use voltex::prelude::*;
use crate::components::EnemyKind;

pub const CELL: f32 = 2.0;
pub const WALL_H: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell { Wall, Floor, Door }

#[derive(Clone)]
pub struct LevelMap {
    pub w: usize,
    pub h: usize,
    cells: Vec<Cell>,
    pub player_spawn: (usize, usize),
    pub enemies: Vec<(usize, usize, EnemyKind)>,
    pub doors: Vec<(usize, usize)>,
    pub regions: Vec<Option<usize>>, // Task 3
    pub region_count: usize,         // Task 3
    pub door_gates: Vec<usize>,      // Task 3: doors[i]가 열리는 조건 region
}

impl LevelMap {
    pub fn parse(rows: &[&str]) -> Result<Self, String> {
        let h = rows.len();
        let w = rows.first().map(|r| r.len()).ok_or("empty map")?;
        let mut cells = vec![Cell::Wall; w * h];
        let mut player = None;
        let mut enemies = Vec::new();
        let mut doors = Vec::new();
        for (z, row) in rows.iter().enumerate() {
            if row.len() != w {
                return Err(format!("row {z} width {} != {w}", row.len()));
            }
            for (x, ch) in row.chars().enumerate() {
                cells[z * w + x] = match ch {
                    '#' => Cell::Wall,
                    'D' => { doors.push((x, z)); Cell::Door }
                    '.' => Cell::Floor,
                    'P' => { player = Some((x, z)); Cell::Floor }
                    'c' => { enemies.push((x, z, EnemyKind::Chaser)); Cell::Floor }
                    's' => { enemies.push((x, z, EnemyKind::Shooter)); Cell::Floor }
                    'B' => { enemies.push((x, z, EnemyKind::Boss)); Cell::Floor }
                    other => return Err(format!("unknown cell '{other}' at ({x},{z})")),
                };
            }
        }
        let mut map = Self {
            w, h, cells,
            player_spawn: player.ok_or("map has no 'P' player spawn")?,
            enemies, doors,
            regions: Vec::new(), region_count: 0, door_gates: Vec::new(),
        };
        map.compute_regions(); // Task 3에서 구현 (지금은 빈 함수로 두면 테스트 통과)
        Ok(map)
    }

    pub fn cell(&self, x: usize, z: usize) -> Cell {
        self.cells[z * self.w + x]
    }

    /// 맵 중심이 월드 원점. +x = 그리드 x 증가, +z = 그리드 z 증가.
    pub fn world_pos(&self, x: usize, z: usize, y: f32) -> Vec3 {
        Vec3::new(
            (x as f32 - self.w as f32 / 2.0 + 0.5) * CELL,
            y,
            (z as f32 - self.h as f32 / 2.0 + 0.5) * CELL,
        )
    }

    pub fn cell_of(&self, p: Vec3) -> (usize, usize) {
        let x = (p.x / CELL + self.w as f32 / 2.0).floor();
        let z = (p.z / CELL + self.h as f32 / 2.0).floor();
        (x.clamp(0.0, self.w as f32 - 1.0) as usize, z.clamp(0.0, self.h as f32 - 1.0) as usize)
    }

    fn compute_regions(&mut self) {} // Task 3
}
```

`components.rs`는 Task 2 시점에 `EnemyKind`만 필요 — 미리 최소로 생성 (`src/components.rs` + main.rs에 `mod components;`):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind { Chaser, Shooter, Boss }
```

- [ ] **Step 4: 통과 확인** — Run: `cargo test -p voltex_depths level`. Expected: 3 passed.
- [ ] **Step 5: Commit** — `git commit -m "feat(depths): level map parsing + grid<->world coords"`

---

### Task 3: level.rs — region 플러드필 + door gate

문이 잠긴 동안 던전은 region으로 나뉜다(문 셀이 경계). 플레이어 시작 region에서 BFS한 depth 기준으로, 각 문의 **gate region = 인접 두 region 중 depth가 작은 쪽** — 그 region의 적이 전멸하면 문이 열린다.

**Files:**
- Modify: `games/voltex_depths/src/level.rs`

- [ ] **Step 1: 실패하는 테스트**

```rust
    #[test]
    fn regions_split_by_doors_and_gates_point_backward() {
        let m = LevelMap::parse(T_MAP).unwrap();
        // 문 왼쪽(P쪽)과 오른쪽(c쪽)은 다른 region
        let rp = m.region_at(m.player_spawn).unwrap();
        let rc = m.region_at((6, 2)).unwrap();
        assert_ne!(rp, rc);
        assert_eq!(m.region_count, 2);
        // 문(인덱스 0)의 gate = 플레이어쪽 region (depth 0)
        assert_eq!(m.door_gates[0], rp);
        // 문/벽 셀은 region 없음
        assert_eq!(m.region_at((4, 2)), None);
    }

    #[test]
    fn three_room_chain_gates_are_sequential() {
        let m = LevelMap::parse(&[
            "###########",
            "#P.#.c#.B.#",
            "#..D..D...#",
            "###########",
        ]).unwrap();
        assert_eq!(m.region_count, 3);
        let r0 = m.region_at(m.player_spawn).unwrap();
        let r1 = m.region_at((5, 1)).unwrap();
        // 첫 문은 r0 클리어로, 둘째 문은 r1 클리어로 열림
        assert_eq!(m.door_gates[0], r0);
        assert_eq!(m.door_gates[1], r1);
    }
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p voltex_depths regions`. Expected: FAIL (`region_at` 미정의 / region_count 0).
- [ ] **Step 3: 구현** (`compute_regions` 채우기 + `region_at`/`region_of`)

```rust
    pub fn region_at(&self, (x, z): (usize, usize)) -> Option<usize> {
        self.regions[z * self.w + x]
    }

    pub fn region_of(&self, p: Vec3) -> Option<usize> {
        self.region_at(self.cell_of(p))
    }

    fn compute_regions(&mut self) {
        let (w, h) = (self.w, self.h);
        self.regions = vec![None; w * h];
        self.region_count = 0;
        // 1) Floor 셀 플러드필 (Door는 경계)
        for start in 0..w * h {
            if self.cells[start] != Cell::Floor || self.regions[start].is_some() {
                continue;
            }
            let id = self.region_count;
            self.region_count += 1;
            let mut stack = vec![start];
            while let Some(i) = stack.pop() {
                if self.regions[i].is_some() || self.cells[i] != Cell::Floor {
                    continue;
                }
                self.regions[i] = Some(id);
                let (x, z) = (i % w, i / w);
                if x > 0 { stack.push(i - 1); }
                if x + 1 < w { stack.push(i + 1); }
                if z > 0 { stack.push(i - w); }
                if z + 1 < h { stack.push(i + w); }
            }
        }
        // 2) region 그래프 BFS depth (시작 = 플레이어 region), 간선 = 문
        let door_pair = |&(x, z): &(usize, usize)| -> Vec<usize> {
            let mut rs = Vec::new();
            for (nx, nz) in [(x.wrapping_sub(1), z), (x + 1, z), (x, z.wrapping_sub(1)), (x, z + 1)] {
                if nx < self.w && nz < self.h {
                    if let Some(r) = self.region_at((nx, nz)) {
                        if !rs.contains(&r) { rs.push(r); }
                    }
                }
            }
            rs
        };
        let start = self.region_at(self.player_spawn).expect("player on floor");
        let mut depth = vec![usize::MAX; self.region_count];
        depth[start] = 0;
        let mut frontier = vec![start];
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for d in &self.doors {
                let rs = door_pair(d);
                if rs.len() == 2 {
                    for (a, b) in [(rs[0], rs[1]), (rs[1], rs[0])] {
                        if frontier.contains(&a) && depth[b] == usize::MAX {
                            depth[b] = depth[a] + 1;
                            next.push(b);
                        }
                    }
                }
            }
            frontier = next;
        }
        // 3) 문별 gate = 인접 region 중 depth 작은 쪽
        self.door_gates = self.doors.iter().map(|d| {
            let rs = door_pair(d);
            match rs.as_slice() {
                [a, b] => if depth[*a] <= depth[*b] { *a } else { *b },
                [a] => *a, // 막다른 문 — 한쪽 region만
                _ => start,
            }
        }).collect();
    }
```

- [ ] **Step 4: 통과 확인** — Run: `cargo test -p voltex_depths`. Expected: 전부 passed.
- [ ] **Step 5: Commit** — `git commit -m "feat(depths): region flood fill + door gate assignment"`

---

### Task 4: level.rs — 벽 충돌 슬라이드 + 시야(LOS)

**Files:**
- Modify: `games/voltex_depths/src/level.rs`

- [ ] **Step 1: 실패하는 테스트**

```rust
    #[test]
    fn try_move_blocks_walls_and_slides() {
        let m = LevelMap::parse(T_MAP).unwrap();
        let p = m.world_pos(1, 1, 0.5);
        // 왼쪽은 벽: x 이동은 막히고 z 이동(대각 입력의 z 성분)은 살아남는다
        let out = try_move(&m, p, Vec3::new(-1.0, 0.0, 0.6), 0.4);
        assert!(out.x >= p.x - 1e-4, "wall must block -x");
        assert!(out.z > p.z, "z component must slide");
    }

    #[test]
    fn closed_door_blocks_open_door_passes() {
        let mut m = LevelMap::parse(T_MAP).unwrap();
        let before = m.world_pos(3, 2, 0.5);
        let blocked = try_move(&m, before, Vec3::new(CELL, 0.0, 0.0), 0.4);
        assert!(blocked.x < m.world_pos(4, 2, 0.0).x - 0.5, "closed door blocks");
        m.open_door((4, 2));
        let passed = try_move(&m, before, Vec3::new(CELL, 0.0, 0.0), 0.4);
        assert!(passed.x > blocked.x, "open door passes");
    }

    #[test]
    fn los_blocked_by_wall_clear_in_open() {
        let m = LevelMap::parse(T_MAP).unwrap();
        let a = m.world_pos(1, 1, 0.5);
        let b = m.world_pos(1, 3, 0.5);   // 같은 방 — 클리어
        assert!(m.los_clear(a, b));
        let c = m.world_pos(6, 2, 0.5);   // 문/벽 너머 — 차단
        assert!(!m.los_clear(a, c));
    }
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p voltex_depths`. Expected: FAIL.
- [ ] **Step 3: 구현**

```rust
    pub fn is_blocked(&self, x: usize, z: usize) -> bool {
        !matches!(self.cell(x, z), Cell::Floor)
    }

    pub fn blocked_at(&self, p: Vec3) -> bool {
        let (x, z) = self.cell_of(p);
        self.is_blocked(x, z)
    }

    /// 문 개방: Door → Floor. region 데이터는 재계산하지 않는다 (gate 판정은
    /// 빌드 시점 스냅샷으로 충분 — 열린 문의 region_of(None)는 호출부에서 처리).
    pub fn open_door(&mut self, (x, z): (usize, usize)) {
        self.cells[z * self.w + x] = Cell::Floor;
    }

    /// 0.25셀 간격 샘플링 시야 체크 (그리드 기반 — 물리 raycast 불필요).
    pub fn los_clear(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let steps = ((d.length() / (CELL * 0.25)).ceil() as usize).max(1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            if self.blocked_at(a + d * t) {
                return false;
            }
        }
        true
    }

/// 축 분리 이동: x/z 각각 독립으로 시도 → 벽에 비비면 미끄러진다.
/// radius 만큼 진행 방향으로 패딩해 모서리 파고들기를 줄인다 (완전한 원-AABB는 YAGNI).
pub fn try_move(map: &LevelMap, from: Vec3, delta: Vec3, radius: f32) -> Vec3 {
    let mut pos = from;
    let probe_x = Vec3::new(pos.x + delta.x + delta.x.signum() * radius, pos.y, pos.z);
    if !map.blocked_at(probe_x) {
        pos.x += delta.x;
    }
    let probe_z = Vec3::new(pos.x, pos.y, pos.z + delta.z + delta.z.signum() * radius);
    if !map.blocked_at(probe_z) {
        pos.z += delta.z;
    }
    pos
}
```
(`Vec3::length()`가 없으면 `voltex_math` Vec3 API에서 실제 이름(`magnitude` 등) 확인 — crates/voltex_math/src/vec3.rs)

- [ ] **Step 4: 통과 확인** — Run: `cargo test -p voltex_depths`. Expected: passed.
- [ ] **Step 5: Commit** — `git commit -m "feat(depths): grid collision slide + sampled line-of-sight"`

---

### Task 5: level.rs — 워크블 지오메트리 + NavMesh 빌드

NavMesh 입력은 **바닥 삼각형만** (Floor + Door 셀당 쿼드 2 삼각형, y=0). 벽은 지오메트리에 없으므로 워크블 경계가 곧 벽. 문은 항상 워크블 — 닫힌 문 통과는 적 활성화 로직(같은 region일 때만 추적)이 막는다.

**Files:**
- Modify: `games/voltex_depths/src/level.rs`, `games/voltex_depths/Cargo.toml` (필요시 — `voltex::ai`는 facade 경유라 추가 dep 없음)

- [ ] **Step 1: 실패하는 테스트**

```rust
    #[test]
    fn geometry_emits_quads_for_walkable_cells_only() {
        let m = LevelMap::parse(&["####", "#P.#", "####"]).unwrap();
        let (verts, idx) = level_geometry(&m);
        assert_eq!(verts.len(), 2 * 4); // 2 walkable cells × 4 verts
        assert_eq!(idx.len(), 2 * 6);   // × 2 tri × 3
        assert!(verts.iter().all(|v| v.y == 0.0));
    }

    #[test]
    fn navmesh_path_detours_around_wall() {
        // 가운데 벽 기둥을 우회해야 하는 ㄷ자 맵
        let m = LevelMap::parse(&[
            "#######",
            "#P....#",
            "#.###.#",
            "#.....#",
            "#######",
        ]).unwrap();
        let nav = build_navmesh(&m);
        let start = m.world_pos(1, 1, 0.0);
        let goal = m.world_pos(5, 1, 0.0);
        let path = voltex::ai::find_path(&nav, start, goal)
            .expect("path must exist around the pillar");
        // 직선이면 2 waypoint — 우회는 더 많다
        assert!(path.len() > 2, "expected detour, got straight line: {path:?}");
        // 경로 위 샘플이 벽을 통과하지 않는다
        for w in path.windows(2) {
            assert!(m.los_clear(
                Vec3::new(w[0].x, 0.5, w[0].z),
                Vec3::new(w[1].x, 0.5, w[1].z),
            ), "path segment crosses a wall: {w:?}");
        }
    }
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p voltex_depths navmesh`. Expected: 컴파일 실패.
- [ ] **Step 3: 구현**

```rust
use voltex::ai::{NavMesh, NavMeshBuilder};

pub fn level_geometry(map: &LevelMap) -> (Vec<Vec3>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idx = Vec::new();
    let half = CELL / 2.0;
    for z in 0..map.h {
        for x in 0..map.w {
            if map.is_blocked(x, z) && map.cell(x, z) != Cell::Door {
                continue;
            }
            let c = map.world_pos(x, z, 0.0);
            let base = verts.len() as u32;
            verts.push(Vec3::new(c.x - half, 0.0, c.z - half));
            verts.push(Vec3::new(c.x + half, 0.0, c.z - half));
            verts.push(Vec3::new(c.x + half, 0.0, c.z + half));
            verts.push(Vec3::new(c.x - half, 0.0, c.z + half));
            idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    (verts, idx)
}

pub fn build_navmesh(map: &LevelMap) -> NavMesh {
    let (verts, idx) = level_geometry(&map.clone());
    NavMeshBuilder {
        cell_size: 0.5,
        cell_height: 0.2,
        agent_height: 2.0,
        agent_radius: 0.4,
        max_slope: 45.0,
    }
    .build(&verts, &idx)
}
```
주의: 삼각형 winding이 voxelizer의 walkable 판정(법선 위쪽)과 맞아야 한다. 위 인덱스 순서로 빌드한 navmesh에서 `find_path`가 `None`이면 winding을 뒤집어 본다 (`[base, base+1, base+2, base, base+2, base+3]`). 둘 다 실패하면 **friction 기록** 후 `NavMeshBuilder` 단위 테스트(crates/voltex_ai/src/navmesh_builder/)에서 기대 입력 포맷을 확인.

- [ ] **Step 4: 통과 확인** — Run: `cargo test -p voltex_depths`. Expected: passed. (이 테스트가 이 게임의 첫 NavMeshBuilder 실전 검증 — 실패하면 엔진 버그 가능성, systematic-debugging으로)
- [ ] **Step 5: Commit** — `git commit -m "feat(depths): walkable geometry + navmesh build with detour regression"`

---

### Task 6: components.rs — 게임 컴포넌트/리소스/이벤트 전체

**Files:**
- Modify: `games/voltex_depths/src/components.rs`

- [ ] **Step 1: 구현** (데이터 정의 — 동작 없음, 테스트는 사용처 태스크에서)

```rust
//! 게임 컴포넌트/리소스/이벤트. 씬 직렬화 안 하므로 Component trait 불필요.
use voltex::prelude::*;
use voltex_runtime::Timer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase { Title, Playing, GameOver, Victory }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Team { Player, Enemy }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind { Chaser, Shooter, Boss }

#[derive(Debug, Clone, Copy)]
pub struct Velocity(pub Vec3);

#[derive(Debug, Clone, Copy)]
pub struct Health { pub hp: i32 }

#[derive(Debug, Clone, Copy)]
pub struct Projectile { pub team: Team, pub damage: i32 }

pub struct Enemy {
    pub kind: EnemyKind,
    pub region: usize,
    pub shoot: Timer,        // 사수 발사 주기 / 보스 패턴 주기
    pub touch: Timer,        // 접촉 데미지 쿨다운
    pub boss_dash_next: bool,
}

impl Enemy {
    pub fn new(kind: EnemyKind, region: usize) -> Self {
        let shoot = match kind {
            EnemyKind::Shooter => Timer::repeating(1.4),
            EnemyKind::Boss => Timer::repeating(2.5),
            EnemyKind::Chaser => Timer::repeating(f32::MAX),
        };
        Self { kind, region, shoot, touch: Timer::repeating(0.8), boss_dash_next: true }
    }
}

pub struct Door { pub gate_region: usize, pub cell: (usize, usize) }

/// 이벤트: 적 사망 (door_system이 RegionCounts 갱신에 사용)
pub struct EnemyKilled { pub region: usize }

// ---- World 리소스 (world.clear() 시 함께 소멸 — start_game이 재삽입) ----
pub struct Score(pub u32);
pub struct RegionCounts(pub Vec<u32>);
```

기존 `EnemyKind`(Task 2의 최소판)는 이 파일로 대체.

- [ ] **Step 2: 빌드 확인** — Run: `cargo build -p voltex_depths`. Expected: 성공 (dead_code 워닝은 사용처 태스크에서 사라짐 — 임시 `#[allow(dead_code)]`는 모듈 단위 1줄만).
- [ ] **Step 3: Commit** — `git commit -m "feat(depths): game components, resources, events"`

---### Task 7: level.rs — build_level 엔티티 시딩

**Files:**
- Modify: `games/voltex_depths/src/level.rs`

- [ ] **Step 1: 실패하는 테스트**

```rust
    #[test]
    fn build_level_seeds_entities_and_region_counts() {
        let m = LevelMap::parse(T_MAP).unwrap();
        let mut w = World::new();
        build_level(&mut w, &m);
        assert!(w.find_by_tag("player").is_some());
        let enemies: Vec<_> = w.query::<crate::components::Enemy>().collect();
        assert_eq!(enemies.len(), 2);
        let doors: Vec<_> = w.query::<crate::components::Door>().collect();
        assert_eq!(doors.len(), 1);
        // RegionCounts: 적 2 모두 오른쪽 region
        let rc = w.resource::<crate::components::RegionCounts>().unwrap();
        assert_eq!(rc.0.iter().sum::<u32>(), 2);
        // 플레이어에 Health/Collider 부착
        let p = w.find_by_tag("player").unwrap();
        assert_eq!(w.get::<crate::components::Health>(p).unwrap().hp, 5);
        assert!(w.get::<voltex::physics::Collider>(p).is_some());
    }
```
(`World::new()` 시그니처가 다르면 voltex_ecs 테스트들이 쓰는 생성 방식을 따른다)

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p voltex_depths build_level`. Expected: FAIL.
- [ ] **Step 3: 구현**

```rust
use voltex::ecs::Tag;
use voltex::physics::Collider;
use crate::components::*;

pub const PLAYER_Y: f32 = 0.5;

pub fn build_level(world: &mut World, map: &LevelMap) {
    // 바닥 — 맵 전체 크기의 납작 큐브
    let mut t = Transform::from_position(Vec3::new(0.0, -0.05, 0.0));
    t.scale = Vec3::new(map.w as f32 * CELL, 0.1, map.h as f32 * CELL);
    world.spawn_bundle((t, PrimitiveMesh::cube(1.0), MaterialDesc::color([0.22, 0.22, 0.26, 1.0])));

    // 벽 + 문
    for z in 0..map.h {
        for x in 0..map.w {
            match map.cell(x, z) {
                Cell::Wall => {
                    let mut t = Transform::from_position(map.world_pos(x, z, WALL_H / 2.0));
                    t.scale = Vec3::new(CELL, WALL_H, CELL);
                    world.spawn_bundle((t, PrimitiveMesh::cube(1.0), MaterialDesc::color([0.45, 0.42, 0.50, 1.0])));
                }
                Cell::Door => {
                    let mut t = Transform::from_position(map.world_pos(x, z, WALL_H / 2.0));
                    t.scale = Vec3::new(CELL, WALL_H, CELL);
                    let gate = map.doors.iter().position(|&d| d == (x, z))
                        .map(|i| map.door_gates[i]).unwrap_or(0);
                    world.spawn_bundle((
                        t, PrimitiveMesh::cube(1.0), MaterialDesc::color([0.85, 0.65, 0.20, 1.0]),
                        Door { gate_region: gate, cell: (x, z) }, Tag("door".into()),
                    ));
                }
                Cell::Floor => {}
            }
        }
    }

    // 플레이어
    world.spawn_bundle((
        Transform::from_position(map.world_pos(map.player_spawn.0, map.player_spawn.1, PLAYER_Y)),
        Tag("player".into()),
        PrimitiveMesh::cube(0.8),
        MaterialDesc::color([0.20, 0.85, 0.35, 1.0]),
        Health { hp: 5 },
        Collider::Sphere { radius: 0.5 },
    ));

    // 적 + region 카운트
    let mut counts = vec![0u32; map.region_count.max(1)];
    for &(x, z, kind) in &map.enemies {
        let region = map.region_at((x, z)).unwrap_or(0);
        counts[region] += 1;
        let (size, color, speed, hp) = match kind {
            EnemyKind::Chaser => (0.45, [0.90, 0.25, 0.20, 1.0], 3.2, 2),
            EnemyKind::Shooter => (0.45, [0.95, 0.55, 0.15, 1.0], 2.4, 2),
            EnemyKind::Boss => (1.1, [0.60, 0.20, 0.80, 1.0], 2.0, 30),
        };
        world.spawn_bundle((
            Transform::from_position(map.world_pos(x, z, PLAYER_Y)),
            PrimitiveMesh::sphere_default(size),
            MaterialDesc::color(color),
            Enemy::new(kind, region),
            voltex::ai::NavAgent::new(speed),
            Health { hp },
            Collider::Sphere { radius: size },
            Velocity(Vec3::new(0.0, 0.0, 0.0)),
        ));
    }
    world.insert_resource(RegionCounts(counts));
    world.insert_resource(Score(0));
}
```
(spawn_bundle 튜플은 최대 12 컴포넌트 — 적 번들 8개라 여유)

- [ ] **Step 4: 통과 확인** — Run: `cargo test -p voltex_depths`. Expected: passed.
- [ ] **Step 5: Commit** — `git commit -m "feat(depths): build_level entity seeding"`

---

### Task 8: combat.rs — 마우스 조준 unprojection

카메라 VP 역행렬로 마우스 → y=PLAYER_Y 평면 교점을 구한다. wgpu NDC: x,y ∈ [-1,1] (y 위가 +), z ∈ [0,1].

**Files:**
- Create: `games/voltex_depths/src/combat.rs` (+ main.rs `mod combat;`)

- [ ] **Step 1: 실패하는 테스트**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use voltex::prelude::*;

    #[test]
    fn ndc_maps_screen_corners() {
        assert_eq!(ndc_from_mouse(0.0, 0.0, 1280.0, 720.0), (-1.0, 1.0));
        let (cx, cy) = ndc_from_mouse(640.0, 360.0, 1280.0, 720.0);
        assert!(cx.abs() < 1e-6 && cy.abs() < 1e-6);
    }

    #[test]
    fn unproject_center_hits_look_target() {
        // 위에서 원점을 내려다보는 카메라: 화면 중앙은 평면 원점 근처를 맞춘다
        let eye = Vec3::new(0.0, 10.0, 5.0);
        let vp = Mat4::perspective(45f32.to_radians(), 16.0 / 9.0, 0.1, 100.0)
            .mul(&Mat4::look_at(eye, Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 1.0, 0.0)));
        let inv = vp.inverse().unwrap();
        let hit = unproject_to_plane(&inv, (0.0, 0.0), 0.5).unwrap();
        assert!(hit.x.abs() < 0.1 && hit.z.abs() < 0.1, "center must hit look target: {hit:?}");
        // 화면 오른쪽은 +x쪽
        let right = unproject_to_plane(&inv, (0.5, 0.0), 0.5).unwrap();
        assert!(right.x > hit.x);
    }
}
```
(`Mat4`에 `mul(&Mat4)`가 없고 `Mul` 연산자만 있으면 `vp = proj * view` 형태로 수정 — crates/voltex_math/src/mat4.rs 확인. 곱 순서는 voltex_renderer `Camera::view_projection()` 구현과 동일하게.)

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p voltex_depths combat`. Expected: 컴파일 실패.
- [ ] **Step 3: 구현**

```rust
//! 전투: 조준 unprojection, 투사체, 피격/사망.
use voltex::prelude::*;

pub fn ndc_from_mouse(mx: f64, my: f64, w: f32, h: f32) -> (f32, f32) {
    ((2.0 * mx as f32 / w) - 1.0, 1.0 - (2.0 * my as f32 / h))
}

/// NDC → 월드 레이 → y=plane_y 평면 교점. 레이가 평면과 평행하면 None.
pub fn unproject_to_plane(vp_inv: &Mat4, ndc: (f32, f32), plane_y: f32) -> Option<Vec3> {
    let h = |v: Vec4| -> Option<Vec3> {
        if v.w.abs() < 1e-9 { return None; }
        Some(Vec3::new(v.x / v.w, v.y / v.w, v.z / v.w))
    };
    let near = h(vp_inv.mul_vec4(Vec4::new(ndc.0, ndc.1, 0.0, 1.0)))?;
    let far = h(vp_inv.mul_vec4(Vec4::new(ndc.0, ndc.1, 1.0, 1.0)))?;
    let dir = far - near;
    if dir.y.abs() < 1e-6 { return None; }
    let t = (plane_y - near.y) / dir.y;
    if t < 0.0 { return None; }
    Some(near + dir * t)
}
```

- [ ] **Step 4: 통과 확인** — Run: `cargo test -p voltex_depths combat`. Expected: passed. **center 테스트가 실패하면 NDC z 규약(0..1 vs -1..1)이 원인일 가능성 1순위** — `Mat4::perspective`가 어느 클립 공간을 쓰는지 mat4.rs에서 확인하고 near z를 맞춘다.
- [ ] **Step 5: Commit** — `git commit -m "feat(depths): mouse aim unprojection to ground plane"`

---

### Task 9: combat.rs — 투사체 스폰/이동/벽 소멸

**Files:**
- Modify: `games/voltex_depths/src/combat.rs`

- [ ] **Step 1: 실패하는 테스트**

```rust
    #[test]
    fn projectile_moves_and_dies_on_wall() {
        let map = crate::level::LevelMap::parse(&["#####", "#P..#", "#####"]).unwrap();
        let mut w = World::new();
        let from = map.world_pos(1, 1, 0.6);
        spawn_projectile(&mut w, from, Vec3::new(1.0, 0.0, 0.0), crate::components::Team::Player, 1, 14.0);
        let e = w.query::<crate::components::Projectile>().map(|(e, _)| e).next().unwrap();
        projectile_system(&mut w, &map, 0.1);
        let after = w.get::<Transform>(e).unwrap().position;
        assert!(after.x > from.x, "projectile must advance +x");
        // 벽까지 충분히 진행시키면 despawn
        for _ in 0..40 { projectile_system(&mut w, &map, 0.1); }
        assert!(!w.is_alive(e), "projectile must despawn on wall");
    }
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p voltex_depths projectile`. Expected: FAIL.
- [ ] **Step 3: 구현**

```rust
use voltex::ecs::Tag;
use voltex::physics::Collider;
use voltex_runtime::Lifetime;
use crate::components::*;
use crate::level::LevelMap;

pub fn spawn_projectile(world: &mut World, from: Vec3, dir: Vec3, team: Team, damage: i32, speed: f32) {
    let d = dir.normalize_or_zero();
    if d.x == 0.0 && d.z == 0.0 { return; }
    let color = match team {
        Team::Player => [1.0, 0.95, 0.35, 1.0],
        Team::Enemy => [0.95, 0.30, 0.60, 1.0],
    };
    world.spawn_bundle((
        Transform::from_position(from),
        PrimitiveMesh::sphere_default(0.18),
        MaterialDesc::color(color),
        Projectile { team, damage },
        Velocity(d * speed),
        Collider::Sphere { radius: 0.25 },
        Lifetime::new(3.0),
    ));
}

/// 투사체 이동 + 벽 셀 진입 시 즉시 소멸 (Lifetime은 안전망).
pub fn projectile_system(world: &mut World, map: &LevelMap, dt: f32) {
    let moves: Vec<(Entity, Vec3)> = world
        .query::<Projectile>()
        .filter_map(|(e, _)| {
            let v = world.get::<Velocity>(e)?.0;
            Some((e, v * dt))
        })
        .collect();
    for (e, dp) in moves {
        let mut dead = false;
        if let Some(t) = world.get_mut::<Transform>(e) {
            t.position = t.position + dp;
            dead = map.blocked_at(t.position);
        }
        if dead {
            world.despawn_recursive(e);
        }
    }
}
```

- [ ] **Step 4: 통과 확인** — Run: `cargo test -p voltex_depths`. Expected: passed.
- [ ] **Step 5: Commit** — `git commit -m "feat(depths): projectile spawn/move/wall-despawn"`

---

### Task 10: combat.rs — 접촉 데미지 매트릭스 + 사망 처리

`set_collision_fn`의 contacts를 받는 순수 핸들러 + 사망 스윕. 페어 매트릭스:
- 플레이어팀 투사체 × Enemy → 적 hp-=, 투사체 소멸
- 적팀 투사체 × player(Tag) → 플레이어 hp-=, 투사체 소멸
- Enemy(Chaser/Boss) × player → enemy.touch 쿨다운 끝났으면 hp-1, 쿨다운 리셋
- 그 외 (적끼리, 투사체끼리, 같은 팀) → 무시

**Files:**
- Modify: `games/voltex_depths/src/combat.rs`

- [ ] **Step 1: 실패하는 테스트**

```rust
    use voltex::physics::ContactPoint;

    fn contact(a: Entity, b: Entity) -> ContactPoint {
        ContactPoint {
            entity_a: a, entity_b: b,
            normal: Vec3::new(0.0, 1.0, 0.0), depth: 0.1,
            point_on_a: Vec3::new(0.0, 0.0, 0.0), point_on_b: Vec3::new(0.0, 0.0, 0.0),
        }
    }

    #[test]
    fn player_projectile_damages_enemy_and_despawns() {
        let mut w = World::new();
        let enemy = w.spawn_bundle((Transform::new(), crate::components::Enemy::new(EnemyKind::Chaser, 0), Health { hp: 2 }));
        let proj = w.spawn_bundle((Transform::new(), Projectile { team: Team::Player, damage: 1 }, Velocity(Vec3::new(0.0,0.0,0.0))));
        handle_contacts(&[contact(proj, enemy)], &mut w);
        assert_eq!(w.get::<Health>(enemy).unwrap().hp, 1);
        assert!(!w.is_alive(proj));
    }

    #[test]
    fn chaser_touch_damages_player_once_per_cooldown() {
        let mut w = World::new();
        let player = w.spawn_bundle((Transform::new(), voltex::ecs::Tag("player".into()), Health { hp: 5 }));
        let enemy = w.spawn_bundle((Transform::new(), crate::components::Enemy::new(EnemyKind::Chaser, 0), Health { hp: 2 }));
        handle_contacts(&[contact(enemy, player)], &mut w);
        handle_contacts(&[contact(enemy, player)], &mut w); // 쿨다운 내 재접촉
        assert_eq!(w.get::<Health>(player).unwrap().hp, 4, "touch damage must respect cooldown");
    }

    #[test]
    fn death_sweep_despawns_sends_event_and_reports() {
        let mut w = World::new();
        build_test_arena(&mut w); // 헬퍼: player(hp5) + chaser(hp0, region1) + boss(hp0)
        let out = death_system(&mut w);
        assert!(out.boss_died);
        assert!(!out.player_died);
        let killed = w.drain_events::<crate::components::EnemyKilled>();
        assert_eq!(killed.len(), 2);
        assert_eq!(w.query::<crate::components::Enemy>().count(), 0);
    }
```
(`build_test_arena`는 테스트 모듈 내 헬퍼로 — hp 0짜리 chaser(region 1)/boss + hp 5 player를 spawn_bundle)

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p voltex_depths`. Expected: FAIL.
- [ ] **Step 3: 구현**

```rust
/// touch 쿨다운 진행은 enemy_ai_system이 tick — 여기서는 finished 검사/리셋만.
pub fn handle_contacts(contacts: &[voltex::physics::ContactPoint], world: &mut World) {
    for c in contacts {
        for (x, y) in [(c.entity_a, c.entity_b), (c.entity_b, c.entity_a)] {
            // 투사체 → 상대
            if let Some(p) = world.get::<Projectile>(x).copied() {
                let hit_enemy = p.team == Team::Player && world.get::<Enemy>(y).is_some();
                let hit_player = p.team == Team::Enemy
                    && world.get::<Tag>(y).map(|t| t.0 == "player").unwrap_or(false);
                if hit_enemy || hit_player {
                    if let Some(h) = world.get_mut::<Health>(y) {
                        h.hp -= p.damage;
                    }
                    world.despawn_recursive(x);
                    continue;
                }
            }
            // 근접 적 → 플레이어 접촉 데미지
            let is_player = world.get::<Tag>(y).map(|t| t.0 == "player").unwrap_or(false);
            if is_player {
                let ready = world.get::<Enemy>(x)
                    .map(|e| e.kind != EnemyKind::Shooter && e.touch.finished())
                    .unwrap_or(false);
                if ready {
                    if let Some(h) = world.get_mut::<Health>(y) { h.hp -= 1; }
                    if let Some(e) = world.get_mut::<Enemy>(x) { e.touch.reset(); }
                }
            }
        }
    }
}

pub struct DeathOutcome { pub player_died: bool, pub boss_died: bool }

pub fn death_system(world: &mut World) -> DeathOutcome {
    let mut out = DeathOutcome { player_died: false, boss_died: false };
    let dead: Vec<Entity> = world.query::<Health>()
        .filter(|(_, h)| h.hp <= 0)
        .map(|(e, _)| e)
        .collect();
    let mut killed = Vec::new();
    for e in dead {
        if let Some(en) = world.get::<Enemy>(e) {
            if en.kind == EnemyKind::Boss { out.boss_died = true; }
            killed.push(EnemyKilled { region: en.region });
            world.despawn_recursive(e);
        } else if world.get::<Tag>(e).map(|t| t.0 == "player").unwrap_or(false) {
            out.player_died = true; // despawn하지 않음 — phase 전환이 처리
        }
    }
    for k in killed {
        if let Some(s) = world.resource_mut::<Score>() { s.0 += 100; }
        world.send_event(k);
    }
    out
}
```
(`Timer`에 `reset()`/`finished()` 있음 — CODEMAP 확인됨. `Projectile`이 Copy 안 되면 `.cloned()`로. Borrow 충돌 나면 스냅샷 패턴: 먼저 `(Entity, 판정)` 수집 후 적용 — flocking_system 방식)

- [ ] **Step 4: 통과 확인** — Run: `cargo test -p voltex_depths`. Expected: passed.
- [ ] **Step 5: Commit** — `git commit -m "feat(depths): contact damage matrix + death sweep with events"`

---

### Task 11: enemy.rs — 추적자 AI (NavAgent 재경로) + 속도 적분

**Files:**
- Create: `games/voltex_depths/src/enemy.rs` (+ main.rs `mod enemy;`)

- [ ] **Step 1: 실패하는 테스트**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use voltex::prelude::*;
    use crate::components::*;
    use crate::level::*;

    /// 기둥 뒤의 추적자가 NavMesh를 타고 플레이어에게 접근한다 — 첫 실전 E2E.
    #[test]
    fn chaser_approaches_player_around_pillar() {
        let m = LevelMap::parse(&[
            "#######",
            "#P....#",
            "#.###.#",
            "#..c..#",
            "#######",
        ]).unwrap();
        let nav = build_navmesh(&m);
        let mut w = World::new();
        build_level(&mut w, &m);
        let player = w.find_by_tag("player").unwrap();
        let p_pos = w.get::<Transform>(player).unwrap().position;
        let enemy = w.query::<Enemy>().map(|(e, _)| e).next().unwrap();
        let d0 = (w.get::<Transform>(enemy).unwrap().position - p_pos).length();

        for _ in 0..240 { // 4초 시뮬레이션
            enemy_ai_system(&mut w, &m, &nav, 1.0 / 60.0, true);
            voltex::ai::nav_agent_system(&mut w, &nav, 1.0 / 60.0);
        }
        let d1 = (w.get::<Transform>(enemy).unwrap().position - p_pos).length();
        assert!(d1 < d0 * 0.5, "chaser must close distance via navmesh: {d0} -> {d1}");
    }

    #[test]
    fn enemy_in_other_region_stays_idle() {
        let m = LevelMap::parse(super::super::level::tests::T_MAP.try_into().unwrap()).unwrap();
        // ↑ 위가 번거로우면 T_MAP을 crate-public 헬퍼로 승격: pub(crate) fn test_map() -> LevelMap
        let nav = build_navmesh(&m);
        let mut w = World::new();
        build_level(&mut w, &m);
        let enemy = w.query::<Enemy>().map(|(e, _)| e).next().unwrap();
        let before = w.get::<Transform>(enemy).unwrap().position;
        for _ in 0..120 {
            enemy_ai_system(&mut w, &m, &nav, 1.0 / 60.0, true);
            voltex::ai::nav_agent_system(&mut w, &nav, 1.0 / 60.0);
        }
        let after = w.get::<Transform>(enemy).unwrap().position;
        assert!((after - before).length() < 0.3, "enemy must idle until player enters its region");
    }
}
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p voltex_depths enemy`. Expected: 컴파일 실패.
- [ ] **Step 3: 구현**

```rust
//! 적 AI: region 활성화 → 추적(NavAgent)/사수/보스. 이동 적분은
//! nav_agent_system(kinematic)과 velocity_system(보스 돌진)이 담당.
use voltex::ai::{NavAgent, NavMesh};
use voltex::prelude::*;
use crate::combat::spawn_projectile;
use crate::components::*;
use crate::level::{try_move, LevelMap, PLAYER_Y};

const SHOOTER_RANGE: f32 = 9.0;
const SHOOTER_KEEP: f32 = 5.0;

pub fn enemy_ai_system(world: &mut World, map: &LevelMap, _nav: &NavMesh, dt: f32, repath: bool) {
    let Some(player) = world.find_by_tag("player") else { return };
    let p_pos = match world.get::<Transform>(player) { Some(t) => t.position, None => return };
    let p_region = map.region_of(p_pos);

    let ids: Vec<Entity> = world.query::<Enemy>().map(|(e, _)| e).collect();
    let mut shots: Vec<(Vec3, Vec3)> = Vec::new();      // (from, dir)
    let mut rings: Vec<Vec3> = Vec::new();              // boss 탄막 중심
    for e in ids {
        let pos = match world.get::<Transform>(e) { Some(t) => t.position, None => continue };
        let (kind, region) = match world.get::<Enemy>(e) { Some(en) => (en.kind, en.region), None => continue };
        // region 활성화: 플레이어가 같은 region에 있을 때만 (열린 문 위 = None이면 비활성 유지)
        let active = p_region == Some(region);
        if let Some(en) = world.get_mut::<Enemy>(e) {
            en.touch.tick(dt);
            if !active { en.shoot.tick(dt); continue; } // 쿨다운은 흘려보냄
        }
        if !active { continue; }

        match kind {
            EnemyKind::Chaser => {
                if repath {
                    if let Some(a) = world.get_mut::<NavAgent>(e) { a.target = Some(p_pos); }
                }
            }
            EnemyKind::Shooter => {
                let dist = (p_pos - pos).length();
                let sees = map.los_clear(pos, p_pos);
                if sees && dist <= SHOOTER_RANGE {
                    if let Some(a) = world.get_mut::<NavAgent>(e) { a.target = None; }
                    let fire = world.get_mut::<Enemy>(e)
                        .map(|en| en.shoot.tick(dt).just_finished()).unwrap_or(false);
                    if fire && dist > SHOOTER_KEEP * 0.3 {
                        shots.push((pos, p_pos - pos));
                    }
                } else if repath {
                    if let Some(a) = world.get_mut::<NavAgent>(e) { a.target = Some(p_pos); }
                }
            }
            EnemyKind::Boss => {
                let (fire, dash) = world.get_mut::<Enemy>(e).map(|en| {
                    let f = en.shoot.tick(dt).just_finished();
                    let d = en.boss_dash_next;
                    if f { en.boss_dash_next = !en.boss_dash_next; }
                    (f, d)
                }).unwrap_or((false, false));
                if fire {
                    if dash {
                        let dir = (p_pos - pos).normalize_or_zero();
                        if let Some(v) = world.get_mut::<Velocity>(e) { v.0 = dir * 12.0; }
                    } else {
                        rings.push(pos);
                    }
                }
                if repath {
                    if let Some(a) = world.get_mut::<NavAgent>(e) { a.target = Some(p_pos); }
                }
            }
        }
    }
    for (from, dir) in shots {
        spawn_projectile(world, Vec3::new(from.x, 0.6, from.z), dir, Team::Enemy, 1, 8.0);
    }
    for c in rings {
        for i in 0..12 {
            let a = i as f32 / 12.0 * std::f32::consts::TAU;
            spawn_projectile(world, Vec3::new(c.x, 0.6, c.z), Vec3::new(a.cos(), 0.0, a.sin()), Team::Enemy, 1, 7.0);
        }
    }
}

/// 보스 돌진 적분 + 감쇠. 벽은 try_move로 슬라이드.
pub fn velocity_system(world: &mut World, map: &LevelMap, dt: f32) {
    let ids: Vec<(Entity, Vec3)> = world.query::<Enemy>()
        .filter_map(|(e, _)| {
            let v = world.get::<Velocity>(e)?.0;
            if v.length() < 0.05 { return None; }
            Some((e, v))
        }).collect();
    for (e, v) in ids {
        if let Some(t) = world.get_mut::<Transform>(e) {
            t.position = try_move(map, t.position, v * dt, 0.6);
            t.position.y = PLAYER_Y;
        }
        if let Some(vel) = world.get_mut::<Velocity>(e) {
            vel.0 = vel.0 * (1.0 - (4.0 * dt).min(1.0));
        }
    }
}
```
주의 — borrow 충돌: `world.query::<Enemy>()` 중첩 `world.get` 패턴이 컴파일 안 되면 (World API에 따라) 먼저 스냅샷 `Vec<(Entity, ..)>` 수집 후 별도 루프로 적용 (위 코드는 이미 그 패턴이지만 query 내부 클로저에서 world 접근이 막히면 `.map(|(e,_)| e).collect()` 후 처리로 더 쪼갠다).

- [ ] **Step 4: 통과 확인** — Run: `cargo test -p voltex_depths`. Expected: passed. **`chaser_approaches…`가 NavAgent/NavMeshBuilder 첫 실전 게이트** — 실패 시 systematic-debugging + friction 기록.
- [ ] **Step 5: Commit** — `git commit -m "feat(depths): chaser/shooter/boss AI + boss dash integration"`

---

### Task 12: game_state.rs — 문 개방 + phase 전이

**Files:**
- Create: `games/voltex_depths/src/game_state.rs` (+ main.rs `mod game_state;`)

- [ ] **Step 1: 실패하는 테스트**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use voltex::prelude::*;
    use crate::components::*;
    use crate::level::*;

    #[test]
    fn door_opens_when_gate_region_cleared() {
        let m = LevelMap::parse(&[
            "#########",
            "#...#...#",
            "#.P.D.c.#",
            "#...#...#",
            "#########",
        ]).unwrap();
        // 적은 오른쪽 region — 문 gate는 왼쪽(시작) region이므로 즉시 열려야 함
        let mut map = m.clone();
        let mut w = World::new();
        build_level(&mut w, &map);
        door_system(&mut w, &mut map);
        assert_eq!(w.query::<Door>().count(), 0, "gate region(시작방, 적 0)이 빈 문은 즉시 개방");
        assert!(!map.blocked_at(map.world_pos(4, 2, 0.5)));
    }

    #[test]
    fn door_with_enemies_in_gate_waits_for_kill_events() {
        let m = LevelMap::parse(&[
            "###########",
            "#P.c#..#B.#",
            "#...D..D..#",
            "###########",
        ]).unwrap();
        let mut map = m.clone();
        let mut w = World::new();
        build_level(&mut w, &map);
        let first_door_cell = map.doors[0];
        door_system(&mut w, &mut map);
        assert!(map.blocked_at(map.world_pos(first_door_cell.0, first_door_cell.1, 0.5)), "첫 문은 c 생존 동안 닫힘");
        // c 사망 이벤트 → 문 개방
        let r = map.region_at((3, 1)).unwrap();
        w.send_event(EnemyKilled { region: r });
        door_system(&mut w, &mut map);
        assert!(!map.blocked_at(map.world_pos(first_door_cell.0, first_door_cell.1, 0.5)));
    }
}
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p voltex_depths door`. Expected: 컴파일 실패.
- [ ] **Step 3: 구현**

```rust
//! 문 개방 / 시작·재시작 / HUD.
use voltex::prelude::*;
use voltex::ecs::Tag;
use crate::components::*;
use crate::level::{build_level, LevelMap};

pub fn door_system(world: &mut World, map: &mut LevelMap) {
    for k in world.drain_events::<EnemyKilled>() {
        if let Some(rc) = world.resource_mut::<RegionCounts>() {
            if let Some(n) = rc.0.get_mut(k.region) {
                *n = n.saturating_sub(1);
            }
        }
    }
    let counts = match world.resource::<RegionCounts>() {
        Some(rc) => rc.0.clone(),
        None => return,
    };
    let open: Vec<(Entity, (usize, usize))> = world.query::<Door>()
        .filter(|(_, d)| counts.get(d.gate_region).copied().unwrap_or(0) == 0)
        .map(|(e, d)| (e, d.cell))
        .collect();
    for (e, cell) in open {
        world.despawn_recursive(e);
        map.open_door(cell);
        if let Some(s) = world.resource_mut::<Score>() { s.0 += 250; }
    }
}

/// Playing 진입: 월드 리셋 + 레벨/리소스 재구성. (NavMesh는 불변 — 재빌드 불필요)
pub fn start_game(world: &mut World, map: &mut LevelMap, source: &LevelMap) {
    world.clear();
    *map = source.clone();
    build_level(world, map);
}
```

- [ ] **Step 4: 통과 확인** — Run: `cargo test -p voltex_depths`. Expected: passed.
- [ ] **Step 5: Commit** — `git commit -m "feat(depths): door opening by region clear + start_game reset"`

---

### Task 13: main.rs — 풀 배선 (입력/카메라/HUD/phase)

**Files:**
- Modify: `games/voltex_depths/src/main.rs`

- [ ] **Step 1: 구현** (배선은 GPU/winit 의존이라 테스트는 빌드 + Task 15 헤드리스/스모크로)

```rust
//! Voltex Depths — 탑다운 트윈스틱 던전. 도그푸딩 게임 1편.
//! 설계: docs/superpowers/specs/2026-06-11-voltex-depths-game-design.md
mod combat;
mod components;
mod enemy;
mod game_state;
mod level;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use voltex::ecs::Tag;
use voltex::prelude::*;
use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult, Timer};

use combat::*;
use components::*;
use level::*;

const MAP: &[&str] = &[
    "#############################################################",
    "#.....#..........#..........#..........#..........#........#",
    "#.....#...c..#...#..c..c....#..s...#...#..c..s....#........#",
    "#..P..D......#...D..........D......#...D..........D....B...#",
    "#.....#..........#...##.....#..........#..c.......#........#",
    "#.....#..........#..........#..........#..........#........#",
    "#############################################################",
];

const PLAYER_SPEED: f32 = 6.0;
const SCREEN: (f32, f32) = (1280.0, 720.0);
/// 에셋은 cwd 무관하게 crate 기준 절대 경로로 (cargo run을 repo 루트/크레이트 어디서
/// 실행해도 동작). 배포 exe에서는 baked 절대 경로가 깨짐 — known limitation + friction
/// 기록 (voltex build의 에셋 패키징/Vfs 연동이 엔진 후속 과제).
pub const ASSET_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

fn main() -> RuntimeResult<()> {
    env_logger::init();
    let source = LevelMap::parse(MAP).expect("MAP must parse");
    let nav = Arc::new(build_navmesh(&source));
    let map = Rc::new(RefCell::new(source.clone()));
    let phase = Rc::new(RefCell::new(GamePhase::Title));
    let cam_vp = Rc::new(RefCell::new(Mat4::identity()));

    let mut app = RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Depths")
            .with_window_size(SCREEN.0 as u32, SCREEN.1 as u32)
            .with_shadows(true)
            .with_hud(true)
            .with_audio_files(vec![
                format!("{ASSET_DIR}/sfx_shoot.wav").into(),
                format!("{ASSET_DIR}/sfx_hit.wav").into(),
                format!("{ASSET_DIR}/sfx_clear.wav").into(),
                format!("{ASSET_DIR}/bgm.wav").into(),
            ])
            .disable_physics()
            .disable_scripts(),
    )?;

    // ---- 입력 바인딩
    app.input_map
        .bind_axis("move_x", KeyCode::KeyD, KeyCode::KeyA)
        .bind_axis("move_y", KeyCode::KeyW, KeyCode::KeyS);

    // ---- 카메라: 플레이어 추적 탑다운 + VP 공유 (조준용, 1프레임 지연 허용)
    {
        let phase = phase.clone();
        let cam_vp = cam_vp.clone();
        app.set_camera_fn(move |cam, world, _dt| {
            let target = match (*phase.borrow(), world.find_by_tag("player")) {
                (GamePhase::Playing, Some(p)) => world.get::<Transform>(p)
                    .map(|t| t.position).unwrap_or(Vec3::new(0.0, 0.0, 0.0)),
                _ => Vec3::new(0.0, 0.0, 0.0),
            };
            cam.position = target + Vec3::new(0.0, 16.0, 9.0);
            cam.yaw = 0.0;
            cam.pitch = -1.05;
            *cam_vp.borrow_mut() = cam.view_projection();
        });
    }

    // ---- 액션: 이동 + 발사 + phase 키
    {
        let phase = phase.clone();
        let map = map.clone();
        let cam_vp = cam_vp.clone();
        let source = source.clone();
        let mut fire_cd = Timer::repeating(0.18);
        app.add_action_system(move |world, input, imap, dt| {
            let ph = *phase.borrow();
            match ph {
                GamePhase::Title => {
                    if input.is_key_just_pressed(KeyCode::Space) {
                        game_state::start_game(world, &mut map.borrow_mut(), &source);
                        spawn_bgm(world);
                        *phase.borrow_mut() = GamePhase::Playing;
                    }
                }
                GamePhase::GameOver | GamePhase::Victory => {
                    if input.is_key_just_pressed(KeyCode::KeyR) {
                        world.clear();
                        *phase.borrow_mut() = GamePhase::Title;
                    }
                }
                GamePhase::Playing => {
                    let m = map.borrow();
                    // 이동 (벽 슬라이드)
                    let mv = imap.vector("move_x", "move_y", input);
                    let dir = Vec3::new(mv.x, 0.0, -mv.y).normalize_or_zero();
                    if let Some(p) = world.find_by_tag("player") {
                        if let Some(t) = world.get_mut::<Transform>(p) {
                            t.position = try_move(&m, t.position, dir * PLAYER_SPEED * dt, 0.45);
                        }
                    }
                    // 발사 (마우스 조준)
                    fire_cd.tick(dt);
                    if input.is_mouse_button_pressed(MouseButton::Left) && fire_cd.just_finished() {
                        let (mx, my) = input.mouse_position();
                        let ndc = ndc_from_mouse(mx, my, SCREEN.0, SCREEN.1);
                        if let Some(inv) = cam_vp.borrow().inverse() {
                            if let (Some(p), Some(aim)) = (
                                world.find_by_tag("player"),
                                unproject_to_plane(&inv, ndc, PLAYER_Y),
                            ) {
                                let from = world.get::<Transform>(p).map(|t| t.position);
                                if let Some(from) = from {
                                    let dir = Vec3::new(aim.x - from.x, 0.0, aim.z - from.z);
                                    spawn_projectile(world, Vec3::new(from.x, 0.6, from.z), dir, Team::Player, 1, 14.0);
                                    play_sfx(world, &format!("{ASSET_DIR}/sfx_shoot.wav"), from);
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    // ---- 게임플레이 시스템 (Playing 동안만)
    {
        let phase = phase.clone();
        let map = map.clone();
        let nav = nav.clone();
        let mut repath = Timer::repeating(0.5);
        app.add_system(move |world, dt| {
            if *phase.borrow() != GamePhase::Playing { return; }
            let due = repath.tick(dt).just_finished();
            let m = map.borrow().clone(); // 문 개방 반영 스냅샷 (맵이 작아 저렴)
            enemy::enemy_ai_system(world, &m, &nav, dt, due);
            voltex::ai::nav_agent_system(world, &nav, dt);
            enemy::velocity_system(world, &m, dt);
            combat::projectile_system(world, &m, dt);
            let out = combat::death_system(world);
            game_state::door_system(world, &mut map.borrow_mut());
            if out.boss_died { *phase.borrow_mut() = GamePhase::Victory; }
            else if out.player_died { *phase.borrow_mut() = GamePhase::GameOver; }
        });
    }

    // ---- 충돌 → 데미지
    {
        let phase = phase.clone();
        app.set_collision_fn(move |contacts, world, _dt| {
            if *phase.borrow() == GamePhase::Playing {
                combat::handle_contacts(contacts, world);
            }
        });
    }

    // ---- HUD
    {
        let phase = phase.clone();
        app.set_hud_fn(move |ui, world| {
            game_state::draw_hud(ui, world, *phase.borrow());
        });
    }

    app.run()
}

fn spawn_bgm(world: &mut World) {
    let mut src = voltex::audio::AudioSource::new(0u32);
    src.looping = true;
    src.volume = 0.35;
    src.playing = true;
    world.spawn_bundle((
        Transform::new(),
        voltex_runtime::AudioPath(format!("{ASSET_DIR}/bgm.wav")),
        src,
        Tag("bgm".into()),
    ));
}
```

`game_state.rs`에 `draw_hud` 추가:
```rust
use voltex::editor::UiContext;

pub fn draw_hud(ui: &mut UiContext, world: &World, phase: GamePhase) {
    let (w, h) = (ui.screen_width, ui.screen_height);
    match phase {
        GamePhase::Title => {
            ui.draw_rect(0.0, 0.0, w, h, [10, 10, 20, 230]);
            ui.draw_text("VOLTEX DEPTHS", w / 2.0 - 120.0, h / 2.0 - 40.0, [240, 220, 120, 255]);
            ui.draw_text("WASD move / mouse aim+fire", w / 2.0 - 140.0, h / 2.0, [200, 200, 200, 255]);
            ui.draw_text("press SPACE to start", w / 2.0 - 110.0, h / 2.0 + 24.0, [120, 230, 140, 255]);
        }
        GamePhase::Playing => {
            let hp = world.find_by_tag("player")
                .and_then(|p| world.get::<Health>(p)).map(|h| h.hp).unwrap_or(0);
            let score = world.resource::<Score>().map(|s| s.0).unwrap_or(0);
            ui.draw_text(&format!("HP {hp}"), 16.0, 16.0, [240, 90, 90, 255]);
            ui.draw_text(&format!("SCORE {score}"), 16.0, 36.0, [240, 220, 120, 255]);
        }
        GamePhase::GameOver => {
            ui.draw_rect(0.0, 0.0, w, h, [30, 5, 5, 200]);
            ui.draw_text("YOU DIED", w / 2.0 - 70.0, h / 2.0 - 12.0, [240, 80, 80, 255]);
            ui.draw_text("press R to restart", w / 2.0 - 100.0, h / 2.0 + 16.0, [200, 200, 200, 255]);
        }
        GamePhase::Victory => {
            ui.draw_rect(0.0, 0.0, w, h, [5, 25, 10, 200]);
            ui.draw_text("DUNGEON CLEARED!", w / 2.0 - 110.0, h / 2.0 - 12.0, [120, 240, 140, 255]);
            ui.draw_text("press R for title", w / 2.0 - 95.0, h / 2.0 + 16.0, [200, 200, 200, 255]);
        }
    }
}
```

`combat.rs`에 `play_sfx` 추가:
```rust
pub fn play_sfx(world: &mut World, path: &str, pos: Vec3) {
    let mut src = voltex::audio::AudioSource::new(0u32);
    src.playing = true;
    src.volume = 0.8;
    world.spawn_bundle((
        Transform::from_position(pos),
        voltex_runtime::AudioPath(path.to_string()),
        src,
        voltex_runtime::Lifetime::new(1.5),
    ));
}
```

import 주의: `MouseButton`/`KeyCode`는 prelude에 없으면 `voltex::platform::{KeyCode, MouseButton}`. `AudioPath`/`Lifetime`/`Timer`는 `voltex_runtime` 경로. `voltex::audio::AudioSource`가 facade에 없으면 `voltex_audio` 직접 dep 추가(Cargo.toml) — friction 기록.

- [ ] **Step 2: 빌드 확인** — Run: `cargo build -p voltex_depths && cargo clippy -p voltex_depths -- -D warnings`. Expected: 성공.
- [ ] **Step 3: Commit** — `git commit -m "feat(depths): full runtime wiring (input/camera/HUD/phase/audio)"`

---

### Task 14: gen_sfx — 절차 생성 WAV 에셋

**Files:**
- Create: `games/voltex_depths/src/bin/gen_sfx.rs`
- Output(커밋 대상): `games/voltex_depths/assets/sfx_shoot.wav`, `sfx_hit.wav`, `sfx_clear.wav`, `bgm.wav`

- [ ] **Step 1: 구현**

```rust
//! 절차 생성 SFX/BGM. `cargo run -p voltex_depths --bin gen_sfx` 로 assets/ 재생성.
use std::f32::consts::TAU;
use std::io::Write;

const RATE: u32 = 44_100;

fn write_wav(path: &str, samples: &[f32]) -> std::io::Result<()> {
    let n = samples.len() as u32 * 2;
    let mut f = std::fs::File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + n).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;            // PCM
    f.write_all(&1u16.to_le_bytes())?;            // mono
    f.write_all(&RATE.to_le_bytes())?;
    f.write_all(&(RATE * 2).to_le_bytes())?;      // byte rate
    f.write_all(&2u16.to_le_bytes())?;            // block align
    f.write_all(&16u16.to_le_bytes())?;           // bits
    f.write_all(b"data")?;
    f.write_all(&n.to_le_bytes())?;
    for s in samples {
        f.write_all(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes())?;
    }
    Ok(())
}

fn tone(freq: f32, secs: f32, decay: f32) -> Vec<f32> {
    (0..(RATE as f32 * secs) as usize).map(|i| {
        let t = i as f32 / RATE as f32;
        (t * freq * TAU).sin() * (-t * decay).exp() * 0.6
    }).collect()
}

fn main() -> std::io::Result<()> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
    write_wav(&format!("{dir}/sfx_shoot.wav"), &tone(880.0, 0.09, 35.0))?;
    write_wav(&format!("{dir}/sfx_hit.wav"), &tone(180.0, 0.15, 18.0))?;
    let mut clear = tone(523.0, 0.12, 8.0);
    clear.extend(tone(659.0, 0.12, 8.0));
    clear.extend(tone(784.0, 0.2, 6.0));
    write_wav(&format!("{dir}/sfx_clear.wav"), &clear)?;
    // BGM: 8초 4음 루프 (낮은 사각파 느낌 = 사인 + 3배음)
    let mut bgm = Vec::new();
    for &f in &[110.0f32, 138.6, 164.8, 138.6] {
        for i in 0..(RATE * 2) as usize {
            let t = i as f32 / RATE as f32;
            bgm.push(((t * f * TAU).sin() + 0.3 * (t * f * 3.0 * TAU).sin()) * 0.18);
        }
    }
    write_wav(&format!("{dir}/bgm.wav"), &bgm)?;
    println!("wrote 4 wav files to {dir}");
    Ok(())
}
```

- [ ] **Step 2: 실행 + 검증** — Run: `cargo run -p voltex_depths --bin gen_sfx`. 그 다음 엔진 파서로 라운드트립 확인:
`cargo test -p voltex_audio --lib` 가 그린인 상태에서, 간단 검증 테스트를 gen_sfx에 두지 말고 게임 테스트에 추가 (`games/voltex_depths/tests/` 또는 level tests 옆):
```rust
#[test]
fn generated_wavs_parse_with_engine_decoder() {
    for name in ["sfx_shoot", "sfx_hit", "sfx_clear", "bgm"] {
        let bytes = std::fs::read(format!("assets/{name}.wav")).expect("run gen_sfx first");
        let clip = voltex::audio::parse_wav(&bytes).expect(name);
        assert!(clip.sample_rate == 44_100 && !clip.samples.is_empty());
    }
}
```
(테스트 working dir는 crate 루트 `games/voltex_depths/` — assets 상대경로 OK)
- [ ] **Step 3: Commit** — `git add games/voltex_depths/assets/*.wav games/voltex_depths/src/bin/gen_sfx.rs && git commit -m "feat(depths): procedural sfx/bgm wav assets + generator"`

---

### Task 15: 헤드리스 통합 시나리오 테스트

**Files:**
- Create: `games/voltex_depths/tests/playthrough.rs`

main.rs 배선과 같은 순서로 시스템을 직접 호출해 "스폰 → 추적 → 사살 → 문 개방 → 보스 → 승리" 한 판을 GPU 없이 검증. (RuntimeApp tick은 GPU 없이 가능하지만 시스템 등록이 클로저라 직접 조합이 더 명확)

- [ ] **Step 1: 테스트 작성** — main.rs의 게임플레이 시스템 블록과 **동일한 호출 순서**:

```rust
use voltex::prelude::*;
use voltex_depths::*; // lib 타깃이 없으므로: 이 테스트를 위해 src/lib.rs 추가 (mod들 pub 재export, main.rs는 lib 사용) — 또는 tests 대신 src/integration_test.rs #[cfg(test)]로. lib.rs 방식 권장:
// src/lib.rs: pub mod combat; pub mod components; pub mod enemy; pub mod game_state; pub mod level;
// src/main.rs: use voltex_depths::{...};

#[test]
fn full_playthrough_kill_unlock_boss_victory() {
    let source = level::LevelMap::parse(&[
        "###########",
        "#P.c#..#B.#",
        "#...D..D..#",
        "###########",
    ]).unwrap();
    let nav = level::build_navmesh(&source);
    let mut map = source.clone();
    let mut w = World::new();
    level::build_level(&mut w, &map);
    let player = w.find_by_tag("player").unwrap();

    let mut victory = false;
    for frame in 0..3600 { // 60초 상한
        let dt = 1.0 / 60.0;
        // 치트: 플레이어가 가장 가까운 적에게 매 프레임 발사 (조준/입력 대체)
        if frame % 12 == 0 {
            let p = w.get::<Transform>(player).unwrap().position;
            if let Some((_, target)) = w.query::<components::Enemy>()
                .map(|(e, _)| (e, w.get::<Transform>(e).unwrap().position))
                .min_by(|a, b| (a.1 - p).length().total_cmp(&(b.1 - p).length()))
            {
                combat::spawn_projectile(&mut w, p, target - p, components::Team::Player, 1, 14.0);
            }
        }
        // 플레이어 전진: 열린 문 방향(+x)으로 걷는다
        if let Some(t) = w.get_mut::<Transform>(player) {
            let pos = t.position;
            t.position = level::try_move(&map, pos, Vec3::new(3.0 * dt, 0.0, 0.0), 0.45);
        }
        let due = frame % 30 == 0;
        enemy::enemy_ai_system(&mut w, &map, &nav, dt, due);
        voltex::ai::nav_agent_system(&mut w, &nav, dt);
        enemy::velocity_system(&mut w, &map, dt);
        combat::projectile_system(&mut w, &map, dt);
        let contacts = voltex::physics::detect_collisions(&w);
        combat::handle_contacts(&contacts, &mut w);
        let out = combat::death_system(&mut w);
        game_state::door_system(&mut w, &mut map);
        voltex_runtime::update_lifetimes(&mut w, dt);
        if out.boss_died { victory = true; break; }
        assert!(!out.player_died, "player must survive this scripted run");
    }
    assert!(victory, "boss must die within 60 simulated seconds");
}
```

- [ ] **Step 2: lib.rs 분리** — `src/lib.rs` 생성 (5 mod pub 선언), `main.rs`는 `use voltex_depths::...`로 전환. Run: `cargo test -p voltex_depths --test playthrough`. Expected: PASS (실패하면 이게 가장 가치 있는 통합 버그 — systematic-debugging).
- [ ] **Step 3: 전체 확인** — Run: `cargo test -p voltex_depths && cargo clippy -p voltex_depths --all-targets -- -D warnings`.
- [ ] **Step 4: Commit** — `git commit -m "test(depths): headless full-playthrough integration"`

---

### Task 16: 시각 회귀 — voltex_capture render_world + 던전 캡처

**Files:**
- Modify: `crates/voltex_capture/src/scene.rs` (pub `render_world` 추가 — 기존 `render_declarative_colored_scene` 본문에서 월드 구성만 분리하는 기계적 추출)
- Create: `games/voltex_depths/tests/visual.rs`
- Modify: `games/voltex_depths/Cargo.toml` (`[dev-dependencies] voltex_capture = { path = "../../crates/voltex_capture" }`)

- [ ] **Step 1: render_world 추출** — `crates/voltex_capture/src/scene.rs`의 `render_declarative_colored_scene`(이 함수가 PrimitiveMesh/MaterialDesc world를 materialize→BasicRenderer로 렌더) 본문을 다음 시그니처로 분리, 기존 함수는 위임:
```rust
/// 외부(게임 테스트)에서 임의 World를 오프스크린 렌더 — 도그푸딩 마찰 해소로 공개.
pub fn render_world(
    gpu: &HeadlessGpu, width: u32, height: u32,
    world: &mut voltex_ecs::World,
    cam_pos: voltex_math::Vec3, cam_yaw: f32, cam_pitch: f32,
) -> Vec<u8>
```
Run: `cargo test -p voltex_capture --lib` (기존 캡처 단위 그린 유지). friction 로그에 "게임이 자기 씬을 픽셀 검증할 공개 진입점 부재 → render_world 추가" 기록.

- [ ] **Step 2: 게임 시각 테스트** (`games/voltex_depths/tests/visual.rs`):
```rust
use voltex::prelude::*;
use voltex_capture::{HeadlessGpu, ImageStats};
use voltex_depths::{components::*, level::*};

#[test]
#[ignore] // GPU 필요 — cargo test -p voltex_depths --test visual -- --ignored
fn dungeon_scene_renders_walls_player_enemies() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => { eprintln!("skipping: {e}"); return; }
    };
    let m = LevelMap::parse(&[
        "#########",
        "#...#...#",
        "#.P.D.c.#",
        "#...#...#",
        "#########",
    ]).unwrap();
    let mut w = World::new();
    build_level(&mut w, &m);
    let rgba = voltex_capture::render_world(
        &gpu, 320, 240, &mut w,
        Vec3::new(0.0, 16.0, 9.0), 0.0, -1.05,
    );
    let stats = ImageStats::analyze(&rgba);
    assert!(!stats.looks_blank(), "dungeon frame blank: {stats:?}");
    let (mut green, mut red) = (false, false);
    for p in rgba.chunks_exact(4) {
        let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
        if g > r + 30 && g > b + 30 { green = true; }
        if r > g + 30 && r > b + 30 { red = true; }
    }
    assert!(green && red, "player(green)/enemy(red) must be visible");
}
```
Run: `cargo test -p voltex_depths --test visual -- --ignored`. Expected: PASS (RTX 4080). PNG 산출물 1장을 `docs/visual/depths_dungeon.png`로 저장하는 줄도 추가 (`voltex_capture::encode_rgba8` + `std::fs::write` — 기존 캡처 테스트들이 쓰는 패턴 그대로).
- [ ] **Step 3: Commit** — `git commit -m "feat(capture): public render_world + depths dungeon visual regression"`

---

### Task 17: 스모크 런 + voltex build E2E + 문서 마감

- [ ] **Step 1: 스모크 런** — `cargo run -p voltex_depths` 백그라운드 8초 → 종료, stderr에 panic/wgpu validation error 0 확인. 타이틀 화면이 뜨는지는 수동 영역(사용자 배치 패스)으로 명시.
- [ ] **Step 2: 릴리스 빌드 E2E** — `cargo run -p voltex_cli -- build` (games/voltex_depths에서, voltex CLI 사용법은 crates/voltex_cli README/STATUS v1.0.55 참조) 또는 실패 시 `cargo build --release -p voltex_depths`로 대체하고 manifest 마찰 기록. 산출 exe 실행 확인(8초 스모크).
- [ ] **Step 3: README** — `games/voltex_depths/README.md`: 조작법, 실행/빌드 명령, gen_sfx 재생성, 스크린샷(`docs/visual/depths_dungeon.png`) 링크, known limitations (창 크기 고정 조준, navmesh가 닫힌 문을 워크블 취급 — region 활성화로 보완).
- [ ] **Step 4: STATUS.md / CHANGELOG.md 갱신** — 기존 v1.0.x 항목 포맷으로 요약 (게임 추가 + render_world + friction 로그 개시).
- [ ] **Step 5: friction 로그 정리** — `docs/dogfood-friction.md`에 누적된 항목들을 P0~P2로 정렬하고 머리말에 "다음 엔진 사이클 후보" 요약 추가.
- [ ] **Step 6: 최종 검증** — `cargo build --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --lib`. Expected: 전부 그린.
- [ ] **Step 7: Commit** — `git commit -m "docs(depths): README + STATUS/CHANGELOG + friction log wrap-up"`

---

## 작업 전반 규칙

- 매 태스크: 해당 crate 빌드 → 워크스페이스 빌드 → 테스트 → 커밋 (CLAUDE.md 변경 워크플로)
- API가 플랜과 다르면 (필드명/시그니처): 코드가 정답 — 해당 crate 소스 확인 후 맞추고, 플랜과의 차이가 "어색함"이면 friction 기록
- 엔진 버그 발견 시: superpowers:systematic-debugging → 엔진 쪽 회귀 테스트와 함께 수정 → 별도 커밋 (`fix(<crate>): ...`)
- 600줄 한도: level.rs가 커지면 `level/mod.rs` + `level/regions.rs`로 분리
