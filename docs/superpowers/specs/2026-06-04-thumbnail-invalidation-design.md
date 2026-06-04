# 썸네일 핫리로드 invalidation (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.93 (썸네일) 의 비범위 후속

## 목표

선택된 텍스처의 파일이 디스크에서 바뀌면 썸네일도 갱신된다. v1.0.93 은
`thumb_cache` 가 영구라 (1) 변경이 반영 안 되고 (2) 디코드 실패 None 캐시가
파일을 고쳐도 영원히 남는다.

## 유닛 — `editor_demo`

- 썸네일 생성 시(`thumb_cache` 신규 entry — 성공/실패 모두) 해당 경로를
  `watcher.watch` — 선택만 하고 Open 안 한 파일도 변경 감지.
- poll 루프: 각 changed 경로에 대해 **분기 처리 전에**
  `thumb_cache.remove(&changed)` (무조건, 없으면 no-op) — Open 된 텍스처(씬
  적용 핫리로드 분기)와 썸네일이 같은 파일일 수 있어 분기와 독립으로 처리.
  다음 프레임 선택 조회가 자연 재디코드/재등록.
- 한계(의도): 재등록은 새 슬롯 — 옛 슬롯 bind group 은 v1.0.93 의 영구-슬롯
  계약대로 잔존 (에디터 수명 규모).

## 검증

- editor_demo build + clippy 그린 (데모 배선 — 단위 테스트 면적은 watcher /
  FileWatcher 쪽에 이미 존재).

## CODEMAP 영향 — 없음 (demo 한정).
