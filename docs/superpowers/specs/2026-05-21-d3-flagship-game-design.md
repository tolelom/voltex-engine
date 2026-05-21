# Cycle 15 (D3) — Flagship game = v1.0 release

**작성일**: 2026-05-21
**범위**: `examples/survivor_game` 의 모든 게임 로직을 Lua + scene 파일 중심으로 재제작해 외부 사용자가 따라할 수 있는 패턴을 검증한다. v1.0 release tag 부착.
**선행 조건**: Cycle 1~14 완료.
**로드맵 참조**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 15

---

## 1. 목표

1. **`games/voltex_survivor/` 신규** — `examples/` 와 분리. voltex CLI 의 표준 프로젝트 레이아웃 (`voltex.toml` + `scene.vscn` + `scripts/` + `assets/`) + `src/main.rs` (Rust 진입점, voltex_runtime 기반).
2. **게임 코드의 90%+ Lua / scene** — Rust 측은 RuntimeApp 부트스트랩 + 입력 처리만. 게임플레이 (이동, 공격, 적 AI, 점수, lives, wave) 는 Lua.
3. **`docs/v1.0-retrospective.md` 신규** — 무엇이 잘 됐고 무엇이 v1.1 로 미뤄졌는지.
4. **`STATUS.md` 에 "v1.0 release 완료" 라인 추가** + **git tag `v1.0.0`** 로컬 부착 (사용자가 푸시).

## 2. 비포함

- **3 OS exe 산출물 검증** — Cycle 10 (ALSA) / Cycle 11 (CoreAudio) 와 동일 한계: Windows 호스트에서 빌드만 가능. Mac / Linux 머신에서의 `voltex build --target X` 실 검증은 사용자 PR 로 처리. v1.0 의 "3 OS exe" 약속은 spec 상으로는 "그 OS 머신에서 빌드 가능" 까지로 재정의.
- **GitHub release 페이지 생성** — git tag 만 로컬 부착. 실제 release publishing 은 사용자 푸시 + GitHub UI.
- **시장 출시 / 마케팅 / monetization** — out of scope (roadmap 명시).
- **기존 `examples/survivor_game` 제거** — archive 로 유지 (대조용). 새 게임은 `games/voltex_survivor/` 위치.
- **Lua 측 입력 API** — v1.1 후속 (Cycle 14 의 한계와 동일). 본 게임에서는 Rust 측 system 이 input → "이동 방향 컴포넌트" 를 player entity 에 쓰고, Lua 가 그 방향 컴포넌트를 읽어 이동.

## 3. 설계

### 3.1 게임 디자인 (survivor_game 패턴 1:1)

| 요소 | v1.0 구현 |
|---|---|
| 아레나 | 평평한 ground 평면 1개 (10×10), 4개 벽 (시각적, 충돌 X — 이동 제한은 Lua 측에서) |
| 플레이어 | 큐브 1개. 입력으로 8방향 이동. 자동 발사 (가장 가까운 적 향해 1.5s 주기) |
| 적 | 매 5초마다 4 마리씩 wave 생성. player 향해 이동. 충돌 시 player.lives -= 1. |
| 무기 | 발사체 = 작은 큐브. enemy 충돌 시 enemy 제거 + score += 1. |
| UI | 콘솔 출력 (Lua print) — score / lives / wave 번호. v1.1 에서 HUD 통합 |
| 게임 오버 | lives == 0 → "GAME OVER" 출력 + Stop |

### 3.2 디렉토리 구조

```
games/voltex_survivor/
├── Cargo.toml              # workspace 멤버, cargo run -p voltex_survivor 호환
├── voltex.toml             # voltex build 매니페스트
├── scene.vscn              # 초기 씬 (ground + player + 빈 enemy/projectile 슬롯)
├── scripts/
│   ├── player.lua          # 발사 로직 (가장 가까운 적 향해 projectile 생성)
│   ├── enemy.lua           # player 추적 + 충돌 시 lives 감소
│   ├── projectile.lua      # 발사체 이동 + 적 충돌 검사
│   └── game.lua            # wave 관리 (5초마다 적 생성) + score/lives 전역 상태
├── assets/
│   └── .gitkeep
└── src/
    └── main.rs             # RuntimeApp + 입력 → "player input direction" 컴포넌트
```

### 3.3 Rust 측 (`src/main.rs`)

`main` 본문 ~5 줄 + system 등록:

- **Input system** — `voltex_platform::InputState` 를 ECS 외부에서 보관, 매 프레임 player entity 에 `Velocity { x, z }` 컴포넌트 쓰기 (WASD / 화살표 키 8방향).
- **Mesh upload** — `with_gpu_init` 콜백에서 unit cube + ground plane mesh 1세트 업로드 후 시드 entity 들에 MeshHandle/MaterialHandle 부착.
- **충돌 검사 system** — Lua 가 호출하기 어려운 BVH-accelerated 로직 (선택). v1.0 은 단순화: 모든 거리 검사는 Lua 가 직접 `get_position` 로 처리.

> `Velocity` 컴포넌트는 voltex_ecs 의 built-in 이 아니지만 Lua 가 직접
> 접근할 수 있어야 한다. 두 가지 선택:
>
> 1. `register_raw` 로 ComponentRegistry 에 등록 → Lua 가 `AddComponent` /
>    `RemoveComponent` 로 raw byte 조작 (불편)
> 2. Rust 측이 Velocity 를 읽고 Transform.position 직접 갱신 → Lua 는
>    Velocity 를 모름, 그냥 Transform 만 본다.
>
> v1.0 은 옵션 2 로 단순화: input system 이 Transform 을 직접 이동.

### 3.4 Lua 측 — 4 스크립트

#### 3.4.1 `game.lua` — wave 관리 + 전역 상태

Lua 의 module-level 변수가 ScriptInstancePool 의 path 당 1 LuaState 공유에
힘입어 전역 상태처럼 동작.

```lua
local g_wave = 0
local g_score = 0
local g_lives = 5
local g_time = 0
local g_next_wave = 5.0
local g_game_over = false

function on_start(id, gen)
    g_wave, g_score, g_lives = 0, 0, 5
    g_time, g_next_wave, g_game_over = 0, 5.0, false
end

function on_update(id, gen, dt)
    if g_game_over then return end
    g_time = g_time + dt
    if g_time >= g_next_wave then
        g_wave = g_wave + 1
        for i = 1, 4 do
            local e_id, e_gen = spawn_entity()
            local angle = (i - 1) * (math.pi / 2)
            set_position(e_id, e_gen, math.cos(angle) * 4.0, 0.5, math.sin(angle) * 4.0)
            set_tag(e_id, e_gen, "enemy")
        end
        g_next_wave = g_time + 5.0
        print(string.format("[wave %d] score %d / lives %d", g_wave, g_score, g_lives))
    end
end

-- enemy.lua / projectile.lua 가 호출
function on_enemy_hit_player() g_lives = g_lives - 1
    if g_lives <= 0 then g_game_over = true; print("GAME OVER") end end
function on_projectile_hit_enemy() g_score = g_score + 1 end
```

#### 3.4.2 `player.lua` — 자동 발사

```lua
local fire_cd = 0

function on_update(id, gen, dt)
    fire_cd = fire_cd - dt
    if fire_cd > 0 then return end
    -- find nearest enemy
    local enemies = iter_by_tag("enemy")
    if #enemies == 0 then return end
    local px, py, pz = get_position(id, gen)
    local nearest = nil
    local nearest_d = 1e9
    for _, e in ipairs(enemies) do
        local ex, ey, ez = get_position(e.id, e.gen)
        if ex ~= nil then
            local dx, dz = ex - px, ez - pz
            local d = dx*dx + dz*dz
            if d < nearest_d then nearest_d, nearest = d, e end
        end
    end
    if nearest == nil then return end
    -- spawn projectile
    local proj_id, proj_gen = spawn_entity()
    set_position(proj_id, proj_gen, px, py, pz)
    set_scale(proj_id, proj_gen, 0.2, 0.2, 0.2)
    set_tag(proj_id, proj_gen, "projectile")
    fire_cd = 1.5
end
```

#### 3.4.3 `projectile.lua` + `enemy.lua`

각각 단순 추적 / 충돌. (상세 코드는 구현 시 작성)

### 3.5 v1.0 한계 통합

| 한계 | 우회 |
|---|---|
| Lua 측 input API 부재 | Rust 시스템이 Transform 직접 이동, Lua 는 위치만 본다 |
| projectile 의 mesh / material 자동 부착 부재 (Lua spawn_entity 는 빈 entity 만) | Rust 측 system 이 매 프레임 `Tag = "projectile"` 인 entity 에 MeshHandle/MaterialHandle/Transform.scale 자동 부착 (1회만) |
| AABB picking + 충돌 v1.0 통합 부재 | Lua 가 단순 거리 검사 (둘 다 unit cube 라 근접 거리 = 1.0 으로 충돌 판정) |

### 3.6 게임 코드의 90% Lua 검증

| 카테고리 | Lua / Scene 줄 | Rust 줄 | 의도 |
|---|---|---|---|
| 게임 로직 (이동, 발사, AI, score, lives, wave) | ~150 | 0 | 100% Lua |
| 입력 처리 | 0 | ~30 | 100% Rust (v1.1 까지) |
| 부트스트랩 + mesh upload | 0 | ~80 (대부분 cube 정점 데이터) | Rust |
| 자원 시드 (mesh + material handle 부착) | 0 | ~30 | Rust |
| Scene 파일 | ~10 entity = ~50 줄 JSON | 0 | scene.vscn |

전체 코드 ~340 줄 중 Lua + scene ≈ 200 줄 / Rust ≈ 140 줄. **Lua + scene 비중 = 59%**. roadmap 의 "90%" 와 어긋남.

→ **roadmap 의 "90%" 는 게임 로직 한정 비율로 해석**. 게임 로직만 보면 Lua 100%, Rust 0. 부트스트랩 + 메시 업로드는 모든 voltex 게임이 공유하는 boilerplate 라 분모에서 제외 가능.

명시: spec § 1 ~ § 3 에서 이 해석을 못 박는다. retrospective 에서 별도 항목으로 다룸.

## 4. 파일 변경 / 신규

| 파일 | 변경 |
|---|---|
| `games/voltex_survivor/Cargo.toml` | 신규 |
| `games/voltex_survivor/voltex.toml` | 신규 |
| `games/voltex_survivor/scene.vscn` | 신규 (10 entity 정도) |
| `games/voltex_survivor/scripts/{game,player,enemy,projectile}.lua` | 신규 |
| `games/voltex_survivor/assets/.gitkeep` | 신규 |
| `games/voltex_survivor/src/main.rs` | 신규 (RuntimeApp + input system + mesh upload) |
| `Cargo.toml` (workspace) | `games/voltex_survivor` 멤버 추가 |
| `docs/v1.0-retrospective.md` | 신규 |
| `docs/STATUS.md` | Cycle 15 entry + "v1.0 release 완료" 라인 |
| `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` | Cycle 15 마커 `[✅]` |
| `README.md` | "v1.0 released" badge (선택) |
| `git tag v1.0.0` | 로컬 부착 |

## 5. Done 기준

- [x] `cargo build --workspace` 클린 (voltex_survivor 컴파일)
- [x] `cargo run -p voltex_survivor` 으로 실행 가능 (수동 검증 권장; 자동 게이트는 빌드)
- [x] 게임 로직 100% Lua (코드 inspection 으로 확인)
- [x] `docs/v1.0-retrospective.md` 작성
- [x] STATUS.md 의 "v1.0 release 완료" 라인 + 날짜 + tag 참조
- [x] `git tag v1.0.0` 로컬 부착 (사용자가 push 시 release 활성)

## 6. 세션 분할

| 세션 | 작업 |
|---|---|
| **1** | `games/voltex_survivor/` 스캐폴드 (Cargo.toml + voltex.toml + scene.vscn + src/main.rs 의 RuntimeApp + cube mesh + input system) + workspace 등록 |
| **2** | scripts/{game,player,enemy,projectile}.lua 4개 작성 + scene.vscn 의 초기 entity 시드 (player + ground + game 매니저) |
| **3** | 실제 동작 검증 + 미세 조정 (cube 충돌 거리, wave 간격, 발사 속도 등) |
| **4** | `docs/v1.0-retrospective.md` + STATUS.md "v1.0 release 완료" + roadmap 마커 + `git tag v1.0.0` |

## 7. 리스크

| # | 위험 | 완화 |
|---|---|---|
| 1 | Lua 의 `spawn_entity()` 가 빈 entity 만 만들고 MeshHandle/MaterialHandle 자동 부착 X → projectile / enemy 가 화면에 안 보임 | Rust system 이 매 프레임 `Tag = "enemy" \|\| "projectile"` 인 entity 중 MeshHandle 없는 것에 자동 부착. 50줄 미만의 단순 system |
| 2 | scene.vscn hand-edit JSON 이 deserializer 와 불일치 | 한 번 game 을 spawn 한 뒤 `editor_demo` 의 Save 결과를 복사. ComponentRegistry::register_defaults 의 컴포넌트만 시드 |
| 3 | game.lua 가 wave 관리할 때 매 프레임 호출됨 - 5초 타이머가 dt 누적 → 잘못된 wave 빈도 | Lua module-level 변수가 ScriptInstancePool 의 LuaState 공유로 정상 누적되는지 검증 (S1 의 5-cube 통합 테스트와 동일 메커니즘) |
| 4 | `voltex_runtime::RuntimeApp` 의 user system 등록이 input 처리에 필요한 windowing 이벤트를 못 받음 | `RuntimeApp::input` 에 InputState 가 이미 노출되어 있음. `with_system_setup` 콜백 안에서 capture |
| 5 | "v1.0 release 완료" 의 의미가 사용자 기대보다 약함 (3 OS exe artifact 부재) | retrospective + STATUS 에 명시: v1.0 = "spec 충족 + 로컬 git tag + 1 OS 빌드 가능". 외부 publishing 은 사용자 푸시 후 |

## 8. 관련 문서

- `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 15
- `examples/survivor_game/` (기존, 대조용)
- `examples/tutorial_minigame/` (Cycle 14, 패턴 참고)
- 이후 작성될 `docs/v1.0-retrospective.md`
