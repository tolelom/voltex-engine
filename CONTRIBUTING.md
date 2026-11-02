# Contributing to Voltex Engine

이 문서는 Voltex Engine 에 코드를 추가/수정하려는 사람을 위한
가이드. 자세한 작업 현황은 [`docs/dev/STATUS.md`](docs/dev/STATUS.md),
미뤄진 항목은 [`docs/dev/DEFERRED.md`](docs/dev/DEFERRED.md), 자동 진행 로드맵은
[`docs/superpowers/specs/2026-05-21-service-readiness-roadmap.md`](docs/superpowers/specs/2026-05-21-service-readiness-roadmap.md) 참고.

## 개발 환경

- **Rust 1.75+** (stable). `rustup component add clippy` 필수.
- **OS**: Windows / Linux / macOS 모두 빌드 가능 (cross-compile 미지원 —
  각 OS 머신에서 native build).
- **외부 도구 (선택)**: `cargo-llvm-cov` (coverage), `cargo-about`
  (license inventory), `cargo-audit` (vulnerability scan), `cargo-deny`
  (v1.1+).

## 로컬 검증 (CI 가 돌리는 것과 동일)

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test  --workspace --lib
cargo test  -p voltex_cli         # bin-only crate, lib count 에서 빠짐
```

위 4개 명령이 모두 green 이면 PR 머지 가능 상태.

## Git Hooks (옵션)

`pre-commit` hook 이 위 검증을 자동으로 돌려준다. 한 번만 install:

```bash
# Linux / macOS
bash .githooks/install.sh

# Windows PowerShell
.\.githooks\install.ps1
```

설치 후 `git commit` 마다 `cargo clippy` + `cargo test --workspace --lib`
가 자동 실행. 워크스페이스가 큰 만큼 첫 실행은 30~60초 소요, 캐시 후엔
5~10초.

스킵:
- 일회성: `git commit --no-verify`
- 영구: `git config --unset core.hooksPath`

## 코드 규칙

### 외부 의존성 정책

런타임 의존은 3개로 제한: `winit`, `wgpu`, `bytemuck`. 빌드 전용 (proc
macro / FFI) 은 `thiserror`, `cc` 만 허용. 신규 외부 crate 추가는 별도
논의 필요.

**자체 구현 우선**: PNG, JPG, glTF, WAV, OGG/Vorbis, TTF, JSON, TOML,
Lua FFI, ALSA / WASAPI / CoreAudio 모두 외부 crate 없이 직접 구현.
README "의존성 정책" 섹션 참고.

### 파일 크기

**600줄 한도**. v1.0.2 에서 4 파일 (session/engine_api/world/manifest)
재분리한 전례. 600줄 초과 시 책임 분리 후 sub-module 로 분리.

### 에러 처리

각 crate 는 `<Crate>Error` enum + `Result<T>` alias 사용 (thiserror).
`Generic(String)` variant + `From<String>` + `From<&str>` 자동 변환 +
`#[from] std::io::Error`. 표준 적용 crate: asset, net, ecs, audio, ai,
script, renderer, editor, runtime, cli.

### Component 등록

직렬화 필요한 컴포넌트는 `voltex_ecs::Component` trait 구현 +
`ComponentRegistry::register::<T>()`. `NAME` 변경 시 기존 씬 파일
호환 깨짐 주의. 자세한 패턴은 `CLAUDE.md` "공통 패턴 인덱스" 참고.

### 테스트 스타일

- 단위 테스트는 동일 파일 `#[cfg(test)] mod tests` 에 위치.
- 통합 테스트는 `tests/` 디렉토리.
- GPU/winit 의존 없는 path 는 무조건 unit-testable 해야 함 (4 crate
  predominantly GPU/winit 가 coverage 80% 미달 — `docs/dev/STATUS.md` 의
  v1.0.6 섹션 참조).
- 외부 dep 추가 없이 fuzz: inline xorshift32 (v1.0.8 패턴 참고).

### Commit / PR

- 단일 책임 단위로 commit. 모듈 분리 + 기능 변경은 별 commit.
- 메시지 형식 (v1.0.x 패턴):
  ```
  <type>(<version>): <짧은 요약>

  <상세 본문 — bullet list>
  ```
  `<type>`: `feat` (신규 기능), `fix` (버그/회귀), `docs` (문서), `ci`
  (CI/build infra), `refactor`, `test`.
- STATUS.md / CHANGELOG.md / DEFERRED.md 동시 업데이트가 표준.
- `git push --force` 와 hooks 우회 (`--no-verify`) 는 사전 합의 없으면
  사용 금지.

## 자동 루프 정책

이 repo 는 자동 루프가 v1.0.x 패치를 master 에 푸시할 수 있다 (예:
v1.0.1 ~ v1.0.13 패치 series). 작업 시작 전 항상 `git pull --ff-only`
필수.

## 첫 PR 체크리스트

- [ ] `cargo build --workspace` 통과
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 통과
- [ ] `cargo test --workspace --lib` 통과
- [ ] STATUS.md / CHANGELOG.md / DEFERRED.md 갱신 (해당 시)
- [ ] CODEMAP.md 갱신 (공개 API 변경 시 — 각 crate 루트)
- [ ] 600줄 한도 확인
- [ ] 새 외부 dep 추가 시 사전 논의

## 라이선스

기여한 코드는 dual-license (MIT OR Apache-2.0) 로 등록된다. `LICENSE-MIT`
+ `LICENSE-APACHE` 참고.
