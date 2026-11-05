# voltex_cli

## 책임 (한 줄)
`voltex` 바이너리 — `new` / `build` / `run` subcommand 로 voltex.toml 기반 프로젝트를 스캐폴드, 빌드, 실행한다.

## 공개 인터페이스
- Bin name: `voltex` (Cargo `[[bin]] name = "voltex"`)
- Subcommands:
  - `voltex new <name>` — 빈 프로젝트 스캐폴드 (voltex.toml + scene.vscn + scripts/spinner.lua + assets/.gitkeep)
  - `voltex build [--target <triple>] [--manifest-path <dir>] [--engine-path <repo>]` — voltex.toml 읽고 wrapper crate 생성 + `cargo build --release` + `<project>/dist/<name>(.exe)` + 자원 복사
  - `voltex run` — `build` 후 산출 exe 실행 (exit code 전파)
  - `voltex --help`

## voltex.toml 매니페스트
```toml
[project]
name = "my_game"           # required
version = "0.1.0"          # required

[build]
scene = "scene.vscn"               # optional, default = "scene.vscn"
scripts = ["scripts/**/*.lua"]     # optional
assets = ["assets/**/*"]           # optional
window_title = "My Game"           # optional, default = project.name
window_size = [1280, 720]          # optional
targets = ["windows-x64"]          # optional, default = host triple

[build.windows-x64]                 # 선택적 per-target 섹션. v1 에서는 icon 만 기록 (사용은 v1.1+)
icon = "assets/icon.ico"
```

자체 미니 TOML 파서 (외부 crate 의존 없음, voltex_ecs 가 JSON 자체 구현하는 정책과 동일). 지원: section header `[a]` / `[a.b]`, string / int / bool / string array / int array, `#` 주석, 빈 줄. 미지원: multiline string, dotted key, inline table, datetime, float.

## 모듈 구조
```
src/
├── main.rs       # dispatch + 도움말 (~50줄)
├── manifest/
│   ├── mod.rs    # Manifest struct + 스키마 검증 + load_from_dir (~350줄)
│   ├── toml.rs   # 미니 TOML 파서 (줄·열 위치 에러) (~270줄)
│   └── tests.rs  # 단위 테스트 + 변조 퍼징
├── error.rs      # CliError + Display (사람-친화 메시지) + Result<T>
├── util.rs       # copy_optional_file/dir, escape_for_rust_string
├── new.rs        # scaffold (디렉토리 + 매니페스트 + 샘플 lua)
├── build/
│   ├── mod.rs    # build_project (kind 분기 → build_rust_target / build_scene_target), resolve_build_target, pack_archive, 씬 에셋 참조 스캔
│   ├── wrapper.rs # render_wrapper_main 등 래퍼 크레이트 템플릿 (Cargo.toml, main.rs)
│   └── tests.rs  # 단위 테스트
└── run.rs        # build 후 exe 실행
```

## 동작 (build 의 단계)
1. `--manifest-path` 또는 cwd 에서 `voltex.toml` 로드
2. `--target` 또는 `manifest.build.targets[0]` 또는 host triple 결정
3. `<project>/dist/wrapper/{Cargo.toml,src/main.rs}` 생성
   - `Cargo.toml`: `voltex_runtime` path 의존 (engine_path 의 `engine/voltex_runtime`)
   - `src/main.rs`: `RuntimeApp::new(RuntimeConfig::new().with_window_title(...).with_window_size(...).with_audio(Vec::new()).with_vfs(...).with_initial_scene(...))?.run()` — **오디오 상시 활성** (AudioPath resolve/스크립트 오디오가 audio 단계 뒤에 있음; 디바이스 실패는 graceful 무음 — v1.0.106)
4. wrapper 디렉토리에서 `cargo build --release`
5. `target/release/wrapper(.exe)` → `dist/<name>(.exe)` 복사
6. `scene.vscn` / `scripts/` / `assets/` 를 dist 폴더에 복사 (없으면 silent skip)
7. `.vpkg` 패킹 시 씬의 `texture_path`/`mesh_path`/`audio_path` 참조 스캔 (`scan_scene_asset_paths`, 포맷 자동감지 VSCN/JSON/text) — 선언 디렉토리 밖 참조 파일 자동 동봉, 디스크에 없으면 경고 후 계속 (v1.0.101; audio v1.0.103)

## 의존성
- workspace 내부: voltex_asset (`.vpkg` write/read), voltex_ecs (빌드 시 씬 에셋 참조 스캔 `scan_scene_asset_paths` — v1.0.101). voltex_runtime 은 wrapper 의 의존, voltex_cli 의 의존 아님
- dev-dependency: voltex_image (E2E 번들 테스트의 PNG 인코딩), voltex_audio (동 테스트의 WAV 생성)
- 외부 crate: 없음 (TOML 파서 자체 구현)
- 시스템 의존: `cargo` 가 PATH 에 있어야 함 (없으면 `CargoMissing` 친화 에러)

## 변경 시 주의점
- **voltex_runtime 위치**: `--engine-path` cli flag 또는 `VOLTEX_REPO` 환경변수로 받음. crates.io publish 후에는 정식 의존성으로 교체 예정.
- **wrapper crate 생성 vs workspace 의존**: 매번 wrapper 생성 + cargo build 호출. wrapper 의 target dir 은 `<project>/dist/wrapper/target` 으로 격리 → 사용자 프로젝트 간 incremental cache 공유는 안 됨. B2 (asset packing) 시 검토.
- **wrapper main.rs codegen**: `escape_for_rust_string` 이 backslash/quote 만 처리. 사용자 manifest 의 `window_title` 가 newline / control char 가지면 코드 깨질 수 있음 (현재 silent). 다음 cycle 에 validate.
- **--target 분기 미구현**: B3 (Cycle 11) 에서 native build per-OS 정책 구현 예정. 현재는 매니페스트의 `targets[0]` 만 보고 cargo build (즉 host triple 로 빌드).

## 테스트
- 19개 단위 테스트 (manifest 9 + new 3 + build 2 + util 3 + error 0 — Display 검증은 추후)
- 실행: `cargo test -p voltex_cli`
- E2E (`tests/`, 전부 `#[ignore]` — `cargo test -p voltex_cli -- --ignored` 로 명시 실행, wrapper 의 release 엔진 링크 ~2-5분):
  - `e2e_new_build.rs` — `voltex new` → `voltex build` → dist exe + 라이선스 번들 확인
  - `e2e_texture_bundle.rs` — v1.0.96~103 체인 통검 (3종 에셋): `texture_path` + `audio_path` + `mesh_path` 씬 → 자동 동봉 로그 + `.vpkg` 키 확인 → exe 를 루즈 파일 없는 디렉토리에서 12초 구동, `initial scene ... loaded` + texture/audio/mesh resolve 양성 로그 단언 (실창/GPU 필요). mesh_path 는 전용 엔티티 — primitive_mesh 가 있으면 materialize 가 MeshHandle 을 먼저 부착해 resolve 가 멱등 skip

## 수동 검증 절차
```bash
export VOLTEX_REPO=<engine repo root>
cargo install --path tools/voltex_cli   # 1회
cd /tmp
voltex new mygame
cd mygame
voltex build
./dist/mygame.exe   # Windows 에서 빈 윈도우
voltex run          # 동등
```
