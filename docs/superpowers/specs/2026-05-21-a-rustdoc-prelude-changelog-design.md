# Cycle 13 (A) — 공개 API 문서화 + facade prelude 완성 + CHANGELOG

**작성일**: 2026-05-21
**범위**: 외부 사용자가 `cargo doc --open` 으로 IDE 없이도 엔진을 이해할 수 있게 한다. v1.0 release 전 breaking change lock 시점.
**선행 조건**: Cycle 1~12 완료 (공개 API 가 굳기 직전).
**로드맵 참조**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 13

---

## 1. 현황 측정

| 항목 | 값 |
|---|---|
| `cargo doc --workspace --no-deps` 워닝 | ~32 (intra-doc link 깨짐 + 파일 충돌 1) |
| `-W missing_docs` 총 누락 | **1586** (voltex_renderer 513, voltex_editor 273, voltex_audio 214, voltex_ecs 132, voltex_script 74, voltex_physics 72, voltex_net 71, voltex_asset 64, voltex_math 59, voltex_runtime 40, voltex_platform 38, voltex_ai 23, voltex_renderer_ecs 13, voltex 0) |

로드맵의 "330 항목" 예측 대비 ~5배. 본 cycle 의 정식 scope 를 합리적으로 재정의한다.

## 2. Scope 재정의

### 2.1 본 cycle 에서 처리 (must-have)

1. **`cargo doc --workspace --no-deps` 워닝 0개** — intra-doc link 깨짐 fix (대부분 `///  - .0,1` 같은 표기를 `///  - `\0\`, \`1\`` 또는 escape). voltex facade ↔ voltex_renderer 파일 충돌 해결.
2. **facade prelude 보강** — C0a/C0b/S1 에서 추가된 컴포넌트가 prelude 에 누락. MeshHandle/MaterialHandle/Bounds/ScriptComponent 추가. examples 의 use 문 90%+ cover 여부 수동 확인.
3. **Tier-1 공개 API rustdoc** — 외부 사용자가 가장 자주 만지는 ~50 항목에 한 줄 docstring. 대상:
   - voltex (facade): `prelude` 모듈, `App`/`RuntimeApp` 진입점
   - voltex_runtime: `RuntimeApp`/`RuntimeConfig` 메서드 ~10개
   - voltex_cli: bin help text 와 별개로 `Manifest` 공개 메서드
   - voltex_ecs core: `World`, `Entity`, `Component` trait, `ComponentRegistry`, `Transform`, `Parent`/`Children`
   - voltex_editor core: `EditorSession`, `EditorCommand` (variants)
4. **CHANGELOG.md (v0.x → v1.0 highlights)** — Cycle 1~13 의 주요 변경을 7~10 항목으로 요약.
5. **`pub(crate)` downgrade 후보 식별** — 강제 변경은 안 함, 후보 목록만 `docs/v1.0-pub-audit.md` 같은 follow-up 노트에 기록.

### 2.2 v1.1 로 deferral (scope out)

1. **전체 1586 missing_docs 해소** — 자동 boilerplate doc 삽입은 정보 가치가 낮고, 수동 docstring 1586개는 본 cycle 한 commit-cycle 에 불가능. STATUS 에 명시.
2. **`#![warn(missing_docs)]` crate 전체 적용** — 위와 같은 이유. v1.0 은 prelude + tier-1 API 만 보장.
3. **`pub(crate)` 자동 다운그레이드** — 실제 변경은 위험도 높음 (외부 사용자가 의존 가능). 후보 노트만 작성.

### 2.3 Roadmap done 기준과의 차이

로드맵 § Cycle 13 의 done 기준은 다음과 같이 재해석한다:

| 원안 | 본 cycle |
|---|---|
| `cargo doc --workspace --no-deps` warning 0 | ✅ 그대로 유지 |
| `cargo build --workspace -- -W missing_docs` warning 0 | ❌ deferral — v1.1 후속 작업으로 명시 |
| facade prelude 가 examples 의 use 문 90%+ cover | ✅ 그대로 유지 |
| CHANGELOG.md 7~10 항목 | ✅ 그대로 유지 |

`#![warn(missing_docs)]` 강제 적용을 푸는 대신 STATUS 에 "v1.0 은 prelude + tier-1 API 한정 rustdoc, full sweep 은 v1.1" 라인 추가.

## 3. 작업 분할

| 세션 | 작업 |
|---|---|
| **1** | `cargo doc` warning 0 만들기: voltex_math/voltex_renderer/voltex_physics/voltex_ai/voltex_script 의 intra-doc link 깨짐 fix. voltex facade vs voltex_renderer 파일 충돌 (`output filename collision`) 해결. |
| **2** | facade `voltex::prelude` 보강 + tier-1 API rustdoc (~50 항목). 변경 시 `cargo doc` 재실행으로 추가 워닝 0 유지. |
| **3** | `CHANGELOG.md` 작성 + `docs/v1.0-pub-audit.md` (pub(crate) 후보 노트) + STATUS.md / roadmap 갱신. |

## 4. 검증 게이트

- `cargo doc --workspace --no-deps 2>&1 | grep -c "^warning"` → **0**
- `cargo build --workspace` → clean (regression 없음)
- `cargo test --workspace --lib` → 914 (E3 종료 기준) 유지
- examples 의 use 문 sample (`triangle.rs`, `runtime_demo/main.rs`, `editor_demo/main.rs`) → `use voltex::prelude::*;` 한 줄로 90%+ cover

## 5. 리스크

| # | 위험 | 완화 |
|---|---|---|
| 1 | intra-doc link fix 가 의도치 않게 doctest 코드를 깨뜨림 | rustdoc 워닝만 만지고 의미 있는 코드 변경 회피. 코멘트 내 backtick escape 만. |
| 2 | facade prelude 에 새 타입 추가 시 examples 의 직접 use 문과 이름 충돌 | 추가 직전 grep 으로 충돌 검사. 충돌 시 prelude 미포함 + 명시 import 권장으로 처리. |
| 3 | CHANGELOG 가 사용자 기대보다 짧음 | roadmap 의 cycle 1~13 표를 1:1 매핑해 자연스럽게 13~15 항목 산출 |
| 4 | v1.1 deferral 이 "끝나지 않은 일" 처럼 보임 | STATUS / CHANGELOG 에 명시: "v1.0 의 docstring 정책은 prelude + tier-1 API; 나머지 항목은 v1.1 점진 작업". 외부 사용자가 cargo doc 으로 정상 빌드되는 사이트를 받을 수 있는 것이 핵심. |

## 6. 관련 문서

- `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 13
- 이후 작성될 `CHANGELOG.md`, `docs/v1.0-pub-audit.md`
