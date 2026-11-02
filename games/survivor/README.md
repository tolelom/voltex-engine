# Voltex Survivor

> 서바이벌 미니 게임. WASD로 움직이면 가장 가까운 적에게 자동으로 쏘고, 적은 웨이브로 몰려온다. 게임 규칙은 Lua 스크립트와 씬 파일에 있고, Rust는 앱 시작, 입력, 메시 업로드만 맡는다.

## 실행

```bash
# 워크스페이스에서
cargo run -p survivor --release

# 실행 파일 하나로 빌드
cd games/survivor
voltex build --engine-path ../..
./dist/survivor.exe       # Windows
./dist/survivor           # Linux·macOS
```

조작: WASD 또는 방향키로 이동, 발사는 자동, ESC로 종료. 점수, 웨이브, 남은 목숨은 콘솔에 찍힌다.

## 구성

| 파일 | 역할 |
|---|---|
| `voltex.toml` | 매니페스트 |
| `scene.vscn` | 시작 엔티티(바닥, 플레이어, 큐브) |
| `scripts/game.lua` | 웨이브 생성, 점수·목숨·웨이브 관리 |
| `scripts/player.lua` | 가장 가까운 적을 향해 발사 |
| `scripts/enemy.lua` | 플레이어 추적, 닿으면 피해를 주고 사라짐 |
| `scripts/projectile.lua` | 직진, 적과 닿으면 둘 다 제거, 일정 시간 뒤 소멸 |
| `src/main.rs` | `RuntimeApp` 시작, 이동 입력(action map), 메시 업로드 |

## 구현 방식

- 게임 규칙은 Lua에만 있다. 적 종류, 무기, 웨이브를 바꿀 때 Rust를 건드리지 않고, 스크립트는 핫 리로드된다.
- 스크립트끼리는 빈 엔티티를 이벤트로 쓴다. 적을 맞히면 `kill_marker`, 플레이어가 맞으면 `damage_marker` 엔티티를 만들고, `game.lua`가 이를 거둬 점수와 목숨에 반영한다.
- Lua가 `spawn_entity()`로 만든 엔티티에는 `main.rs`의 시스템이 태그를 보고 스크립트와 메시·재질 핸들을 붙인다. 그래서 새 엔티티가 바로 그려지고 움직인다.

