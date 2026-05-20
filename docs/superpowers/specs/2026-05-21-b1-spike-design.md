# Cycle 7 (B1-spike) — prototype CLI 설계 (1세션 timebox)

**작성일**: 2026-05-21
**상위 로드맵**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 7
**Sub-project**: B
**세션 수**: 1 (timebox 엄격)
**의존**: Cycle 6 (B0 — voltex_runtime) ✅

---

## 1. 목적

B1 의 미지수 (CLI 골격, 프로젝트 디렉토리 레이아웃, cargo wrapper invocation, exe 산출 메커니즘) 를 prototype 한 번 통과시켜본다. 산출은 dirty 해도 OK, 학습이 자산.

## 2. Scope

- 신규 bin crate `crates/voltex_cli` (binary 이름 = `voltex`)
- 단일 subcommand: `voltex build <project_dir>`
  - 입력 디렉토리 구조 (prototype):
    ```
    <project_dir>/
    ├── scene.vscn         (옵션)
    ├── scripts/*.lua      (옵션, 0개 이상)
    └── assets/**/*        (옵션)
    ```
  - 동작:
    1. project_dir 검증 (디렉토리 존재 여부만)
    2. 임시 wrapper crate 생성 (`<project_dir>/dist/wrapper/`)
        - `Cargo.toml`: voltex_runtime 의존 (path = 상대 경로 또는 workspace 경유)
        - `src/main.rs`: `RuntimeApp::new(RuntimeConfig::new().with_window_title(<name>)).run()` 호출
    3. wrapper 디렉토리에서 `cargo build --release` 호출
    4. 산출 exe 를 `<project_dir>/dist/<name>.exe` 로 복사
    5. scene.vscn / scripts / assets 를 dist 폴더에 단순 복사
  - 출력 경로: `<project_dir>/dist/`

- 학습 노트: `docs/superpowers/notes/2026-05-21-b1-spike-learnings.md` 작성
  - voltex_cli 의 위치/이름 결정
  - cargo wrapper 호출 방식 (어떤 manifest path / target dir 사용했는지)
  - voltex_runtime 을 사용자 프로젝트가 어떻게 의존하는지 (workspace member? path dep?)
  - voltex.toml 매니페스트에 무엇이 들어가야 하는지 (B1 의 입력)
  - 어떤 부분이 어려웠고 어떻게 우회했는지

## 3. Out-of-scope (B1+ 의 일)

- voltex.toml 매니페스트 파싱 (이 cycle 은 인자 = 디렉토리 경로 하나)
- `voltex new` 스캐폴드
- `voltex run`
- asset packing (B2)
- multi OS (B3)
- 사용자 친화 에러 메시지 (prototype 은 panic + stderr 로 OK)
- cargo workspace 자동 등록 (사용자가 직접 path 지정)

## 4. 아키텍처 / 결정

### 4.1 voltex_cli 위치 / 이름
- `crates/voltex_cli/` 디렉토리
- `[[bin]] name = "voltex"` 으로 설정해서 사용자는 `voltex build ...` 로 호출
- bin crate 이름과 패키지 이름이 달라야 함 (facade `voltex` crate 와 충돌 방지) — 패키지명 `voltex_cli`, bin명 `voltex`

### 4.2 wrapper crate 생성 전략
prototype 단계에서 가장 단순한 방법: **임시 wrapper crate** 를 `<project_dir>/dist/wrapper/` 에 작성 후 cargo build. workspace 통합 안 함.

```
<project_dir>/dist/wrapper/
├── Cargo.toml        # voltex_runtime 의존 (path = <게임엔진 절대 경로>)
└── src/main.rs       # 4줄짜리 RuntimeApp 호출
```

문제점:
- voltex_runtime 의 path 를 사용자가 안 알면 작동 안 함 → prototype 단계에선 환경변수 `VOLTEX_REPO` 로 받음
- workspace integration 부재 → 매번 dep tree 재해석

이 두 가지는 학습 노트에 명시.

### 4.3 cargo invocation
```rust
std::process::Command::new("cargo")
    .args(["build", "--release"])
    .current_dir(wrapper_dir)
    .status()?;
```

성공 시 산출물: `<wrapper_dir>/target/release/wrapper.exe` (Windows). 이걸 `<project_dir>/dist/<name>.exe` 로 rename.

### 4.4 프로젝트 이름 결정
prototype 단계: `<project_dir>` 의 마지막 path component 를 프로젝트 이름으로 사용. e.g. `voltex build ./mygame` → name = "mygame".

### 4.5 자원 처리
- `scene.vscn` 존재 시 dist 폴더에 복사
- `scripts/` 폴더 전체를 dist/scripts/ 로 재귀 복사
- `assets/` 폴더 전체를 dist/assets/ 로 재귀 복사
- 실제 runtime 이 어떻게 자원을 찾는지는 v1.1 의 VFS 영역 — prototype 에서는 exe 와 같은 폴더에 풀어둠

### 4.6 wrapper 의 RuntimeApp 코드 (생성된 src/main.rs)
```rust
use voltex_runtime::{RuntimeApp, RuntimeConfig};

fn main() -> voltex_runtime::RuntimeResult<()> {
    RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("<name>"),
    )?
    .run()
}
```

scene/scripts/assets 자동 로딩은 v1.1 — prototype 의 wrapper 는 단순히 윈도우만 띄움 (학습 노트에 명시).

## 5. 모듈 구조

```
crates/voltex_cli/
├── Cargo.toml
└── src/
    └── main.rs       # 모든 로직 한 파일 (~150줄). prototype 정신: 분리 안 함.
```

## 6. 에러 처리
- 모든 에러는 stderr 출력 + `std::process::exit(1)`. anyhow 등 의존성 불필요.
- 사용자 친화화는 B1 (Cycle 8) 의 일.

## 7. 테스트
- 단위 테스트 1~2개 (path 처리 helper 정도). prototype 정신상 cargo build 호출 테스트는 미작성 (CI 안 도는 게 명확).

## 8. Done 기준 (로드맵 § Cycle 7)
- [ ] `crates/voltex_cli` 신규 + bin name "voltex"
- [ ] `voltex build <dir>` 가 wrapper crate 만들고 cargo build 호출 → `<dir>/dist/<name>.exe` 산출
- [ ] 산출된 exe 더블클릭 → 빈 RuntimeApp 윈도우 표시 (회전 큐브는 v1.1+)
- [ ] **학습 노트** 작성: voltex.toml 에 무엇이 들어가야 할지, 어떤 cargo invocation 패턴이 작동했는지
- [ ] workspace `cargo build --workspace` 깨끗
- [ ] roadmap Cycle 7 `[✅ 1/1]`

## 9. 위험 / 미지수
- **cargo invocation 환경**: 사용자 PC 에 cargo 가 PATH 에 있어야 함. 없으면 에러로 fail. prototype 수용.
- **voltex_runtime path 의존**: `VOLTEX_REPO` 환경변수로 받음. 사용자 OS 분리 / cargo install 으로 voltex_runtime 을 system-level 로 두는 것은 B1+.
- **wrapper crate 의 cargo target dir**: 사용자 프로젝트마다 별도 target 생성됨 → 디스크 사용량 큼. B1 에서 공유 target 또는 다른 전략 고려.
- **timebox 초과 위험**: 1세션 안에 안 끝나면 "안 끝났음 + 어디까지 갔는지" 를 학습 노트에 기록하고 B1 spec 에서 정식 해결.
