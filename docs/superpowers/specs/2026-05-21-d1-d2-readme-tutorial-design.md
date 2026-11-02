# Cycle 14 (D1 + D2) — README + 5분 quickstart + 30분 tutorial

**작성일**: 2026-05-21
**범위**: 외부 사용자가 fresh clone 후 5분 안에 첫 화면을 보고, 30분 안에 미니 게임을 빌드할 수 있게 한다.
**선행 조건**: Cycle 1~13 완료 (공개 API + facade prelude 안정).
**로드맵 참조**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 14

---

## 1. 목표

1. `README.md` 신규 — 5분 quickstart + feature 요약 + 두 가지 워크플로 (code-first / editor-first) 안내.
2. `docs/tutorial.md` 신규 — 30분 안에 빈 voltex 프로젝트 → editor 에서 미니 게임 → Lua 스크립트로 입력 처리 → Save → Build → 실행. 5~7 chapter.
3. `examples/tutorial_minigame/` 신규 — 튜토리얼의 최종 산출물. 사용자가 막힐 때 비교용. voltex CLI 프로젝트 레이아웃 (`voltex.toml` + `scene.vscn` + `scripts/*.lua` + `assets/`) 으로 구성.

## 2. 비포함

- API reference — `cargo doc --open` 이 담당 (Cycle 13 에서 워닝 0 + tier-1 docstring 정비 완료).
- 비디오 / GIF 튜토리얼.
- 다국어 번역.
- **스크린샷** — Cycle 11/12 와 동일 한계: 현재 작업 환경은 GUI 자동화 부재. 튜토리얼은 텍스트 + ASCII 다이어그램만으로 자기 완결되도록 설계. 스크린샷 자리는 placeholder (`<screenshot: ...>` 마커) 로 두고 사용자가 실제 머신에서 채울 수 있게 한다.

## 3. 설계

### 3.1 README.md 구조

```
# Voltex Engine

> 한 문장 소개: Rust 로 작성한 winit + wgpu 기반 3D 게임 엔진.
> 외부 crate 의존 2개 (winit / wgpu) 만으로 PBR, 물리, 오디오, AI, 에디터,
> Lua 스크립팅, 멀티 OS 빌드 파이프라인 까지 제공한다.

[배지: Rust stable, License MIT/Apache-2.0, v1.0]

## 5분 quickstart
1. `git clone https://git.tolelom.xyz/tolelom/game_engine`
2. `cd game_engine && cargo run --example triangle`
3. 윈도우에서 색깔 삼각형 확인.

## 두 가지 워크플로
- **Code-first**: `use voltex::prelude::*;` — Bevy/Macroquad 스타일.
- **Editor-first**: `voltex new mygame; cd mygame; voltex build; ./dist/mygame.exe`.

## Features (한 줄씩)
- ECS + 씬 그래프 + 자체 직렬화 (.vscn JSON/binary)
- PBR (Cook-Torrance) + CSM/Point/Spot shadow + IBL (SH)
- 디퍼드 + SSGI + RT shadows/reflections/AO + post (TAA/Bloom/DOF)
- 자체 OBJ/glTF/PNG/JPG 파서 (외부 deps 0)
- GJK/EPA 물리 + BVH + sleep + CCD + raycast
- 자체 WAV/Vorbis 디코더 + WASAPI/ALSA/CoreAudio backend
- NavMesh + A* + Funnel + steering
- UDP + reliable/ordered + 스냅샷+delta+보간
- Lua 5.4 (자체 FFI) + per-entity ScriptComponent + 핫리로드
- Immediate-mode editor (도킹, 인스펙터, gizmo, viewport picking)
- `voltex_cli` 멀티 OS 빌드 (.vpkg 자원 임베드)

## 의존성 정책
런타임 crate 2개: `winit` 0.30, `wgpu` 28. 그 외 모든 것은 자체 구현.

## 문서
- [Tutorial (30 min)](docs/tutorial.md) — 0 에서 빌드된 게임까지.
- [Build Matrix](docs/build-matrix.md) — 3 OS 빌드 명령.
- [CHANGELOG](CHANGELOG.md)
- [STATUS](docs/dev/STATUS.md) — 작업 현황 + cycle history.

## License
MIT OR Apache-2.0 (사용자 선택). `THIRD_PARTY_LICENSES` 에 winit + wgpu 라이선스 고지.

## Contributing
한 줄: PR 환영. spec → plan → impl 흐름은 `docs/superpowers/` 참고.
```

### 3.2 tutorial.md 구조 (Chapter 1~7)

| Ch | 제목 | 분량 | 내용 |
|---|---|---|---|
| 1 | 프로젝트 만들기 | 3 min | `voltex new minigame` → 폴더 구조 설명 (voltex.toml / scene.vscn / scripts/ / assets/) |
| 2 | 에디터 실행 + 첫 씬 | 5 min | `voltex_cli` 의 editor mode (또는 examples/editor_demo 로 대체) — Spawn → Inspector slider 로 위치 변경 → Save |
| 3 | Lua 스크립트 추가 | 5 min | spinner.lua: `on_update(id, gen, dt)` 가 회전 → Inspector "+ script" → 경로 설정 → Play → 회전 |
| 4 | 적 + 입력 | 7 min | player.lua + enemy.lua: input.is_key_pressed / world.find_by_tag / 거리 검사 |
| 5 | UI + 점수 | 5 min | 자체 immediate-mode UI 호출 — Hud panel 그리기 + 점수 증가 |
| 6 | Save + Build | 3 min | Save 후 `voltex build --target <host>` → `dist/minigame.exe` 실행 |
| 7 | 다음 단계 | 2 min | 멀티 OS 빌드, asset packing, 멀티플레이어, ... 안내 |

각 chapter 끝에:
- **체크포인트 commit hash** — examples/tutorial_minigame 의 git history 와 매핑 (사용자가 막히면 `git checkout <hash>` 로 비교)
- **스크린샷 마커**: `<screenshot: minigame chapter X 결과>` — 사용자가 실제 머신에서 캡쳐 후 PR 환영

### 3.3 examples/tutorial_minigame/

```
examples/tutorial_minigame/
├── Cargo.toml              # cargo run -p tutorial_minigame 호환
├── src/
│   └── main.rs             # RuntimeApp::new(cfg).run() 4 줄
├── voltex.toml             # voltex CLI 빌드용 매니페스트
├── scene.vscn              # editor 에서 만든 초기 씬
├── scripts/
│   ├── spinner.lua         # chapter 3 산출물
│   ├── player.lua          # chapter 4
│   └── enemy.lua           # chapter 4
└── assets/
    └── .gitkeep
```

**듀얼 워크플로 호환**:
- `cargo run -p tutorial_minigame` — code-first 사용자가 그대로 실행 가능 (workspace 멤버)
- `voltex build` (튜토리얼 chapter 6 의 산출) — CLI 워크플로

`Cargo.toml` 의 path 가 정확하고 workspace 에 추가되어야 `cargo build --workspace` 가 깨끗.

## 4. 파일 변경 / 신규

| 파일 | 변경 |
|---|---|
| `README.md` | 신규 (또는 기존 placeholder 교체) |
| `docs/tutorial.md` | 신규 (7 chapter, ~150 줄 markdown) |
| `examples/tutorial_minigame/Cargo.toml` | 신규 (workspace 멤버) |
| `examples/tutorial_minigame/src/main.rs` | 신규 (RuntimeApp 4줄) |
| `examples/tutorial_minigame/voltex.toml` | 신규 |
| `examples/tutorial_minigame/scene.vscn` | 신규 (hand-edited JSON, 3~5 entity) |
| `examples/tutorial_minigame/scripts/{spinner,player,enemy}.lua` | 신규 |
| `Cargo.toml` (workspace) | tutorial_minigame 멤버 추가 |
| `docs/dev/STATUS.md` | Cycle 14 entry + Examples 목록 +1 |
| `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` | Cycle 14 마커 `[✅]` |

## 5. Done 기준

- [x] `cargo build --workspace` 클린 (tutorial_minigame 컴파일)
- [x] `cargo run -p tutorial_minigame` 으로 실행 가능 (수동 검증 권장, 코드 빌드 충족이 자동 게이트)
- [x] README 의 5분 quickstart 4 줄을 그대로 실행 가능
- [x] tutorial.md chapter 1~7 자기 완결 (스크린샷 없이도 따라갈 수 있는 텍스트)
- [x] STATUS / roadmap 갱신

## 6. 한계 / 한 줄 deferral

- **스크린샷 미포함** — 현재 환경은 GUI 자동화 부재. README + tutorial 본문에 `<screenshot: ...>` placeholder 가 6~10 개. v1.0 release 직후 사용자가 실 머신에서 캡쳐 + PR 가능.
- **에디터 모드 voltex_cli 명령** — 로드맵 § 1.2 의 `voltex_cli --bin voltex_editor` 는 미구현 (CLI 의 editor mode 는 v1.1 후속). 튜토리얼은 `examples/editor_demo` 를 에디터 진입점으로 안내.

## 7. 세션 분할

| 세션 | 작업 |
|---|---|
| **1** | `README.md` (5분 quickstart + feature list + license + contributing) |
| **2** | `docs/tutorial.md` chapter 1~4 + `examples/tutorial_minigame/` 스캐폴드 (voltex.toml + scene.vscn + scripts) |
| **3** | tutorial.md chapter 5~7 + tutorial_minigame 의 src/main.rs + workspace 등록 + STATUS / roadmap |

## 8. 리스크

| # | 위험 | 완화 |
|---|---|---|
| 1 | tutorial_minigame 의 Lua 스크립트가 실제 voltex_script API 와 어긋남 | 각 chapter 의 코드 스니펫을 작성하면서 즉시 `cargo build` 로 검증. Lua 의 경우 spinner.lua 와 같은 기존 데모 (S1) 의 패턴을 직접 모방 |
| 2 | scene.vscn 의 hand-edit JSON 이 serialize_scene_json 의 출력 포맷과 어긋남 | 한 번 `examples/editor_demo` 의 Save 결과를 참고로 복사. ComponentRegistry::register_defaults 와 일치 |
| 3 | 스크린샷 없으면 사용자가 따라가기 어려움 | ASCII 다이어그램 + 정확한 UI 라벨 표기 (예: "Inspector 패널 우상단의 '+ script' 버튼") 로 보완. 실 머신 사용자의 PR 을 v1.0 후 받음 |

## 9. 관련 문서

- `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 14
- `docs/build-matrix.md` (Cycle 11) — tutorial chapter 6 의 빌드 명령 출처
- `assets/scripts/spinner.lua` (Cycle 5) — Lua 코드 패턴 참고
