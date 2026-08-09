# Voltex Engine

> Rust 로 작성한 winit + wgpu 기반 3D 게임 엔진. 런타임 crate 의존 3 개
> (`winit` / `wgpu` / `bytemuck`) 만으로 PBR, 물리, 오디오, AI, 에디터, Lua
> 스크립팅, 멀티 OS 빌드 파이프라인까지 제공한다. 자세한 카운트와 정책은
> 아래 "의존성 정책" 섹션 참고.

![status: v1.0.114](https://img.shields.io/badge/status-v1.0.114-brightgreen)
![clippy: 0 warnings](https://img.shields.io/badge/clippy-0%20warnings-brightgreen)
![audit: 0 vulnerabilities](https://img.shields.io/badge/cargo--audit-0%20vulnerabilities-brightgreen)
![coverage: 82%](https://img.shields.io/badge/coverage-82%25%20lib-brightgreen)
![licenses: permissive only](https://img.shields.io/badge/licenses-permissive%20only-brightgreen)
![parser fuzz: 0 panics](https://img.shields.io/badge/parser%20fuzz-0%20panics-brightgreen)
![rendering: visually verified](https://img.shields.io/badge/rendering-visually%20verified-brightgreen)
![runtime: Windows verified](https://img.shields.io/badge/runtime-Windows%20verified-green)
![runtime: Linux/macOS unverified](https://img.shields.io/badge/runtime-Linux%2FmacOS%20unverified-orange)
![rust: stable](https://img.shields.io/badge/rust-stable-orange)
![license: MIT OR Apache--2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green)

---

## 스크린샷

엔진의 실제 렌더 출력 (헤드리스 오프스크린 캡처, `voltex_capture`). 재현·세부는
[`docs/visual/`](docs/visual/README.md).

| PBR (Cook-Torrance) | Directional shadows | Multi-light |
|---|---|---|
| ![PBR sphere grid](docs/visual/pbr_grid.png) | ![Shadow scene](docs/visual/shadow_scene.png) | ![Multi-light](docs/visual/multi_light.png) |
| **7×7 metallic/roughness + IBL** | **2-pass shadow map** | **컬러 point/spot lights** |

| Gizmos | Editor UI |
|---|---|
| ![Translate gizmo](docs/visual/gizmo_translate.png) | ![Editor UI](docs/visual/editor_ui.png) |
| **translate / rotate / scale 핸들** | **IMGUI + 폰트 아틀라스** |

---

## 5분 quickstart

### 예제 실행

```bash
git clone https://git.tolelom.xyz/tolelom/game_engine
cd game_engine
cargo run --example triangle
```

윈도우가 열리고 색깔 삼각형이 보이면 성공. `ESC` 로 종료.

다른 example: `cargo run --example pbr_demo` / `runtime_demo` / `editor_demo` /
`shadow_demo` / `multi_light_demo` / `ibl_demo` / `deferred_demo` /
`hierarchy_demo` / `asset_demo` / `audio_demo` / `model_viewer` / `many_cubes`.

### 새 게임 만들기 (`voltex` CLI)

```bash
# 위 clone 한 디렉토리에서:
cargo install --path crates/voltex_cli       # voltex 바이너리 설치
export VOLTEX_REPO=$(pwd)                    # 또는 매번 --engine-path 사용
cd ~/projects
voltex new my_game && cd my_game             # scene.vscn + spinner.lua scaffold
voltex build                                 # → dist/my_game.exe (self-contained)
voltex run                                   # 빌드 + 실행
```

`VOLTEX_REPO` 가 없으면 `voltex build --engine-path /path/to/voltex` 로
한 번씩 명시. CLI 가 누락 시 친화 에러 메시지로 두 가지 방법을 안내한다.
PowerShell 은 `$env:VOLTEX_REPO='C:/path/to/voltex'`.

자세한 단계별 안내는 [docs/tutorial.md](docs/tutorial.md) 참고 (30분).

---

## 두 가지 워크플로

### Code-first — Bevy / Macroquad 스타일

```rust
use voltex::prelude::*;

fn main() -> voltex::runtime::RuntimeResult<()> {
    let cfg = RuntimeConfig::new()
        .with_window_title("hello voltex")
        .with_initial_scene("scene.vscn");
    RuntimeApp::new(cfg)?.run()
}
```

prelude 한 줄로 `World` / `Entity` / `Transform` / `RuntimeApp` /
`MeshHandle` / `Bounds` / `ScriptComponent` 등 자주 쓰는 타입이 모두
들어온다.

### Editor-first — Unity / Godot 스타일

```bash
# 프로젝트 스캐폴드 + 빌드 + 실행 (현재 OS 의 단일 exe 산출)
voltex new my_game
cd my_game
voltex build
./dist/my_game        # Linux/macOS
./dist/my_game.exe    # Windows
```

`voltex.toml` 매니페스트가 scene + scripts + assets 를 `.vpkg` 아카이브로
묶어 실행파일에 임베드 — 산출물은 단독 폴더에 두어도 동작한다 (자원 외부
파일 의존 0). 자세한 OS 별 사전 요구는
[docs/build-matrix.md](docs/build-matrix.md).

---

## Features

- **ECS** — `World` / `Entity` (id + generation) / `Component` trait / `Scheduler`
  + 의존성-기반 그룹 빌드, 씬 그래프 (`Parent` / `Children`), `.vscn` JSON + 바이너리
  포맷 직렬화
- **Rendering** — PBR (Cook-Torrance, ORM + emissive 텍스처 맵), normal mapping,
  CSM / Point cube map / Spot shadow, IBL (SH irradiance + compute BRDF LUT,
  Hosek-Wilkie sky), 디퍼드 G-Buffer 4 MRT, SSGI (bilateral blur + half-res
  + temporal), RT shadows / reflections / AO (wgpu BLAS+TLAS), post (HDR +
  ACES + Bloom + TAA + SSR + DOF + Motion blur + Auto exposure)
- **Image / Mesh parsers** — OBJ + glTF 2.0 (+ GLB, animations 파싱) + PNG
  (deflate + filter) + JPG (Baseline + Progressive) + BMP — 모두 자체 구현
- **Physics** — Sphere / Box / Capsule / ConvexHull / Mesh collider, GJK + EPA,
  BVH (refit), rigid body (semi-implicit Euler + sequential impulse + Coulomb
  friction + sleep + island), CCD, raycast (all-hits + closest), Ray vs AABB /
  sphere / capsule / box / triangle / mesh
- **Audio** — WAV (16/24/32-bit) + OGG/Vorbis 자체 디코더, WASAPI (Windows) /
  ALSA (Linux) / CoreAudio (macOS) / Null backend, 3D 사운드 + Doppler + HRTF
  (ITD/ILD) + Reverb (Schroeder) + Echo + Occlusion + MixGroup + AudioBus
  + 동적 그룹 + EffectChain 파이프라인
- **AI** — NavMesh (manual + voxelization 자동 빌더), A\* + Funnel (string
  pulling), steering (seek/flee/arrive/wander) + 동적 장애물 회피
- **Networking** — UDP, Reliable + Ordered channel (ACK + 재전송 + RTT),
  Snapshot + delta 압축, InterpolationBuffer, LagCompensation (history rewind),
  PacketCipher (XOR + anti-replay) + AuthToken
- **Scripting** — Lua 5.4 자체 빌드 + FFI (외부 crate 0), `ScriptComponent`
  per-entity, `ScriptInstancePool` (.lua 당 하나의 LuaState 공유), 핫리로드,
  17 개 engine API (spawn / destroy / position / rotation / scale / tag /
  find_by_tag / iter_entities / iter_by_tag / add_child / remove_child)
- **Editor** — Immediate-mode UI (자체 구현, 외부 GUI crate 0), 도킹 트리,
  플로팅 패널, 3D 뷰포트 (오프스크린 + 오빗 카메라), Hierarchy / Inspector
  panel, AssetBrowser, 자체 TTF 파서 + 래스터라이저 + 글리프 캐시 + 아틀라스,
  Translate / Rotate / Scale gizmo, AABB picking, Play/Stop 모드 + 라이프사이클
  훅, Undo/Redo (Inverse 스택), AddComponent/RemoveComponent UI
- **Asset packing** — `.vpkg` 아카이브 + `Vfs` (Filesystem ↔ Archive 동일
  API), `include_bytes!` 기반 self-contained exe 산출
- **Build pipeline** — `voltex_cli` (`new` / `build` / `run`) + `voltex.toml`
  매니페스트 + native-only 멀티 OS 디스패치 + 친화 에러 메시지
- **CI / 릴리스** — GitHub Actions full matrix (Linux + Windows blocking,
  macOS advisory), cargo-deny (RUSTSEC + license + duplicate + source),
  cargo-about (license listing), cargo-semver-checks (API stability),
  changelog enforcement (pre-push + PR gate), nightly E2E `voltex new+build`
  smoke test, tag-driven 3-OS release artifact pipeline
  ([`docs/release.md`](docs/release.md))
- **Runtime ergonomics** — `RuntimeApp` 4-line `main()`, HUD primitive
  (FPS + user segment in window title), panic hook → crash log file,
  structured `voltex_runtime::log` (env-driven, no external crate)

---

## 의존성 정책

런타임 crate 직접 의존: **3 개**.

| Crate | Version | 역할 |
|---|---|---|
| `winit` | 0.30.x | 크로스플랫폼 윈도우 + 키보드/마우스 입력 |
| `wgpu` | 28.x | Vulkan / DX12 / Metal / WebGPU 추상 |
| `bytemuck` | 1.x | GPU 메모리 레이아웃 (Pod/Zeroable derive + cast) — wgpu 사용 시 사실상 표준 동반 |

빌드 전용 의존 (런타임 표면 비노출, **2 개**):

| Crate | Version | 역할 |
|---|---|---|
| `thiserror` | 2.x | `Error` derive 매크로 (M4 에러 표준화). 컴파일 후 별도 코드 없음 |
| `cc` | 1.x | `voltex_script` 의 Lua 5.4 C 소스 빌드 스크립트 (`build.rs` 전용) |

OS / 시스템 API (Win32, WASAPI, ALSA, CoreAudio, Lua C) 는 모두 FFI 직접
호출. 그 외 모든 것 (수학, 이미지/메시 파서, 오디오 디코더, 물리, AI,
네트워킹, 에디터 GUI, 폰트 래스터라이저, asset packing, CLI, manifest
parser) 은 자체 구현.

예제/바이너리 전용 보조 (`log`, `env_logger`, `pollster`) 는 라이브러리
crate 어디에도 들어가지 않으며 위 카운트와는 별개다. 트랜지티브 포함
`Cargo.lock` 의 총 crate 수는 298 (대부분 wgpu / winit 의 GPU / 윈도우
스택).

---

## Crate 구조

```
voltex (facade, L4)
├── voltex_math       — Vec / Mat / Quat / AABB / Ray
├── voltex_platform   — Window / Input / Timer
├── voltex_asset      — Handle / Assets / AssetLoader / FileWatcher / Vfs / vpkg
├── voltex_ecs        — World / Entity / Component / Scheduler / Scene
├── voltex_audio      — WAV / Vorbis / mixer / backends
├── voltex_ai         — NavMesh / A* / steering
├── voltex_net        — UDP / reliable / snapshot / interpolation
├── voltex_script     — Lua 5.4 + ScriptInstancePool + engine_api
├── voltex_renderer   — wgpu / PBR / shadows / IBL / deferred / RT / post
├── voltex_renderer_ecs — ECS-driven rendering bridge
├── voltex_physics    — Collider / GJK / RigidBody / raycast / CCD
├── voltex_editor     — IMGUI / dock / viewport / inspector / gizmo / TTF
├── voltex_runtime    — RuntimeApp + winit handler (조립된 앱 진입점)
└── voltex_cli        — bin "voltex" (new / build / run) + voltex.toml
```

자세한 의존 그래프 + 모듈 책임은 [CLAUDE.md](CLAUDE.md) 의 "Crate 의존성
그래프" 섹션 + 각 crate 의 `CODEMAP.md` 참고.

---

## 문서

- [docs/tutorial.md](docs/tutorial.md) — 30분 안에 0에서 빌드된 미니 게임까지
- [docs/voltex-toml-reference.md](docs/voltex-toml-reference.md) — `voltex.toml` 매니페스트 전체 reference (v1.0.16)
- [docs/build-matrix.md](docs/build-matrix.md) — 3 OS 빌드 명령 + GitHub Actions 예시
- [docs/release.md](docs/release.md) — release tagging + 산출물 자동화 (v1.0.34)
- [examples/README.md](examples/README.md) — 14 example 추천 학습 순서 + 카테고리별
- [CONTRIBUTING.md](CONTRIBUTING.md) — 외부 contributor 가이드 (의존성 정책 / 600줄 / 에러 표준 / 첫 PR 체크리스트)
- [docs/STATUS.md](docs/STATUS.md) — 작업 현황 + Cycle history + 테스트 통계
- [docs/DEFERRED.md](docs/DEFERRED.md) — 간소화 / 미뤄진 항목 목록
- [CHANGELOG.md](CHANGELOG.md) — Cycle 별 주요 변경
- [docs/superpowers/specs/2026-03-24-voltex-engine-design.md](docs/superpowers/specs/2026-03-24-voltex-engine-design.md) — 전체 엔진 설계 (원본 spec)
- [docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md](docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md) — v1.0 까지의 cycle 로드맵
- [docs/superpowers/specs/2026-05-21-service-readiness-roadmap.md](docs/superpowers/specs/2026-05-21-service-readiness-roadmap.md) — v1.0.x 서비스 배포 완성도 로드맵
- API reference: `cargo doc --workspace --no-deps --open`

---

## License

Voltex 자체는 **MIT OR Apache-2.0** 듀얼 (사용자 선택) — `LICENSE-MIT`
+ `LICENSE-APACHE` 참고.

트랜지티브 의존성 라이선스 고지는
[THIRD-PARTY-LICENSES.md](THIRD-PARTY-LICENSES.md) 에 자동 생성됨.
v1.0.7 시점: **284 crate 전부 permissive** (MIT 264 / Apache-2.0 11 /
Zlib 3 / BSD 3 / ISC / CC0 / Unicode 각 1). GPL/AGPL/LGPL/proprietary
의존성 0건 — MIT OR Apache-2.0 와 100% 호환.

재생성 (의존성 변경 후):

```bash
cargo install --locked cargo-about --features cli
cargo about generate about.hbs -o THIRD-PARTY-LICENSES.md
```

CI 가 동일 명령으로 license drift 검출 (`license-check` advisory job).
새로운 transitive 가 `about.toml` 의 `accepted` 리스트 밖이면 자동
실패한다.

---

## Contributing

PR 환영. 외부 contributor 용 전체 가이드는 [CONTRIBUTING.md](CONTRIBUTING.md)
에 정리 (의존성 정책, 600줄 한도, 에러 표준, 컴포넌트 등록, 테스트 스타일,
커밋 컨벤션, 첫 PR 체크리스트). 자동 진행 로드맵 + 자동 루프 정책도 동봉.

작업 흐름은 spec → plan → implementation → verify 의 4 단계로, 각 단계의
산출물은 `docs/superpowers/specs/` 와 `docs/superpowers/plans/` 에 보존된다.
AI 작업 시점의 워크플로 (CODEMAP 우선 탐색 등) 는 [CLAUDE.md](CLAUDE.md) 참고.

PR 제출 전 로컬 검증 (CI 와 동일한 게이트):

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test -p voltex_cli
```

위 4개를 매 커밋 자동으로 돌리려면 git hook 설치:

```bash
bash .githooks/install.sh                  # Linux / macOS
# 또는
.\.githooks\install.ps1                    # Windows PowerShell
```

CI (`.github/workflows/ci.yml`) 는 위 4 명령을 Ubuntu + Windows 매트릭스에서
실행하고, macOS 는 advisory (non-blocking) 잡으로 돌린다. 모두 통과해야
merge 가능. Linux 빌드는 ALSA 헤더 (`sudo apt-get install -y libasound2-dev`)
필요 — CI 가 자동 설치한다.

### Coverage

선택 검증 (의존성: `cargo install cargo-llvm-cov` + `rustup component add
llvm-tools-preview`):

```bash
cargo llvm-cov --workspace --lib --summary-only
```

**현재 baseline (v1.0.6)**: 82.06% functions / 82.74% lines / 73.01%
regions across all 14 crate. 80% 평균 목표 충족. 14 crate 중 10 개가
crate-단위 80%+; 낮은 4 crate (`voltex_platform` 46%, `voltex_renderer_ecs`
54%, `voltex_runtime` 61%, `voltex_renderer` 66%) 는 모두 GPU/winit
의존 코드 (BasicRenderer, ApplicationHandler 콜백, shadow/RT/TAA 컴퓨트
셰이더) — 헤드리스 unit test 로는 도달 불가. E2E + GPU 통합 테스트는
v1.1+ 후속.

CI 는 coverage 를 advisory job (`coverage`, `continue-on-error: true`)
으로 측정한다. 임계치 enforcement (`--fail-under-lines 80`) 는 GPU
패스가 통합 테스트로 커버되기 전까지 비활성.

빠른 이슈 / 제안은 git repo 의 issue tracker 에 — Korean / English 둘 다 환영.

---

## 개발자 노트

- Windows 머신에서 1차 검증되었으며, **Linux ALSA + macOS CoreAudio** 백엔드는
  타입 체크 + 단위 테스트는 통과하나 실 머신에서의 audio_demo 소리 출력
  검증은 v1.0 release 후 사용자 머신 + PR 로 진행될 예정. 동일하게 **3 OS
  exe 산출 검증** 도 v1.0 release 직후 사용자 확인 단계로 분리되어 있다 (자세한
  내용은 [docs/build-matrix.md](docs/build-matrix.md) 의 § 검증 한계).
- 자동 루프 (Claude / 다른 agent) 가 `master` 에 push 할 수 있어 작업 전
  `git pull --ff-only` 를 권장.
