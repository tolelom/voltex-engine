# Voltex Build Matrix

Cycle 11 (B3) 의 산출물. `voltex build` 는 **native build only** — cross-compile 하지 않는다. 각 OS 의 산출물은 그 OS 의 머신에서 직접 빌드해야 한다.

## 사전 요구

| OS | Rust toolchain | 추가 의존 | 비고 |
|---|---|---|---|
| Windows 10 / 11 x64 | `stable-x86_64-pc-windows-msvc` | Visual Studio 2022 Build Tools (MSVC linker) | `link.exe` 가 PATH 에 있어야 함 |
| Ubuntu 22.04+ / Debian / Arch x64 | `stable-x86_64-unknown-linux-gnu` | `libasound2-dev` (Debian/Ubuntu) 또는 `alsa-lib` (Arch) | ALSA dev headers 필요 |
| macOS 13+ arm64 | `stable-aarch64-apple-darwin` | Xcode Command Line Tools (`xcode-select --install`) | AudioToolbox.framework 는 OS 표준 — 추가 설치 불필요 |

Intel Mac (`macos-x64`) 도 동일한 절차로 빌드 가능하나 1차 검증 대상은 arm64.

## 빌드 명령

각 OS 의 native 머신에서 직접 실행:

```bash
# Windows (PowerShell 또는 bash)
voltex build --target windows-x64

# Linux
voltex build --target linux-x64

# macOS (Apple Silicon)
voltex build --target macos-arm64
```

`--target` 을 생략하면 host triple 을 자동 선택한다. manifest 의 `targets` 배열이 host 를 포함하지 않으면 친화 에러를 출력하고 종료한다.

### 산출물 위치

```
<project>/dist/
├── <project_name>(.exe)         # 단일 실행 파일 (Vfs 로 자원 임베드, B2)
├── scripts/                     # 폴더 fallback 자원 (개발 모드 호환)
└── assets/
```

## voltex.toml multi-target 예시

```toml
[project]
name = "my_game"
version = "0.1.0"

[build]
scene = "scene.vscn"
scripts = ["scripts/**/*.lua"]
assets = ["assets/**/*"]
targets = ["windows-x64", "linux-x64", "macos-arm64"]

[build.windows-x64]
icon = "assets/icon.ico"

[build.macos-arm64]
icon = "assets/icon.icns"
```

`targets` 배열에 3개 OS 를 모두 적어두면, 각 OS 의 머신에서 동일한 `voltex build` 가 자기 OS 의 산출물만 생산한다.

> **참고**: `[build.<target>] icon = ...` 필드는 매니페스트 파서가 이미 읽어 들이지만, 실제 icon 임베드는 v1.1 후속 작업이다 (B3 spec § 3.2.3).

## 친화 에러 메시지 (B3)

`voltex_cli::build::validate_target` 이 cross-compile 시도와 매니페스트 불일치를 다음과 같이 알린다:

```
$ voltex build --target macos-arm64   # on Windows host
voltex: cross-compile not supported.
  current host: windows-x64
  requested target: macos-arm64
  hint: run `voltex build --target macos-arm64` on a macos-arm64 machine.
```

```
$ voltex build                        # manifest targets=["windows-x64"] on Linux
voltex: manifest targets do not include host triple.
  current host: linux-x64
  manifest targets = ["windows-x64"]
  hint: add "linux-x64" to the targets array, or pass --target linux-x64 explicitly.
```

## GitHub Actions 3-OS matrix (옵션 참고)

본 cycle 의 정식 done 기준에는 CI 통합이 포함되지 않으나, 사용자가 release pipeline 을 구성할 때 참고할 스니펫:

```yaml
name: voltex-build-matrix
on: { push: { tags: ["v*"] } }

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: windows-latest
            triple: windows-x64
          - os: ubuntu-latest
            triple: linux-x64
            apt: libasound2-dev
          - os: macos-latest
            triple: macos-arm64
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - if: matrix.apt
        run: sudo apt-get install -y ${{ matrix.apt }}
      - run: cargo install --path crates/voltex_cli
      - run: voltex build --target ${{ matrix.triple }}
      - uses: actions/upload-artifact@v4
        with:
          name: my_game-${{ matrix.triple }}
          path: dist/
```

## 검증 한계 (현재 시점)

- **Linux ALSA backend** — Cycle 10 (P1) 에서 작성됨. Windows dev 머신에서 작성되어 실제 Linux 머신에서의 `cargo build --target x86_64-unknown-linux-gnu` + `audio_demo` 소리 출력 검증은 후속.
- **macOS CoreAudio backend** — Cycle 11 (B3) 에서 작성됨. 동일하게 Windows dev 머신에서 작성. 실제 Mac 머신에서의 `cargo build --target aarch64-apple-darwin` + `audio_demo` 소리 출력 검증은 후속.
- Windows 환경에서는 `cargo check -p voltex_audio --target aarch64-apple-darwin` 으로 type-check 정상성 확인.

각 OS 의 실제 런타임 검증은 사용자가 해당 머신에 접근 가능할 때 별도 작업으로 진행.
