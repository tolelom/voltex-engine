# `voltex.toml` Reference (v1.0.x)

> Voltex 프로젝트의 단일 매니페스트. `voltex build` / `voltex run` / `voltex new` 가 모두 이 파일을 읽는다.
> v1.0.11 부터 알 수 없는 키/섹션은 친화 에러로 즉시 거부 (Levenshtein ≤ 3 suggestion 동반).

## 최소 예시 (`voltex new <name>` 가 만드는 형태)

```toml
[project]
name = "my_game"
version = "0.1.0"
```

이것만으로도 `voltex build` 가능. 모든 `[build]` 필드는 디폴트가 있다.

## 전체 예시

```toml
# Top-level comment OK
[project]
name = "my_game"      # 영문/숫자/언더스코어
version = "0.1.0"

[build]
scene = "level1.vscn"                     # 시작 scene (Vfs / fs 경로)
scripts = ["scripts/**/*.lua"]            # glob 배열
assets = ["assets/**/*"]                  # glob 배열
window_title = "My Awesome Game"          # 윈도우 타이틀바
window_size = [1920, 1080]                # [width, height]
targets = ["windows-x64", "linux-x64"]    # 빌드 가능 host 화이트리스트

[build.windows-x64]
icon = "assets/icon.ico"                  # exe 아이콘 (Windows)

[build.linux-x64]
icon = "assets/icon.png"
```

## 섹션 / 필드 화이트리스트

`Manifest::parse` 가 검증하는 정확한 키 셋. 그 외는 `UnknownField` / `UnknownSection` 에러.

### `[project]` (필수)

| 키 | 타입 | 필수 | 기본 | 설명 |
|---|---|---|---|---|
| `name` | string | ✅ | — | 프로젝트 이름. `dist/` 의 exe 이름이 됨 (`dist/<name>.exe`). Rust 게임 crate 빌드에서는 이 이름과 같은 바이너리를 배포하며, 같은 이름이 없고 바이너리가 2개 이상이면 빌드가 실패한다(v1.0.116). |
| `version` | string | ✅ | — | SemVer 권장. 현재 cargo 에는 전달하지 않음 (`voltex_runtime` 의 `--version` 은 자체 crate version) — 향후 동기화 예정. |

### `[build]` (선택, 전부 디폴트)

| 키 | 타입 | 기본 | 설명 |
|---|---|---|---|
| `scene` | string | `"scene.vscn"` | 시작 scene 파일 경로. `voltex.toml` 디렉토리 기준 상대 경로. `voltex build` 가 v1.0.11 path validation 에서 존재 확인. |
| `scripts` | string array | `["scripts/**/*.lua"]` | Lua 스크립트 glob. 각 glob 의 root (strip_glob 결과) 디렉토리가 존재해야 함 (v1.0.11). `voltex build` 가 `.vpkg` 에 묶음. |
| `assets` | string array | `["assets/**/*"]` | 자산 (텍스처/메쉬/사운드) glob. scripts 와 동일 규칙. |
| `window_title` | string | `<project.name>` | OS window 타이틀바. RuntimeApp 의 default app name (크래시 로그 파일 prefix 도 됨, v1.0.10). |
| `window_size` | int array | `[1280, 720]` | `[width, height]`. 둘 다 양의 정수. 다른 길이의 배열은 `InvalidValue` 에러. |
| `targets` | string array | `[<host_triple>]` | 빌드 가능 host 화이트리스트. `voltex build` 가 현재 host 와 mismatch 시 `CrossCompileNotSupported` 또는 `HostNotInManifestTargets` 에러. |

### `[build.<triple>]` (선택, per-target override)

`<triple>` 은 임의 문자열 — open-ended. 알려진 값:

- `windows-x64`
- `linux-x64`
- `macos-x64`
- `macos-arm64`

(`aarch64-linux-android` 같은 사용자 정의 triple 도 허용 — validate_schema 가 prefix `build.` 만 검사.)

| 키 | 타입 | 기본 | 설명 |
|---|---|---|---|
| `icon` | string | — | exe 아이콘 파일 경로. v1.0.x 에서는 metadata 로만 보존, 실제 exe 임베드는 v1.1+. |

## 파싱 규칙 (mini TOML 서브셋)

- `# ... ` 행 끝까지 코멘트 (string literal 안 제외)
- 빈 줄 무시
- 헤더: `[section]` 또는 `[section.sub]` (한 레벨)
- 값: `"string"` / `123` / `true` / `false` / `["a", "b"]` / `[1, 2]`
- escape: `\"`, `\\`, `\n`, `\t` (string 안)
- 미지원: multiline string, dotted key, datetime, float, inline table → `ManifestParse { line, msg }`

## v1.0.11 strict validation

다음 모두 친화 에러로 거부:

```
[project]
name = "g"
version = "0.1.0"
typo_field = "x"     # ✗ UnknownField { section: "project", field: "typo_field" }

[build]
windows_title = "Foo" # ✗ UnknownField { suggestion: Some("window_title") } — Levenshtein 1

[deploy]              # ✗ UnknownSection { name: "deploy", valid_prefixes: [...] }
url = "..."
```

Path validation (v1.0.11):

```toml
[build]
scene = "missing.vscn"   # ✗ ManifestPathMissing { field: "build.scene", path: ... }
```

`voltex build --no-validate-paths` 로 path validation 스킵 가능 (build-time path 생성 시나리오용).

## `voltex build` 출력 (`dist/`)

`voltex build` 성공 시 `dist/` 에 생성되는 것:

| 산출물 | 출처 |
|---|---|
| `<name>.exe` (또는 `<name>`) | 빌드된 self-contained 게임 exe |
| `<scene>.vscn` | manifest 의 `build.scene` 사본 |
| `scripts/`, `assets/` | manifest glob 의 결과 |
| `voltex_archive.vpkg` | scene + scripts + assets 통합 archive (exe 가 `include_bytes!` 로 임베드) |
| `THIRD-PARTY-LICENSES.md` | engine 의 의존성 라이선스 (v1.0.12) |
| `LICENSE-MIT`, `LICENSE-APACHE` | engine 라이선스 (v1.0.12) |

## CLI 와의 매핑

| CLI 옵션 | 매니페스트 영향 |
|---|---|
| `voltex build --target X` | `build.targets` 에 X 가 있어야 함 + X == host_triple |
| `voltex build --manifest-path P` | P 디렉토리의 `voltex.toml` 사용 |
| `voltex build --engine-path R` | 매니페스트 무관 (engine 위치 지정) |
| `voltex build --no-validate-paths` | `Manifest::validate_paths` 스킵 |
| `voltex run [...]` | `voltex build [...]` 와 동일 옵션 |

## 관련 문서

- [`CONTRIBUTING.md`](../CONTRIBUTING.md) — 의존성 정책 / 600줄 한도 / 에러 표준
- [`docs/STATUS.md`](STATUS.md) — v1.0.x 패치 상세 (v1.0.11 schema 강화 / v1.0.12 라이선스 번들)
- [`docs/DEFERRED.md`](DEFERRED.md) — 향후 추가 예정 필드 후보
- [`crates/voltex_cli/src/manifest/mod.rs`](../crates/voltex_cli/src/manifest/mod.rs) — 파서 + validate_schema + validate_paths 본체

## 향후 추가 후보 (v1.1+)

- `[build]` 에 `release_profile` (debug/release 선택 — 현재 release 고정)
- `[build]` 에 `optimize_for` (size / speed)
- `[build.<triple>]` 에 `binary_name` (현재 `<project.name>` 고정)
- `[publish]` 섹션 (steam app id, itch.io 슬러그 등)
- `[deps]` 섹션 (user-defined Cargo crate 의존 — 현재는 voltex_runtime 만 자동)
