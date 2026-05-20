# Cycle 8 (B1) — 정식 voltex CLI + voltex.toml + Windows exe 설계

**작성일**: 2026-05-21
**상위 로드맵**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 8
**Sub-project**: B
**세션 수**: 3
**의존**: Cycle 7 (B1-spike) — 학습 노트가 출발점

---

## 1. 목적

B1-spike 의 prototype 을 production-grade 로 끌어올린다. `voltex.toml` 매니페스트 + 3개 subcommand (`new`, `build`, `run`) + 사람-친화 에러 + 10+ 단위 테스트.

## 2. Scope

### 2.1 In-scope
- `voltex.toml` 매니페스트 형식 정의 + 파싱 (자체 TOML 파서 — 외부 의존 회피, 동일 정책으로 voltex_ecs 의 JSON 자체 구현과 일관)
- 3 subcommand:
  - `voltex new <name>` — 프로젝트 디렉토리 스캐폴드 (voltex.toml + scene 빈 파일 + sample lua + main 템플릿 없음 — wrapper 는 build 가 만듦)
  - `voltex build [--target <triple>]` — 매니페스트 기반 wrapper 생성 + cargo build --release + dist 산출
  - `voltex run` — build 후 dist/exe 실행
- 사용자 친화 에러: "missing field `[project] name` in voltex.toml" / "scene path not found: <path>" 등
- 단위 테스트 ≥10 (manifest 파싱 success/failure 케이스, path resolution, build command 분해)

### 2.2 Out-of-scope
- asset packing (Cycle 9 B2)
- multi OS native build (Cycle 11 B3) — 이 cycle 은 windows-x64 만
- 사용자 Rust 시스템 추가 — B1 의 wrapper main.rs 는 scene/scripts 로딩만
- `voltex publish` / `voltex test` 등 추가 명령

## 3. voltex.toml 매니페스트 형식

```toml
[project]
name = "my_game"           # required, identifier 패턴 [A-Za-z_][A-Za-z0-9_-]*
version = "0.1.0"          # required

[build]
scene = "scene.vscn"                # optional, default = "scene.vscn"
scripts = ["scripts/**/*.lua"]      # optional, default = ["scripts/**/*.lua"]
assets = ["assets/**/*"]            # optional, default = ["assets/**/*"]
window_title = "My Game"            # optional, default = project.name
window_size = [1280, 720]           # optional, default = [1280, 720]
targets = ["windows-x64"]           # optional, default = host triple

[build.windows-x64]
icon = "assets/icon.ico"            # optional (실제 적용은 v1.1+ — 매니페스트 형식만 정의)
```

### 3.1 자체 TOML 파서 정책
voltex_ecs 가 JSON 자체 구현하는 패턴 따라서 **외부 crate 의존 없이** 미니 TOML 파서를 작성한다. 지원 범위 (sufficient subset):
- `[section]`, `[section.subsection]` 헤더
- `key = value` (value = string, integer, array of string, array of integer)
- 주석 (`#` 이후 EOL 까지)
- 빈 줄

지원 안 함: multiline string, dotted key, inline table, datetime, float — v1 매니페스트에 불필요.

파서는 `crates/voltex_cli/src/manifest.rs` 안의 한 파일. ~200줄 추정.

## 4. 모듈 구조

```
crates/voltex_cli/src/
├── main.rs           # CLI dispatch + subcommand 라우팅. 짧게 유지 (~60줄).
├── manifest.rs       # 미니 TOML 파서 + Manifest struct + load_from_dir
├── new.rs            # voltex new <name> 스캐폴드
├── build.rs          # voltex build [--target X]: wrapper 생성 + cargo build + 자원 복사
├── run.rs            # voltex run: build + exe 실행
├── util.rs           # copy_optional_file/dir, escape_for_rust_string 등 (spike 에서 이전)
└── error.rs          # CliError + Result<T> (사람-친화 메시지)
```

각 파일 ≤ 300줄 예상.

## 5. CLI dispatch

```rust
// main.rs
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("new")   => run_or_exit(new::cmd(&args[2..])),
        Some("build") => run_or_exit(build::cmd(&args[2..])),
        Some("run")   => run_or_exit(run::cmd(&args[2..])),
        Some("--help" | "-h") | None => { print_help(); ExitCode::SUCCESS }
        Some(other) => { eprintln!("voltex: unknown subcommand `{}`", other); ExitCode::from(2) }
    }
}
```

### 5.1 `voltex new <name>`
- 인자 1개 (프로젝트 이름) 검증
- 디렉토리 `<name>/` 생성 (이미 있으면 에러)
- `<name>/voltex.toml` 작성 (디폴트 매니페스트)
- `<name>/scene.vscn` 빈 파일 (JSON `{"entities":[]}`)
- `<name>/scripts/spinner.lua` 샘플 (engine repo 의 assets/scripts/spinner.lua 복사 또는 inline)
- `<name>/assets/.gitkeep`

### 5.2 `voltex build [--target X]`
- 현재 디렉토리 또는 `--manifest-path <path>` 옵션으로 voltex.toml 위치 결정
- 매니페스트 파싱 → Manifest struct
- target 미지정 시 host triple 사용 (Windows host → "windows-x64")
- spike 의 로직 그대로 확장:
  1. dist 디렉토리 생성
  2. wrapper crate 생성 (`dist/wrapper/Cargo.toml` + `src/main.rs`)
     - wrapper main.rs 가 매니페스트의 window_title/window_size 를 with_window_title/with_window_size 로 전달
     - scene 파일 로드 호출 `with_initial_scene` (옵션)
  3. `cargo build --release --target-dir dist/wrapper/target` 호출
  4. exe 를 `dist/<name>.exe` 로 복사
  5. scene + scripts + assets 를 dist 폴더에 복사

### 5.3 `voltex run`
- 내부적으로 `build` 호출 후 `dist/<name>.exe` 실행
- exit code 전파

## 6. 에러 처리

```rust
// error.rs
pub enum CliError {
    MissingArg(&'static str),
    ManifestNotFound(PathBuf),
    ManifestParse { line: usize, msg: String },
    MissingField { section: &'static str, field: &'static str },
    InvalidValue { field: &'static str, got: String },
    SceneNotFound(PathBuf),
    NewTargetExists(PathBuf),
    CargoMissing,
    CargoBuildFailed(i32),
    Io(std::io::Error),
}
```

`Display` 구현 시 친화 메시지:
```
error: missing required field [project] name in voltex.toml
help: add `name = "<your project name>"` under [project]
```

## 7. 테스트 (≥10, 작성 12)

| # | 파일 | 케이스 |
|---|---|---|
| 1 | manifest.rs | `parses_minimal_manifest` (project name+version 만) |
| 2 | manifest.rs | `parses_full_manifest` (build 섹션 + window_title 등) |
| 3 | manifest.rs | `missing_name_returns_missing_field` |
| 4 | manifest.rs | `invalid_window_size_array_returns_error` |
| 5 | manifest.rs | `array_of_strings_parses` (scripts/assets) |
| 6 | manifest.rs | `comments_and_blank_lines_are_skipped` |
| 7 | manifest.rs | `subsection_header_parses` (`[build.windows-x64]`) |
| 8 | new.rs | `new_creates_expected_files` (임시 tmp 디렉토리에서 검증) |
| 9 | new.rs | `new_refuses_existing_directory` |
| 10 | build.rs | `wrapper_main_includes_window_title_from_manifest` (생성된 main.rs 문자열 검증, cargo build 호출 안 함) |
| 11 | build.rs | `build_skips_missing_assets_silently` |
| 12 | util.rs | `escape_for_rust_string_handles_special_chars` (spike 에서 이전) |

cargo build 자체 호출 테스트는 비용/환경 의존 큼 → CI 회피, 수동 검증.

## 8. 시각 검증 (수동)
- `cargo install --path crates/voltex_cli` (PATH 등록)
- `voltex new mygame` → `mygame/` 디렉토리 생성 + voltex.toml + scripts/spinner.lua
- `cd mygame && voltex build` → `dist/mygame.exe` 생성
- `./dist/mygame.exe` 더블클릭 → 윈도우 뜸 + scene 빈 World
- `voltex run` 도 동일 결과

## 9. 세션 분할

### Session 1: 매니페스트 파서 + 에러 + 테스트
- `manifest.rs` (자체 TOML 파서 + Manifest struct + load_from_dir)
- `error.rs` (CliError + Display + From<std::io::Error>)
- 테스트 #1~7

### Session 2: voltex new + voltex build (Windows 타겟)
- `util.rs` (spike 에서 이전)
- `new.rs` (스캐폴드)
- `build.rs` (manifest-driven build + wrapper 생성)
- main.rs 의 dispatch 갱신
- 테스트 #8~12

### Session 3: voltex run + 친화 에러 메시지 + 최종 정리
- `run.rs`
- 에러 메시지 polish + help 출력
- CODEMAP / STATUS / 로드맵 갱신

## 10. 위험 / 미지수

### 10.1 voltex_runtime 경로
spike 의 `VOLTEX_REPO` 환경변수 대신 매니페스트의 `[engine] path` 옵션 또는 cli flag `--engine-path` 로 받자. 더 단순한 방법: voltex_cli 가 빌드되는 시점에 `env!("CARGO_MANIFEST_DIR")` 로 자신의 위치를 알고, `../voltex_runtime` 으로 상대 경로 계산. 사용자가 `cargo install` 한 경우에는 그 자체 위치가 임의이므로 이 방법은 안 됨.

**결정**: B1 도 환경변수 `VOLTEX_REPO` 유지 + `--engine-path` cli flag 추가. crates.io publish 가 가능해지는 시점 (v1.0 후) 에 정식 의존성 메커니즘 도입.

### 10.2 host triple 추출
`std::env::consts::OS` + `std::env::consts::ARCH` 조합으로 추출. windows-x64, linux-x64, macos-arm64 매핑은 build.rs 안의 상수 테이블.

### 10.3 cargo target-dir 공유
spike 가 wrapper 마다 별도 target → 디스크 폭발. B1 에서 `--target-dir <dist>/wrapper/target` 명시. 같은 프로젝트는 incremental cache 재활용.

### 10.4 단위 테스트의 cargo invocation
build.rs 의 cargo invocation 자체를 단위 테스트하는 건 비용 과다 → 만들어지는 main.rs **문자열**만 검증 (테스트 #10). 실제 cargo build 는 수동 검증.

## 11. 변경 영향도

| Crate / Module | 변경 |
|---|---|
| voltex_cli (5 신규 모듈) | 신규 |
| voltex_cli::main | spike 의 단일 main 을 dispatch 로 분리 |
| 다른 crate | 변경 없음 |
| docs/STATUS.md | Cycle 8 entry |
| docs/superpowers/specs/.../roadmap.md | Cycle 8 `[✅]` |
| 새 폴더 `crates/voltex_cli/CODEMAP.md` | 신규 |

## 12. Done 기준 (로드맵 § Cycle 8)
- [ ] `voltex.toml` 매니페스트 파싱 + Manifest struct + 7개 단위 테스트
- [ ] `voltex new <name>` 가 스캐폴드 생성
- [ ] `voltex build` 가 매니페스트 기반 dist 출력
- [ ] `voltex run` 이 build + 실행
- [ ] 친화 에러 메시지
- [ ] voltex_cli 단위 테스트 ≥10 (작성 12)
- [ ] B1-spike 학습 노트의 미지수 4개 (voltex.toml 필드, wrapper main 의 자원 로딩, target-dir 공유, host triple) 해소
- [ ] roadmap Cycle 8 `[✅ 3/3]`
