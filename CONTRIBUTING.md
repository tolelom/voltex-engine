# 기여 안내

> Voltex Engine 코드를 고칠 때 지키는 규칙과 커밋 전에 돌리는 검사.

## 개발 환경

- Rust stable과 clippy(`rustup component add clippy`)
- Linux는 ALSA 헤더: `sudo apt-get install -y libasound2-dev`
- 선택 도구: cargo-llvm-cov(커버리지), cargo-about(의존 crate 라이선스 고지 생성, `cargo install --locked cargo-about --features cli`), cargo-deny(취약점·라이선스 검사, 설정은 `deny.toml`)
- 파서 퍼징: [fuzz/README.md](fuzz/README.md)

## 커밋 전 검사

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test -p voltex_cli
```

voltex_cli는 바이너리 crate라 `--lib`에 잡히지 않아 따로 돌린다.

GPU가 필요한 테스트는 `#[ignore]`로 빠져 있다. GPU가 있는 기기에서 `cargo test --workspace --tests -- --ignored`로 돌린다.

### git 훅

```bash
bash .githooks/install.sh      # Linux·macOS
.\.githooks\install.ps1        # Windows PowerShell
```

- 커밋할 때: clippy와 `cargo test --workspace --lib`
- push할 때: engine/, tools/, examples/, games/를 바꿨는데 CHANGELOG.md를 고치지 않았으면 막는다. aarch64 Linux 대상 clippy도 돈다(타깃이 없으면 건너뜀).

한 번 건너뛸 때는 `--no-verify`.

## 코드 규칙

### 의존성

엔진 외부 crate는 wgpu·winit(GPU와 창)과 작은 보조 crate(bytemuck, thiserror, log, pollster)만 쓴다. Lua는 C 소스를 저장소에 넣어 `cc`로 빌드하고, env_logger는 예제와 게임에서만 쓴다. 나머지는 직접 구현한다. 새 외부 crate는 추가하기 전에 먼저 논의한다.

의존 crate를 바꾸면 `scripts/gen_third_party_licenses.sh`로 `THIRD-PARTY-LICENSES.md`를 다시 만든다. `voltex build`가 이 파일을 게임 `dist/`에 같이 넣는다.

### 파일 크기

한 파일은 600줄 이하로 둔다. 넘으면 책임별로 하위 모듈로 나눈다.

### 에러 처리

crate마다 `<Crate>Error` enum과 `Result<T>` alias를 둔다(thiserror). 예외 crate와 변환 패턴은 [CLAUDE.md](CLAUDE.md)의 "공통 패턴 인덱스".

### 컴포넌트

직렬화할 컴포넌트는 `voltex_ecs::Component`를 구현하고 `ComponentRegistry::register::<T>()`로 등록한다. `NAME`을 바꾸면 기존 씬 파일을 읽지 못한다.

### 테스트

- 단위 테스트는 모듈 안 `#[cfg(test)] mod tests`. 길어지면 같은 폴더의 `tests.rs`로 뗀다.
- 통합 테스트는 crate의 `tests/`.

### 커밋

- 한 커밋에 한 가지 변경. 모듈 분리와 기능 변경은 나눈다.
- 메시지는 `<type>(<범위>): <요약>`. type은 feat, fix, refactor, test, docs, build, chore, ci. 범위는 crate나 게임 이름(`fix(physics)`, `feat(leap)`).
- 코드를 바꾸면 CHANGELOG.md의 Unreleased 절에 적는다. 공개 API가 바뀌면 그 crate의 `CODEMAP.md`도 고친다.

## 라이선스

보낸 코드는 이 저장소와 같은 조건(MIT 또는 Apache-2.0 중 선택)으로 배포된다.
