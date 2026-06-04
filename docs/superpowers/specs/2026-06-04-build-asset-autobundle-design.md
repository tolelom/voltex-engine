# voltex build 씬 참조 에셋 자동 동봉 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.96~100 (경로 영속화 + Vfs resolve) 의 최종 배선

## 현황 (코드 확인 완료)

`pack_archive` 는 이미 manifest 의 scene + scripts + assets 디렉토리를
forward-slash manifest-상대 키로 `.vpkg` 에 재귀 수집 — v1.0.99/100 형식과
일치해 **선언된 디렉토리 안의 에셋은 이미 동작**한다. 격차 2개:

1. 씬이 참조하는 에셋(TexturePath/MeshPath)이 선언 디렉토리 **밖**이면 누락.
2. 참조 에셋이 어디에도 없으면 **조용히 깨진 빌드** 출하.

## 설계

### 유닛 1 — `voltex_cli::scan_scene_asset_paths(bytes) -> Vec<String>`

- voltex_cli 에 `voltex_ecs` 의존 추가 (workspace crate — 외부 dep 0 정책
  유지. ecs 는 math/asset 만 끌어옴, GPU 무관).
- 포맷 자동 감지: `VSCN` 매직 → binary, 첫 비공백 `{` → JSON, 그 외 →
  `.vscn` 텍스트(레지스트리 컴포넌트 미포함 — 빈 결과, 정상).
- `ComponentRegistry::register_raw` 는 fn 포인터라 클로저 캡처 불가 → CLI
  로컬 마커 컴포넌트(`TexPathRef(String)` / `MeshPathRef(String)`)를 raw
  deserializer 가 World 에 부착, 디시리얼라이즈 후 query 로 수집.
  Transform 등 다른 컴포넌트는 미등록 → 자연 skip (등록 기반 직렬화 계약).

### 유닛 2 — `pack_archive` 확장

씬 수집 후: 참조 경로마다 이미 entries 에 있으면 통과; 없는데
`manifest_dir.join(path)` 가 존재하면 **자동 동봉** + 로그; 디스크에도 없으면
`voltex build: warning: scene references missing asset ...` (빌드는 계속 —
런타임 fallback 이 흰색 기본 텍스처로 렌더, advisory 가 맞음).

## 검증

- 단위: JSON 씬에서 두 경로 추출 / binary 씬 추출 / 텍스트 씬 빈 결과 /
  pack_archive 가 선언 밖 참조 파일 자동 동봉 / 누락 시 경고만 하고 entries
  불변. build/clippy(-D warnings)/test 그린.

## 비범위 (후속)

- Audio 참조 수집(AudioSource 는 ClipId 기반), 씬이 참조하는 씬(중첩),
  자동 동봉의 중복 제거 최적화(현재 entries 선형 탐색 — 규모상 충분).

## CODEMAP 영향

- `voltex_cli/CODEMAP.md` — scan + auto-bundle, voltex_ecs 의존 추가 명기.
