# 에셋 상대경로 정규화 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.96~98 (경로 영속화). Vfs/.vpkg 동봉의 전제조건.

## 문제

TexturePath/MeshPath 가 브라우저의 **절대경로**를 저장 — 씬이 다른 머신에서
깨지고, `.vpkg` 번들 엔트리(상대 키)와 매칭될 수 없다.

## 설계

**저장 형식 = 브라우저 root 상대 + forward slash** (`assets/hero.png`).
- 브라우저는 root 밖 탐색을 차단(navigate_to/go_up 가드)하므로 Open 경로는
  항상 root 안 — strip 항상 성공 (밖이면 절대경로 fallback, 방어).
- editor_demo 의 root = CWD → 상대경로 fs read 가 CWD 기준으로 자연 동작.
  런타임도 게임을 프로젝트 루트에서 실행하는 기존 규약(`cargo run`/`voltex
  run`) 그대로 동작 — resolve 코드 무변경.
- forward slash 는 Windows fs API 도 허용 + 크로스 OS 씬 호환.

## 유닛 1 — `voltex_editor::AssetBrowser::relative_to_root`

`(&self, path: &Path) -> String` — root prefix strip + `\` → `/`.
단위 테스트 (root 안 strip / root 밖 fallback).

## 유닛 2 — `editor_demo`

Open 핸들러 첫머리에서 활성화 경로를 **한 번** 상대화해 전 분기가 동일 형태
사용 — TexturePath/MeshPath 저장, watch 맵 키, thumb 캐시 키, fs read 모두
일관 (절대/상대 이중 키 분열 방지). LoadScene path 문자열도 상대.

## 호환성

기존에 절대경로로 저장된 씬: PathBuf::from(절대) read 는 같은 머신에서 계속
동작 — resolve 경로는 형식 무관 (마이그레이션 불필요, 새 저장부터 상대).

## 검증

- voltex_editor 단위 +2. editor_demo build. build/clippy/test 그린.

## 비범위 (후속)

- Vfs 경유 resolve, `.vpkg` 에셋 동봉(이 형식을 키로 사용), 씬 파일 위치
  기준 상대화(현재 root=CWD 규약으로 충분).

## CODEMAP 영향

- `voltex_editor/CODEMAP.md` — relative_to_root.
