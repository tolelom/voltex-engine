# voltex.toml

> Voltex 프로젝트의 매니페스트. `voltex new`가 만들고 `voltex build`와 `voltex run`이 읽는다. 모르는 키나 섹션이 있으면 비슷한 이름을 제안하며 에러로 멈춘다.

## 예시

`[project]`만 있어도 빌드된다. `[build]`의 필드는 모두 기본값이 있다.

```toml
[project]
name = "my_game"      # 영문, 숫자, 밑줄
version = "0.1.0"

[build]
scene = "level1.vscn"
scripts = ["scripts/**/*.lua"]
assets = ["assets/**/*"]
window_title = "My Game"
window_size = [1920, 1080]
targets = ["windows-x64", "linux-x64"]

[build.windows-x64]
icon = "assets/icon.ico"
```

## `[project]` (필수)

| 키 | 타입 | 설명 |
|---|---|---|
| `name` | string | 실행 파일 이름(`dist/<name>.exe`) |
| `version` | string | 프로젝트 버전. 빌드에는 쓰이지 않는다 |

## `[build]` (선택)

| 키 | 타입 | 기본값 | 설명 |
|---|---|---|---|
| `kind` | string | `"scene"` | `"scene"`: 씬과 Lua로 만든 게임. 실행 파일에 씬·스크립트·에셋을 묶는다. `"rust"`: 게임 로직이 Rust인 cargo 패키지. 아래 절 참고 |
| `scene` | string | `"scene.vscn"` | 시작 씬. `voltex.toml` 기준 상대 경로이고, 빌드 전에 있는지 확인한다 |
| `scripts` | string 배열 | `["scripts/**/*.lua"]` | Lua 스크립트 glob. glob 앞부분의 폴더가 있어야 한다 |
| `assets` | string 배열 | `["assets/**/*"]` | 텍스처, 메시, 소리 glob. 규칙은 `scripts`와 같다 |
| `window_title` | string | `name` 값 | 창 제목. 크래시 로그 파일 이름 앞에도 붙는다 |
| `window_size` | int 배열 | `[1280, 720]` | `[너비, 높이]`. 둘 다 양의 정수 |
| `targets` | string 배열 | 지금 OS | 빌드를 허용할 OS. [build-matrix.md](build-matrix.md) |

### `kind = "rust"`일 때

- `voltex.toml` 옆에 `Cargo.toml`이 있어야 한다. 그 패키지를 `cargo build --release`로 빌드한다.
- 실행 파일은 `[project] name`과 이름이 같은 바이너리를 고른다. 같은 이름이 없으면 바이너리가 하나일 때만 그것을 쓰고, 둘 이상이면 에러다.
- `scene`이 없어도 된다. `scripts`, `assets`의 기본값은 빈 배열이라 배포할 에셋은 적어 줘야 한다.
- 에셋은 실행 파일 안에 묶지 않고 `dist/`에 원래 상대 경로대로 복사한다. 게임은 `RuntimeConfig::with_asset_root("assets")`로 읽는다.

## `[build.<대상>]` (선택)

대상별 설정. 대상 이름은 `windows-x64`, `linux-x64`, `macos-x64`, `macos-arm64`이고, 그 밖의 이름도 받는다.

| 키 | 타입 | 설명 |
|---|---|---|
| `icon` | string | 아이콘 파일 경로. 읽어서 보관만 하고 실행 파일에 넣지는 않는다 |

## 문법

TOML의 일부만 읽는다.

- 주석 `#`, 빈 줄
- 헤더 `[section]`, `[section.sub]`(한 단계까지)
- 값: 문자열, 정수, `true`·`false`, 문자열·정수 배열
- 문자열 이스케이프: `\"`, `\\`, `\n`, `\t`

여러 줄 문자열, 점 표기 키, 날짜, 실수, 인라인 테이블은 지원하지 않고, 쓰면 줄 번호와 함께 에러가 난다.

## 검증

```toml
[project]
name = "g"
version = "0.1.0"
typo_field = "x"        # 에러: 모르는 필드

[build]
windows_title = "Foo"   # 에러: 모르는 필드, window_title 제안
scene = "missing.vscn"  # 에러: 파일 없음

[deploy]                # 에러: 모르는 섹션
```

빌드 중에 파일을 만드는 경우처럼 경로 확인을 건너뛰어야 하면 `voltex build --no-validate-paths`.

## `dist/` 출력

| 파일 | 내용 |
|---|---|
| `<name>.exe`(Windows) 또는 `<name>` | 게임 실행 파일. `kind = "scene"`이면 씬·스크립트·에셋이 들어 있다 |
| 시작 씬, `scripts/`, `assets/` | 매니페스트 glob 결과를 상대 경로대로 복사 |
| `THIRD-PARTY-LICENSES.md`, `LICENSE-MIT`, `LICENSE-APACHE` | 엔진과 의존 crate의 라이선스 |
| `wrapper/` | `kind = "scene"` 빌드의 중간 산출물(생성한 crate와 `.vpkg` 묶음) |

glob이 프로젝트 루트 전체를 잡아도 `dist/`와 `target/`은 복사하지 않는다.

## CLI 옵션

| 옵션 | 뜻 |
|---|---|
| `--target X` | 빌드 대상. `targets`에 있고 지금 OS여야 한다 |
| `--manifest-path P` | `P` 폴더의 `voltex.toml`을 쓴다 |
| `--engine-path R` | 엔진 저장소 위치. 환경 변수 `VOLTEX_REPO`로도 줄 수 있다 |
| `--no-validate-paths` | 경로 확인 건너뜀 |

`voltex run`은 `voltex build`와 같은 옵션을 받고, 빌드한 뒤 실행한다.

파서 구현: [tools/voltex_cli/src/manifest/mod.rs](../tools/voltex_cli/src/manifest/mod.rs).
