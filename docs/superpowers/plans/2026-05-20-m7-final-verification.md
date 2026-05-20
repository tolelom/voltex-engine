# M7: 재구조화 최종 검증 구현 계획

**Goal:** M1-M6에서 도입한 재구조화 변경(CODEMAP / 파일 분리 / 에러 표준화 / ECS 패턴 / facade)을 최종 검증하고, 마무리 정리(추가 예제 마이그레이션, cargo doc, 문서 sync)를 한다.

**Architecture:** 코드 변경 최소. 검증 게이트 통과 + 문서 sync.

**Tech Stack:** Rust workspace. 외부 의존성 추가 없음.

**Spec reference:** `docs/superpowers/specs/2026-05-20-engine-restructure-design.md` 마일스톤 M7 + 성공기준 1-7.

---

## 성공기준 점검 (2026-05-20)

| # | 기준 | 상태 |
|---|---|---|
| 1 | 모든 crate가 `CODEMAP.md` 보유 | ✅ 12/12 (11 sub + voltex facade) |
| 2 | 모든 `.rs` 600줄 이하 | ✅ M2/M3 후 0건 |
| 3 | 모든 crate가 `thiserror` 기반 통일 에러 사용 | ✅ M4 완료 (asset/net/ecs/audio/ai/script/renderer/editor) |
| 4 | ECS 컴포넌트/시스템 등록 패턴 표준화 | ✅ M5 완료 (Component trait + `register::<T>()`) |
| 5 | 최상위 `voltex` facade crate 존재 | ✅ M6 완료 |
| 6 | 테스트 760+ 전부 통과 | ✅ 765개 통과 (M5 +3, M6 +2) |
| 7 | 모든 examples 빌드/실행 정상 | ✅ `cargo build --workspace` 통과 |

성공기준 7개 모두 충족. M7은 마무리 검증과 문서 sync로 한정.

---

## 작업 순서

- [ ] **Step 1**: 이 plan 작성 (본 문서)
- [ ] **Step 2**: `cargo doc -p voltex --no-deps` 무경고 확인. 경고 발생 시 최소 수정
- [ ] **Step 3**: 추가 예제 1개 facade 마이그레이션 — `examples/many_cubes` 또는 `examples/hierarchy_demo` 1개로 실제 사용 검증
- [ ] **Step 4**: STATUS.md에 M1-M7 진행상황 summary 섹션 추가
- [ ] **Step 5**: DEFERRED.md 갱신 (재구조화 작업으로 해결된 항목 정리)
- [ ] **Step 6**: 최종 commit + push

---

## 검증 게이트

```bash
cargo build --workspace
cargo test --workspace --lib           # 765+ 통과
cargo doc -p voltex --no-deps          # 무경고
cargo clippy -p voltex -- -D warnings  # facade는 단순 re-export라 통과 기대
```

---

## 리스크 및 완화

| 리스크 | 영향 | 완화 |
|---|---|---|
| cargo doc 경고가 무경고 기준 미달 | 성공기준 보조 미충족 | 경고는 missing_docs류가 대부분 — facade는 docstring 보강만으로 해결 |
| 추가 예제 마이그레이션이 다른 기능 깨뜨림 | 빌드 실패 | 의존성 줄이는 변경만 — main.rs는 import 경로만 교체. 빌드 즉시 확인 |

---

## Out of Scope

- 모든 예제의 facade 마이그레이션 — M6 spec out of scope. 1개만 추가하여 사용 패턴 검증
- 게임 템플릿/스캐폴딩 (spec out of scope)
- voltex_renderer sub-crate 분해 (spec out of scope)
- App 추상화 (M6에서 yagni 보류)

---

## 후속 작업

M7 완료 후 다음 phase: 실제 게임 제작 단계 진입. STATUS.md "다음 작업 후보"의 6개 항목 참고:
1. inspector 통합
2. scene 영속화 검증
3. EditorCommand 확장 (AddChild/RemoveChild, Undo/Redo)
4. viewport raycast/gizmo
5. Lua API 확장 + Entity.generation 픽스
6. survivor_game ECS/Physics 재작성
