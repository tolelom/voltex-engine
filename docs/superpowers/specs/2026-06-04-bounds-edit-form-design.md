# 인스펙터 Bounds 값 편집 폼 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.92 (값 표시, read-only) 비범위 후속. 핸드오프 우선순위 4.

## 문제

Components 목록의 빌트인 값 요약은 read-only. v1.0.92 비범위가 명시한
"값 편집 폼 (Bounds 슬라이더 → AddComponent dispatch로 undo 지원)" 구현.

## 발견된 전제 격차 — AddComponent undo 의미론

`capture_inverse` 의 AddComponent inverse 가 무조건
`RemoveComponentByName` — **이미 붙어 있는 컴포넌트를 AddComponent 로 덮어
쓴 뒤 undo 하면 prior 값 복원이 아니라 컴포넌트 삭제**가 된다. "+ bounds"
신규 추가에는 맞는 의미지만 값 편집에는 데이터 파괴.

### 유닛 1 — capture_inverse 개선

AddComponent inverse 를 분기: prior `serialize` 가 Some(기존 값 존재) →
`AddComponentByBytes(prior)` (RemoveComponent undo 와 동일 메커니즘 재사용),
None(신규) → 기존 `RemoveComponentByName`. 신규 추가 undo 의미 불변 (기존
e3 테스트 가드), 편집 undo 는 값 복원으로 교정.

### 유닛 2 — 인스펙터 Bounds 슬라이더

- Components 목록에서 `bounds` 만 요약 텍스트 대신 6 슬라이더 (Min X/Y/Z,
  Max X/Y/Z, 범위 ±50 — Pos 슬라이더와 동일). 변경 시
  `AddComponent { name: "bounds", data }` dispatch — deserialize 가
  덮어쓰고(E3 멱등 계약), 유닛 1 로 undo 가 prior 값 복원.
- `bounds_bytes(min, max) -> Vec<u8>` 순수 헬퍼 (Component::serialize 와
  동일 24-byte LE 레이아웃) — 레이아웃 drift 는 round-trip 테스트로 가드.
- 드래그 중 슬라이더 틱마다 1 dispatch — SetTransform 과 동일한 v1 수용
  (히스토리 코얼레싱은 기존 비범위 그대로).

## 검증

- 단위: bounds_bytes ↔ registry deserialize round-trip / AddComponent
  edit-undo 가 prior Bounds 복원 / 신규 add-undo 는 여전히 제거 (기존
  테스트). build/clippy(-D warnings)/workspace test 그린.

## 비범위 (후속)

- mesh/material_handle·script 편집 폼 (핸들은 에셋 참조 UI 가 맞고, script
  는 경로 텍스트 입력 — 별도 사이클), 커스텀 컴포넌트 리플렉션 편집,
  드래그 코얼레싱(슬라이더 release 시 1 entry).

## CODEMAP 영향

- voltex_editor CODEMAP — inspector 항목에 Bounds 편집 + AddComponent
  inverse 분기 명기.
