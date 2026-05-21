# Voltex Survivor — flagship game (v1.0)

> Survivor-style mini game. WASD 이동 + 자동 발사 + wave 시스템. 게임
> 로직은 100% Lua + scene 으로 표현, Rust 측은 voltex_runtime 부트스트랩
> + 입력 → Transform 의 ~80 라인.

이 디렉토리는 v1.0 로드맵 Cycle 15 (D3) 의 산출물이자 voltex 가 실
게임을 굴릴 수 있다는 증거. 동시에 외부 사용자가 자기 게임을 만들 때
참고할 수 있는 reference layout 이다.

## 실행

```bash
# 개발자 모드 (워크스페이스에서)
cargo run -p voltex_survivor --release

# 배포 (정식 build pipeline 으로 self-contained exe 산출)
cd games/voltex_survivor
voltex build --engine-path ../..
./dist/voltex_survivor.exe       # Windows
./dist/voltex_survivor           # Linux/macOS
```

조작:
- **WASD** — 이동
- **자동** — 가장 가까운 적 향해 0.8초 주기 발사
- ESC / 창 닫기 — 종료

## 디렉토리 구조

| 파일/디렉토리 | 역할 |
|---|---|
| `voltex.toml` | 매니페스트 (game name / scene / scripts / assets / window) |
| `scene.vscn` | 시드 엔티티 (ground + 플레이어 + 3 cube) |
| `scripts/game.lua` | 3초 첫 wave, 5초 간격 4적 spawn. score/lives/wave 추적. `kill_marker` / `damage_marker` entity 메일박스로 이벤트 수확 |
| `scripts/player.lua` | 가장 가까운 적 향해 발사. 방향은 spawned projectile 의 rotation.y 에 인코딩 |
| `scripts/enemy.lua` | 1.2 unit/sec 로 플레이어 추적, 충돌 거리 ≤ 0.55 시 `damage_marker` 드랍 + self-destruct |
| `scripts/projectile.lua` | rotation.y 방향 6 unit/sec, enemy 충돌 시 `kill_marker` 드랍 + 양쪽 despawn, 3초 lifetime |
| `assets/` | 메쉬/머티리얼 (현재 빈 디렉토리 — 모든 메쉬는 RuntimeApp 의 with_gpu_init 에서 cube 생성) |
| `src/main.rs` | RuntimeApp 부트스트랩 + input system + mesh upload (~80 라인) |

## 학습 포인트

- **Lua 가 게임 로직의 source of truth**: Rust 는 boilerplate 만, 새 적 유형 / 새 무기 / wave 패턴 변경은 모두 Lua 수정 + 핫 리로드.
- **Entity 메일박스 패턴**: `kill_marker` / `damage_marker` 같은 empty entity 가 다른 스크립트가 수확하는 이벤트 큐 역할. Lua 측 입력 API 부재 (v1.0 한계) 우회.
- **handle-attach 시스템**: Lua 가 `spawn_entity()` 한 entity 에 main.rs 의 시스템이 Tag 기반으로 ScriptComponent + mesh/material handle 자동 부착. 신규 entity 가 즉시 렌더링 + 스크립트 실행됨.

## 알려진 한계 (v1.1+)

- Lua 측 입력 API 부재 → 발사/이동은 모두 Rust 가 처리, 게임 로직만 Lua. v1.1 의 `input.is_key_pressed` 도입 후 100% Lua 게임 가능.
- HUD UI 부재 → score/wave/lives 는 콘솔 stdout 출력만. `voltex_editor` UI 의 runtime 노출 (v1.1) 후 in-game HUD 가능.

## 검증

`cargo run -p voltex_survivor --release` Windows 호스트 기준 30초 안정
(6 wave / 24 enemy kill / 0 panic) — v1.0.0 release 검증. macOS / Linux
실 머신 검증은 환영.

스펙: `docs/superpowers/specs/2026-05-21-d3-flagship-game-design.md`
