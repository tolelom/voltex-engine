# 인스펙터 컴포넌트 값 표시 (read-only) (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.89 (인스펙터 컴포넌트 목록 + Remove)

## 목표

v1.0.89 의 Components 목록이 **이름만** 보여준다. 빌트인 컴포넌트는 값 요약을
한 줄 곁들여, 엔티티 상태를 인스펙터에서 바로 읽을 수 있게 한다 (편집은 후속 —
"리플렉션" 인프라 없이 빌트인 한정 직접 읽기).

### 현황 근거 (코드 확인 완료)

- 대상 타입 전부 voltex_ecs 의 pub 필드 — `world.get::<T>()` 직접 읽기 가능,
  리플렉션 불필요: `Bounds(pub AABB)`, `MeshHandle(pub Handle)`,
  `MaterialHandle(pub Handle)`, `ScriptComponent { script_path, instance_id }`.
- transform / tag 는 전용 편집 UI(슬라이더/입력)가 이미 값을 보여줌 → 요약 생략.
- `register_raw` 컴포넌트(예: audio_source)는 에디터가 타입을 모름 → 요약
  없음(이름만) — 의도된 한계.

## 유닛 — `voltex_editor::panels::inspector`

- **`component_value_summary(session, entity, name) -> Option<String>`**
  (pub, 순수): 빌트인 이름 → 한 줄 요약.
  - `bounds` → `min(x, y, z) max(x, y, z)` (소수 2자리)
  - `mesh_handle` / `material_handle` → `id=N gen=M`
  - `script` → `script_path` (+ instance_id 가 UNBOUND 아니면 `#id`)
  - `transform` / `tag` / 미지의 이름 → `None`
- Components 목록 렌더: `- name` 버튼 다음 줄에 요약이 있으면 들여쓴 text.

## 검증

- 단위: bounds/핸들/script 요약 포맷, transform·tag·미지 이름은 None,
  컴포넌트 미부착 시 None.
- build / clippy(`-D warnings`) / test 그린.

## 비범위 (후속)

- 값 **편집**(빌트인 폼 위젯), 커스텀 컴포넌트 리플렉션(바이트 스키마 표시),
  Handle 요약에서 에셋 이름 역참조(RenderAssets 는 에디터 밖).

## CODEMAP 영향

- `crates/voltex_editor/CODEMAP.md` — `component_value_summary` 반영.
