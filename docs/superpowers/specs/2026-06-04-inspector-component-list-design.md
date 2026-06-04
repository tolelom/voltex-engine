# 인스펙터 컴포넌트 목록 + Remove (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: E3 (AddComponent/RemoveComponent 커맨드 + undo + 인스펙터 Add 버튼)

## 목표

E3 가 커맨드 레이어(AddComponent/RemoveComponent + 양방향 undo)와 인스펙터의
"Add Component" 버튼까지 구현했지만, **엔티티에 어떤 컴포넌트가 붙어 있는지
보는 UI 와 떼는 UI 가 없다** — `RemoveComponent` 는 현재 undo 내부에서만 쓰인다.
인스펙터에 "Components" 섹션을 추가해 이 격차를 닫는다.

### 현황 근거 (코드 확인 완료)

- `ComponentRegistry::entries() -> &[ComponentEntry]` 열거 이미 존재.
- `ComponentEntry.serialize` 가 `Option<Vec<u8>>` 반환 — `is_some()` = 부착 여부.
- `EditorCommand::RemoveComponent` + `Inverse::AddComponentByBytes`(prior bytes
  캡처) undo 경로 완비 — UI 가 dispatch 만 하면 됨.
- 인스펙터 Add 버튼은 부착 여부와 무관하게 6개 모두 항상 표시 (멱등이지만 노이즈).

## 유닛 1 — `voltex_editor::panels::inspector`

- **`attached_component_names(session, entity) -> Vec<String>`** (pub, 순수) —
  `registry.entries()` 중 `(entry.serialize)(world, entity).is_some()` 인
  이름 목록. UI 와 테스트가 같은 로직을 공유.
- **"-- Components --" 섹션** (Parent 표시 아래, Add 섹션 위):
  부착된 각 컴포넌트마다 `- <name>` 버튼 → `RemoveComponent` dispatch.
  `register_raw` 엔트리(예: runtime 의 audio_source)는 remove 가 no-op 이라
  버튼이 동작하지 않을 수 있음 — 기존 커맨드 규약(no-op Ok) 그대로, doc 명시.
- **Add 버튼 필터링**: 이미 부착된 컴포넌트의 `+ <name>` 버튼은 숨김
  (멱등 semantics 불변 — 표시만 정리).

## 검증

- 단위 (GUI 없이 기존 e3/inspector 테스트 스타일):
  - `attached_component_names` 가 부착된 registered 컴포넌트만 나열.
  - Remove 버튼 로직 복제 → dispatch → 컴포넌트 제거 → undo 시 prior bytes 복원.
  - Add 필터: transform 부착 후 Add 후보 목록에서 제외됨.
- build / clippy(`-D warnings`) / test 그린.

## 비범위 (후속)

- 컴포넌트 **값** 표시/편집 (리플렉션 인프라 — Transform/Tag 외 raw bytes 뿐),
- 커스텀(앱 등록) 컴포넌트의 Add 버튼 동적 생성 (`default_bytes` 가 raw 엔트리에
  의미 없음), 스크롤 (컴포넌트 6개 내라 불필요).

## CODEMAP 영향

- `crates/voltex_editor/CODEMAP.md` — `attached_component_names` 반영.
