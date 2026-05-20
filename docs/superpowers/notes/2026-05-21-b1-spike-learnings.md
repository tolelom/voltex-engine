# B1-spike 학습 노트 (2026-05-21)

**관련 spec**: `docs/superpowers/specs/2026-05-21-b1-spike-design.md`
**Cycle**: 7 (B1-spike, 1-session timebox)
**상태**: 시간 내 완료 — prototype 동작 + 학습 정리

목적: B1 (Cycle 8 정식 CLI) 의 미지수 (CLI 골격, 프로젝트 디렉토리 레이아웃, cargo invocation, exe 산출) 을 한 번 통과시켜본 경험에서 무엇이 작동했고 무엇이 어려웠는지 기록.

---

## 1. 결정 / 작동한 패턴

### 1.1 패키지명 vs binary 이름 분리
- `crates/voltex_cli/Cargo.toml`: `[package] name = "voltex_cli"`, `[[bin]] name = "voltex"` → 사용자는 `voltex build ...` 로 호출하면서 facade `voltex` crate 와 패키지명 충돌은 회피.
- B1 에서도 그대로 유지. `cargo install --path crates/voltex_cli` 로 system PATH 에 `voltex` 바이너리가 들어간다.

### 1.2 wrapper crate 생성
- `<project_dir>/dist/wrapper/Cargo.toml` + `src/main.rs` 를 cli 가 동적으로 작성하고, 그 디렉토리에서 `cargo build --release` 호출.
- 산출물 `target/release/wrapper.exe` 를 `<project_dir>/dist/<name>.exe` 로 복사.
- 단순하고 동작함. 단점: wrapper 당 별도 target 디렉토리 → 디스크 사용량 큼. 캐시 공유 전략은 B2 에서 검토.

### 1.3 voltex_runtime 의존 path
- prototype 에서는 환경변수 `VOLTEX_REPO` 로 엔진 repo 절대 경로 받음.
- TOML 직렬화 시 Windows 경로의 `\` 를 `/` 로 치환해야 함 (아니면 TOML 파서가 이상하게 해석).
- B1 정식 해결: voltex_runtime 을 crates.io 또는 vendored 디렉토리에 두고 버전 핀. 현재 v0.1.0 단계라 crates.io publish 없음 → path 방식 유지. B1 spec 작성 시 명시.

### 1.4 자원 복사 정책
- `scene.vscn` 파일 + `scripts/` 디렉토리 + `assets/` 디렉토리를 dist 폴더에 단순 복사. exe 와 같은 디렉토리.
- 실행 시 runtime 이 이걸 어떻게 찾는지는 v1.1 의 VFS (B2) 영역. prototype 의 wrapper main.rs 는 자원 로딩 안 함 — 빈 윈도우만 띄움.

## 2. 어려웠던 점

### 2.1 cargo invocation 환경 의존
- `cargo` 가 PATH 에 있어야 함. 없으면 `failed to spawn cargo`. CI / packaged 환경에서는 rustup-init 또는 portable cargo 필요.
- B1 의 사람-친화 에러 메시지가 여기서 중요해질 듯: "cargo not found in PATH — install rustup at https://rustup.rs".

### 2.2 wrapper main.rs 에 사용자 데이터 주입
- 프로젝트 이름을 그대로 `with_window_title` 인자에 박는데, 이름에 `"` / `\` 가 있으면 컴파일 에러 → 이스케이프 함수 추가 (`escape_for_rust_string`).
- B1 에서는 manifest 의 `[project] name` 가 더 정형화될 거라 큰 문제 아닐 수 있지만, 같은 패턴 (자원 path 주입 등) 이 반복되면 codegen helper 빼서 별도 모듈로.

### 2.3 wrapper 의 cargo build 가 매번 cold start
- 매 invocation 마다 voltex_runtime 의 모든 dependency 가 재컴파일됨 (target dir 분리). voltex_runtime 자체가 wgpu/winit/voltex_script(=Lua C) 까지 끌고 가서 시간이 1~3분.
- B2 에서 target 디렉토리 공유 전략 (`CARGO_TARGET_DIR` 환경변수 설정) 검토 필요.

## 3. B1 (Cycle 8) 의 voltex.toml 매니페스트에 들어가야 할 것

prototype 경험상 다음이 명시되어야 사용성 충분:

```toml
[project]
name = "my_game"
version = "0.1.0"

[build]
scene = "scene.vscn"         # 옵션. 없으면 빈 World 부팅
scripts = ["scripts/**/*.lua"]
assets = ["assets/**/*"]
window_title = "My Game"     # main.rs 의 with_window_title 인자
window_size = [1280, 720]    # 옵션, 기본 1280x720

# 차후: [build.windows-x64], [build.linux-x64], [build.macos-arm64] 의 target-specific 설정
```

### B1 가 추가로 처리해야 할 미지수
- voltex_runtime 의존 path 또는 버전 (현재 path 방식 — `VOLTEX_REPO` 환경변수 자동 검출)
- `voltex new <name>` 의 디폴트 디렉토리 구조 (이 노트 § 1.2 그대로)
- `voltex run` 의 동작 (build 후 dist/<name>.exe 실행)
- 사용자 친화 에러 메시지 (현재는 stderr panic-like)
- 단위 테스트: 매니페스트 파싱 / 경로 해석 / 빌드 명령 분해 등

## 4. 우회 / 보류한 것

- **사용자 Rust 시스템 추가**: wrapper main.rs 에 사용자 `add_system` 호출 주입 불가. B1 에서 `[build] user_systems = [...]` 또는 별도 메커니즘 필요. 현재 prototype 은 빈 RuntimeApp 만.
- **scene/script 자동 로딩**: wrapper main.rs 가 `with_initial_scene` 안 부름. dist 폴더에 자원 풀어둬도 runtime 이 못 찾음. B1 에서 wrapper 가 `std::env::current_exe()` 기준 상대 경로로 자원 찾도록.
- **OS 분기**: prototype 은 `cfg!(windows)` 로 exe 확장자만 분기. B3 의 일.

## 5. B1 spec 작성 시 살릴 코드 / 폐기할 코드

- 살릴: `copy_optional_dir` / `copy_dir_recursive` / `escape_for_rust_string` — 유틸리티성, B1 에 그대로 이전.
- 살릴: wrapper 디렉토리 생성 + cargo build 호출 패턴.
- 폐기: `VOLTEX_REPO` 환경변수 의존 — manifest 또는 cli flag 로 교체.
- 폐기: `<project_dir>` 의 마지막 path component 를 프로젝트 이름으로 — manifest 의 `[project] name` 사용.
- 폐기: 단일 main.rs 의 ~200줄 평면 구조 — B1 에서는 `manifest.rs` / `build.rs` / `scaffold.rs` 등으로 분리.

## 6. 종합

prototype 1세션 timebox 안에 골격 + 1개 동작하는 subcommand 작성 완료. 위에 정리한 결정과 미지수 해소 방향이 B1 (Cycle 8) spec 의 출발점.
