# Voltex Survivor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 쿼터뷰 3D 서바이벌 게임 — 적 웨이브를 처치하며 생존, Voltex 엔진 전체 기능 활용

**Architecture:** `examples/survivor_game/` 단일 바이너리. ECS World에 플레이어/적/투사체 엔티티. 포워드 PBR 렌더링 + 섀도우. AI 내비메시 + A* + 스티어링. 물리 충돌 감지. 오디오 SFX. IMGUI HUD.

**Tech Stack:** voltex_math, voltex_platform, voltex_renderer, voltex_ecs, voltex_physics, voltex_ai, voltex_audio, voltex_editor

**Spec:** `docs/superpowers/specs/2026-03-25-survivor-game.md`

---

## File Structure

```
examples/survivor_game/
├── Cargo.toml
└── src/
    ├── main.rs          — 진입점, 윈도우, 이벤트 루프
    ├── game.rs          — GameState, ECS World, 게임 틱 로직
    ├── player.rs        — Player 컴포넌트, 이동, 발사
    ├── enemy.rs         — Enemy 컴포넌트, 스폰, AI 추격
    ├── projectile.rs    — Projectile 컴포넌트, 이동, 충돌
    ├── arena.rs         — 아레나 바닥 + 장애물 + NavMesh
    ├── camera.rs        — 쿼터뷰 카메라
    ├── wave.rs          — 웨이브 시스템
    ├── hud.rs           — IMGUI HUD
    └── render.rs        — 렌더 설정 (파이프라인, 메시, 머티리얼)
```

---

## Task 1: 프로젝트 셸 + 아레나 + 카메라

아레나 바닥과 장애물이 렌더링되고 쿼터뷰 카메라로 볼 수 있는 최소 동작.

**Files:** Cargo.toml, main.rs, arena.rs, camera.rs, render.rs

이 태스크는 pbr_demo/shadow_demo 패턴을 따라 윈도우+GPU+PBR 파이프라인을 설정하고, 평평한 바닥 메시 + 박스 장애물을 렌더링합니다.

- 아레나: 바닥 (20x20 flat box, y=0), 장애물 4개 (random-ish 위치의 박스)
- 카메라: 고정 쿼터뷰 (offset 0,15,10 from center, look_at center)
- 라이트: directional light (태양)
- 렌더: 포워드 PBR, 섀도우 맵

Commit: `feat(game): add survivor_game with arena and quarter-view camera`

---

## Task 2: 플레이어 이동

ECS에 플레이어 엔티티 추가. WASD로 XZ 평면 이동. 카메라가 플레이어를 추적.

**Files:** game.rs, player.rs, camera.rs 수정

- Player 컴포넌트: hp, fire_cooldown, invincible_timer
- Transform: 플레이어 위치
- Collider: Sphere { radius: 0.5 }
- 입력: WASD → velocity, 매 프레임 position += velocity * dt
- 카메라: player position + offset으로 갱신
- 아레나 경계 체크 (position clamp)

Commit: `feat(game): add player movement with WASD and camera follow`

---

## Task 3: 투사체 발사

마우스 방향으로 투사체 발사. XZ 평면에서 마우스 위치 계산.

**Files:** projectile.rs, player.rs 수정, game.rs 수정

- 마우스 → 레이캐스트로 Y=0 평면 교차점 계산 (간단한 ray-plane)
- 조준 방향: (교차점 - 플레이어 위치).normalize() on XZ
- 좌클릭: fire_cooldown <= 0이면 투사체 스폰
- Projectile 컴포넌트: velocity (Vec3), lifetime (2.0s)
- 매 프레임: position += velocity * dt, lifetime -= dt, lifetime <= 0이면 despawn
- 시각: 작은 노란 구체

Commit: `feat(game): add projectile shooting toward mouse aim`

---

## Task 4: 적 스폰 + AI 추격

내비메시 기반 AI로 적이 플레이어를 추격.

**Files:** enemy.rs, arena.rs 수정 (NavMesh 생성), wave.rs, game.rs 수정

- NavMesh: 아레나 바닥을 삼각형으로 분할 (장애물 영역 제외, 단순 수동 정의)
- Enemy 컴포넌트: speed
- 스폰: 아레나 가장자리 랜덤 위치
- AI 루프:
  1. find_path(navmesh, enemy_pos, player_pos)
  2. follow_path + seek 스티어링
  3. position += steering * speed * dt
- Wave: 3 enemies → 5 → 8 → ... (웨이브 간 5초)
- 시각: 빨간 구체

Commit: `feat(game): add enemy spawning with NavMesh A* and steering AI`

---

## Task 5: 충돌 + 데미지 + 점수

투사체↔적, 적↔플레이어 충돌 처리.

**Files:** game.rs 수정, player.rs 수정

- detect_collisions(world) 사용
- 투사체↔적: 둘 다 despawn, score += 100
- 적↔플레이어: 적 despawn, player HP -= 1 (invincible 1초)
- HP <= 0: game_over = true

Commit: `feat(game): add collision detection, damage, and scoring`

---

## Task 6: HUD + 게임 오버 + 오디오

IMGUI HUD + 오디오 SFX + 게임 오버/재시작.

**Files:** hud.rs, game.rs 수정, main.rs 수정

- HUD: voltex_editor IMGUI
  - 좌상단: "HP: 3/3", "Wave: 1", "Score: 0"
  - 게임 오버: 큰 텍스트 "GAME OVER" + "Score: XXXX" + "Press R to restart"
- 오디오: AudioSystem
  - 발사 SFX: 짧은 사인파 (generate procedurally, 440Hz 0.05s)
  - 적 처치 SFX: 220Hz 0.1s
  - 발사/처치 시 play()
- R키: game_over 시 리셋 (World 초기화)

Commit: `feat(game): add HUD, audio SFX, and game over/restart`

---

## Task Summary

| Task | 내용 | 핵심 엔진 |
|------|------|-----------|
| 1 | 아레나 + 카메라 + 렌더 | renderer, platform, math |
| 2 | 플레이어 이동 | ecs, platform (input) |
| 3 | 투사체 발사 | physics (ray-plane), ecs |
| 4 | 적 AI + 웨이브 | ai (navmesh, A*, steering), ecs |
| 5 | 충돌 + 데미지 | physics (detect_collisions) |
| 6 | HUD + 오디오 + 게임오버 | editor (IMGUI), audio |

각 태스크는 이전 태스크에 의존하므로 순차 실행. Task 1이 가장 큰 설정 작업 (GPU, 파이프라인).
