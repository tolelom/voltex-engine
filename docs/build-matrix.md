# OS별 빌드

> `voltex build`는 지금 쓰는 OS용 실행 파일만 만든다. 다른 OS용은 그 OS에서 빌드한다.

## 준비물

| OS | Rust 툴체인 | 추가로 필요한 것 |
|---|---|---|
| Windows 10·11 x64 | `stable-x86_64-pc-windows-msvc` | Visual Studio 2022 Build Tools(`link.exe`가 PATH에 있어야 함) |
| Linux x64 | `stable-x86_64-unknown-linux-gnu` | ALSA 헤더: Debian·Ubuntu `libasound2-dev`, Arch `alsa-lib` |
| macOS arm64 | `stable-aarch64-apple-darwin` | Xcode Command Line Tools(`xcode-select --install`) |

Intel Mac은 대상 이름이 `macos-x64`다.

## 빌드

```bash
voltex build                        # 지금 OS 대상
voltex build --target windows-x64   # 대상을 적을 때. linux-x64, macos-arm64, macos-x64
```

`voltex.toml`의 `targets`에 지금 OS가 없거나, 다른 OS를 `--target`으로 적으면 빌드하지 않고 멈춘다.

```
voltex: cross-compile not supported.
  current host: windows-x64
  requested target: macos-arm64
  hint: run `voltex build --target macos-arm64` on a macos-arm64 machine.
```

```
voltex: manifest targets do not include host triple.
  current host: linux-x64
  manifest targets = ["windows-x64"]
  hint: add "linux-x64" to the targets array, or pass --target linux-x64 explicitly.
```

여러 OS에 배포할 게임은 `targets`에 모두 적어 두고 OS마다 같은 `voltex build`를 돌린다. `dist/`에 나오는 파일과 매니페스트 필드는 [voltex-toml-reference.md](voltex-toml-reference.md).

## 확인 범위

| OS | 확인한 것 |
|---|---|
| Windows | 빌드, 테스트, 예제·게임 실행 |
| Linux | aarch64 대상 교차 clippy. 실제 기기에서 실행과 소리 출력은 확인하지 않음 |
| macOS | Windows에서 voltex_audio 타입 검사(`cargo check --target aarch64-apple-darwin`)만. 빌드와 실행은 확인하지 않음 |
