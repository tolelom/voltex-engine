# M4: 에러 타입 표준화 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 모든 crate가 `thiserror` 기반의 통일된 `<Crate>Error` 타입을 사용한다. `Result<T, String>` 패턴 제거.

**Architecture:** 각 crate에 `error.rs` 추가. `<Crate>Error` enum (thiserror::Error 파생). Generic 변형으로 기존 String 메시지를 무손실 마이그레이션, `#[from]` 자동 변환으로 crate 간 에러 전파.

**Tech Stack:** Rust, `thiserror` 2.0. 검증은 `cargo build/test` + `grep` 카운트.

**Spec reference:** `docs/superpowers/specs/2026-05-20-engine-restructure-design.md` Layer 3, 성공기준 #3.

---

## Audit 결과 (2026-05-20)

`Result<T, String>` 패턴: **125건, 8 crate**

| Crate | 건수 | 의존성 레벨 |
|---|---|---|
| voltex_asset | 4 | L0 |
| voltex_net | 11 | L0 |
| voltex_audio | 30 | L1 |
| voltex_ecs | 16 | L1 |
| voltex_ai | 8 | L2 |
| voltex_script | 8 | L2 |
| voltex_renderer | 46 | L2 |
| voltex_editor | 2 | L3 |

**String 없음 (skip)**: voltex_math (L0), voltex_platform (L1), voltex_physics (L2)

**기존 자체 에러**: voltex_editor의 `CommandError` (수동 impl, thiserror 미사용) — M4에서 `EditorError`로 통합.

---

## 마이그레이션 순서 (의존성 in → out)

```
1. voltex_asset (L0)   ← 외부 호출자 없음
2. voltex_net    (L0)   ← 외부 호출자 없음
3. voltex_ecs    (L1)   ← ai, physics, script, editor 호출
4. voltex_audio  (L1)   ← (대부분 자체 사용)
5. voltex_ai     (L2)
6. voltex_script (L2)
7. voltex_renderer (L2) ← editor 호출
8. voltex_editor (L3)   ← 최상위
```

각 crate를 task 1개로 처리. 한 crate 끝나면 빌드/테스트 통과 후 commit.

---

## 공통 패턴: 모든 crate에 동일한 구조

### Cargo.toml 변경

워크스페이스 `Cargo.toml`의 `[workspace.dependencies]`에 thiserror 추가:

```toml
thiserror = "2.0"
```

각 crate의 `Cargo.toml`에:

```toml
[dependencies]
thiserror = { workspace = true }
# 기존 deps...
```

### error.rs 템플릿

```rust
// crates/voltex_<crate>/src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum <Crate>Error {
    /// String 메시지를 그대로 보존하는 generic variant (마이그레이션 호환).
    #[error("{0}")]
    Generic(String),

    /// IO 에러 (필요한 경우만).
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<String> for <Crate>Error {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for <Crate>Error {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}

/// Type alias for `Result<T, <Crate>Error>`.
pub type Result<T> = std::result::Result<T, <Crate>Error>;
```

### lib.rs 변경

```rust
pub mod error;
pub use error::{<Crate>Error, Result};
```

### 함수 시그니처 변경

```rust
// Before
pub fn parse(...) -> Result<T, String> { ... }
pub fn parse(...) -> std::result::Result<T, String> { ... }

// After (선택지 1: type alias 사용)
pub fn parse(...) -> Result<T> { ... }

// After (선택지 2: explicit)
pub fn parse(...) -> std::result::Result<T, <Crate>Error> { ... }
```

권장: **선택지 1 (type alias)**. crate 내부 함수는 type alias 사용. 단, 외부에서 `voltex_<crate>::Result`로 사용하려면 alias가 pub해야 함.

### Err 변경

```rust
// Before
return Err("decode failed".to_string());
return Err(format!("got {}", x));

// After (option A: 명시적)
return Err(<Crate>Error::Generic("decode failed".to_string()));
return Err(<Crate>Error::Generic(format!("got {}", x)));

// After (option B: .into() — From<String>/From<&str> 통해)
return Err("decode failed".into());
return Err(format!("got {}", x).into());
```

권장: **option B**. 더 짧고, `From` impl 덕에 자동 변환.

### `?` 연산자 호환성

`Result<T, String>` 함수를 호출할 때 `?`로 전파하면 String이 호출자의 에러 타입으로 변환되어야 함. `From<String> for <Crate>Error` impl 덕에 자동 됨.

마이그레이션 중 한 crate만 에러 타입을 바꾸면, 그 crate 안의 호출자는 무수정 동작 (`?`가 String→<Crate>Error 자동 변환).

### 다른 voltex crate 호출 시

다른 voltex crate의 함수가 `<OtherCrate>Error`를 반환하면, 자기 crate에서 받기 위해 `From<<OtherCrate>Error>` impl이 필요. M4에서는 일단 **각 crate가 독립적으로 자기 에러만 만들고**, 다른 crate 에러를 `From`으로 받는 건 필요할 때만 추가. 만약 마이그레이션 중 빌드 실패하면 그 시점에 추가.

가장 흔한 경우: `voltex_ecs::Error` → `voltex_renderer::Error` 등. 일단 String으로 받아도 됨 (`Err(e.to_string().into())`).

---

## Task 0: 워크스페이스 thiserror 추가

- [ ] **Step 1: 워크스페이스 Cargo.toml에 thiserror 추가**

`Cargo.toml`의 `[workspace.dependencies]` 끝에 추가:

```toml
thiserror = "2.0"
```

- [ ] **Step 2: 빌드 검증**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --workspace 2>&1 | tail -3
```

Expected: 빌드 성공 (의존성 추가만이라 변경 없음).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "build: add thiserror 2.0 to workspace dependencies"
```

---

## Task 1: voltex_asset 에러 표준화

`Result<T, String>` 건수: 4

- [ ] **Step 1: Cargo.toml에 thiserror 추가**

`crates/voltex_asset/Cargo.toml`의 `[dependencies]`에:
```toml
thiserror = { workspace = true }
```

- [ ] **Step 2: error.rs 생성**

`crates/voltex_asset/src/error.rs`:
```rust
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("{0}")]
    Generic(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<String> for AssetError {
    fn from(s: String) -> Self { Self::Generic(s) }
}

impl From<&str> for AssetError {
    fn from(s: &str) -> Self { Self::Generic(s.to_string()) }
}

pub type Result<T> = std::result::Result<T, AssetError>;
```

- [ ] **Step 3: lib.rs 업데이트**

`crates/voltex_asset/src/lib.rs`에 추가:
```rust
pub mod error;
pub use error::{AssetError, Result};
```

- [ ] **Step 4: `Result<T, String>` 마이그레이션**

`crates/voltex_asset/src/**/*.rs`의 모든 `Result<T, String>`을 `Result<T>` 또는 `Result<T, AssetError>`로 변경. 4건이므로 grep으로 위치 찾기:

```bash
grep -rn "Result<.*, String>" crates/voltex_asset/src --include="*.rs"
```

각 위치에서 시그니처 변경. `Err("...")`/`Err("...".to_string())`은 `Err("...".into())`로 변경.

- [ ] **Step 5: 빌드 + 테스트 검증**

```bash
cargo build -p voltex_asset 2>&1 | tail -5
cargo test -p voltex_asset --lib 2>&1 | grep "test result"
grep -rn "Result<.*, String>" crates/voltex_asset/src --include="*.rs" | wc -l
```

Expected:
- 빌드 성공
- 22개 테스트 통과
- `Result<T, String>` 카운트 0

- [ ] **Step 6: Commit**

```bash
git add crates/voltex_asset/
git commit -m "refactor(asset): introduce AssetError (thiserror) and remove Result<T, String>"
```

---

## Task 2: voltex_net 에러 표준화

`Result<T, String>` 건수: 11

Task 1과 동일한 패턴. crate name `voltex_net`, error name `NetError`, alias `Result<T>`.

- [ ] **Step 1**: `Cargo.toml` thiserror 추가
- [ ] **Step 2**: `crates/voltex_net/src/error.rs` 생성 (`NetError` enum + `From<String>` impl + `Result<T>` alias)
- [ ] **Step 3**: `crates/voltex_net/src/lib.rs`에 `pub mod error; pub use error::{NetError, Result};`
- [ ] **Step 4**: 11건 마이그레이션 (grep으로 위치 확인 후 시그니처 + Err 변경)
- [ ] **Step 5**: 빌드 + 테스트 (47개 통과)
- [ ] **Step 6**: Commit (`refactor(net): introduce NetError (thiserror) and remove Result<T, String>`)

---

## Task 3: voltex_ecs 에러 표준화

`Result<T, String>` 건수: 16

Task 1 패턴. `EcsError`, `Result<T>`.

- [ ] **Step 1-3**: thiserror 추가, `error.rs` 생성, lib.rs 갱신
- [ ] **Step 4**: 16건 마이그레이션. 주로 scene/json/binary_scene 직렬화. **변경 사항**: lib.rs에서 `pub use error::EcsError;`를 추가하므로, 기존 `Result` alias가 이미 std::result::Result로 fully-qualified된 곳은 영향 없음.
- [ ] **Step 5**: 빌드 + 테스트 (104개 통과)
- [ ] **Step 6**: Commit (`refactor(ecs): introduce EcsError (thiserror) and remove Result<T, String>`)

---

## Task 4: voltex_audio 에러 표준화

`Result<T, String>` 건수: 30 (vorbis 대부분)

Task 1 패턴. `AudioError`, `Result<T>`.

- [ ] **Step 1-3**: thiserror 추가, `error.rs` 생성, lib.rs 갱신
- [ ] **Step 4**: 30건 마이그레이션. vorbis sub-modules (bit_reader, codebook, floor, residue, headers, decoder)의 시그니처가 영향. 각 sub-module에서 `use super::super::error::{AudioError, Result};` 또는 `use crate::Result;` 사용.
- [ ] **Step 5**: 빌드 + 테스트 (84개 통과)
- [ ] **Step 6**: Commit (`refactor(audio): introduce AudioError (thiserror) and remove Result<T, String>`)

---

## Task 5: voltex_ai 에러 표준화

`Result<T, String>` 건수: 8

Task 1 패턴. `AiError`, `Result<T>`.

- [ ] **Step 1-3**: thiserror 추가, `error.rs` 생성, lib.rs 갱신
- [ ] **Step 4**: 8건 마이그레이션
- [ ] **Step 5**: 빌드 + 테스트 (34개 통과)
- [ ] **Step 6**: Commit (`refactor(ai): introduce AiError (thiserror) and remove Result<T, String>`)

---

## Task 6: voltex_script 에러 표준화

`Result<T, String>` 건수: 8

Task 1 패턴. `ScriptError`, `Result<T>`.

- [ ] **Step 1-3**: thiserror 추가, `error.rs` 생성, lib.rs 갱신
- [ ] **Step 4**: 8건 마이그레이션
- [ ] **Step 5**: 빌드 + 테스트 (18개 통과)
- [ ] **Step 6**: Commit (`refactor(script): introduce ScriptError (thiserror) and remove Result<T, String>`)

---

## Task 7: voltex_renderer 에러 표준화

`Result<T, String>` 건수: 46 (가장 많음)

Task 1 패턴. `RendererError`, `Result<T>`. 영향 위치: gltf/, jpg/, png, deflate, gpu, texture 등.

- [ ] **Step 1-3**: thiserror 추가, `error.rs` 생성, lib.rs 갱신
- [ ] **Step 4**: 46건 마이그레이션. 큰 작업이지만 mechanical. 위치별로 grep으로 확인 후 점진적.
- [ ] **Step 5**: 빌드 + 테스트 (200개 통과)
- [ ] **Step 6**: Commit (`refactor(renderer): introduce RendererError (thiserror) and remove Result<T, String>`)

---

## Task 8: voltex_editor 에러 표준화 (CommandError 통합)

`Result<T, String>` 건수: 2. 기존 `CommandError` 통합.

`crates/voltex_editor/src/error.rs`:
```rust
use voltex_ecs::Entity;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EditorError {
    #[error("{0}")]
    Generic(String),

    #[error("entity not alive: id={} gen={}", .0.id, .0.generation)]
    EntityNotAlive(Entity),

    #[error("io: {0}")]
    Io(String),

    #[error("parse: {0}")]
    Parse(String),
}

impl From<String> for EditorError {
    fn from(s: String) -> Self { Self::Generic(s) }
}

impl From<&str> for EditorError {
    fn from(s: &str) -> Self { Self::Generic(s.to_string()) }
}

pub type Result<T> = std::result::Result<T, EditorError>;
```

기존 `CommandError`를 `EditorError`로 대체. `command.rs`의 `pub enum CommandError`와 그 `impl Display`/`impl Error`를 제거하고, 함수 시그니처를 `Result<Entity, EditorError>` 등으로 변경. 외부 사용자(examples/editor_demo)에서 `CommandError` 참조하면 `EditorError`로 교체.

- [ ] **Step 1-3**: thiserror 추가, `error.rs` 생성, lib.rs 갱신
- [ ] **Step 4**: 기존 `CommandError` 제거 + 호출자 업데이트 + 2건 String 마이그레이션
- [ ] **Step 5**: 빌드 + 테스트 (105개 통과). examples도 빌드 (`cargo build --examples`)
- [ ] **Step 6**: Commit (`refactor(editor): introduce EditorError (thiserror), absorb CommandError`)

---

## Task 9: 최종 검증 + CODEMAP 갱신 + push

- [ ] **Step 1: 전체 `Result<T, String>` 카운트 0 확인**

```bash
grep -rn "Result<.*, String>" crates/*/src --include="*.rs" | wc -l
```

Expected: 0 (또는 매우 적음 — examples는 무관).

- [ ] **Step 2: 각 crate에 `<Crate>Error` 존재 확인**

```bash
for c in math platform asset net audio ecs ai physics script renderer editor; do
  f="crates/voltex_$c/src/error.rs"
  if [ -f "$f" ]; then
    echo "OK $f"
  else
    echo "(missing: $f — Result<T,String>가 0건이라 skip 가능)"
  fi
done
```

Expected:
- voltex_asset, voltex_net, voltex_ecs, voltex_audio, voltex_ai, voltex_script, voltex_renderer, voltex_editor: error.rs 존재
- voltex_math, voltex_platform, voltex_physics: error.rs 없음 (skip — Result<T,String> 미사용)

- [ ] **Step 3: 전체 빌드/테스트**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --workspace 2>&1 | tail -3
cargo build --examples 2>&1 | tail -3
cargo test --workspace --lib 2>&1 | grep -E "^test result" | awk '{sum += $4} END {print sum}'
```

Expected:
- 빌드 성공 (workspace + examples)
- 760 테스트 통과

- [ ] **Step 4: CODEMAP 갱신**

각 영향받은 crate의 CODEMAP "공통 패턴 인덱스" 또는 "변경 시 주의점"에 다음 추가:
- `<Crate>Error`가 단일 에러 타입이며 `Result<T>` alias 제공
- `pub use error::{<Crate>Error, Result};`로 노출

루트 `CLAUDE.md`의 "공통 패턴 인덱스" → "에러 처리" 섹션 갱신:
```markdown
### 에러 처리
- 모든 crate가 `voltex_<crate>::<Crate>Error` + `Result<T>` alias 사용 (thiserror 기반)
- crate 간 에러 전파: `#[from]` 또는 `.into()` (Generic variant로 자동 변환)
- 외부 String 메시지는 `Generic(String)` variant로 흡수
```

- [ ] **Step 5: CODEMAP commit**

```bash
git add CLAUDE.md crates/*/CODEMAP.md
git commit -m "docs(codemap): document error standardization (M4)"
```

- [ ] **Step 6: Push**

```bash
git log --oneline -15
git push origin master
```

---

## 완료 기준 (Definition of Done)

- [ ] 워크스페이스에 `thiserror = "2.0"` 추가
- [ ] 8개 crate에 `error.rs` 추가 + `<Crate>Error` enum 정의
- [ ] crate 전체에서 `Result<T, String>` 카운트 0 (또는 매우 적음 — examples는 무관)
- [ ] voltex_editor의 기존 `CommandError`가 `EditorError`로 통합
- [ ] `cargo build --workspace` 성공
- [ ] `cargo build --examples` 성공
- [ ] `cargo test --workspace --lib` 760개 통과
- [ ] CODEMAP/CLAUDE.md 갱신
- [ ] Push 완료

## 리스크 및 대응

- **`Err("...")` 변환 누락**: 일부 `Err("...")` 구문에서 type 추론 실패 → 컴파일러가 에러. `.into()` 추가.
- **crate 간 의존성**: A crate가 B crate의 함수를 호출하고 그 함수가 `BError`를 반환하면, A의 함수가 `?`로 전파 시 `From<BError> for AError`가 필요. M4 마이그레이션 중 빌드 실패 시 해당 `From` impl 추가.
- **CommandError 호환성 깨짐**: `voltex_editor::CommandError`를 명시적으로 import한 코드가 깨질 수 있음 (예: examples/editor_demo). 점검 후 `EditorError`로 교체.
- **테스트 코드 영향**: 테스트에서 `Err("...")` 패턴 사용 시 시그니처 변경에 영향 → 동일하게 `.into()` 적용.

## 다음 마일스톤

M5: ECS 컴포넌트/시스템 등록 패턴 표준화.
