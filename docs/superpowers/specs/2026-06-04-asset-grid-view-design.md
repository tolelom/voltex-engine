# 에셋 브라우저 폴더 그리드 뷰 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.93 (이미지 위젯 인프라 + 선택 파일 썸네일), v1.0.95 (썸네일 invalidation). 핸드오프 우선순위 3.

## 문제

브라우저는 텍스트 리스트 + 선택 파일 1개의 썸네일뿐. 폴더 안 텍스처들을
한눈에 보고 고르는 그리드 뷰가 없다.

## 설계 (레이어링 유지: voltex_editor 는 GPU 무관, 디코드/업로드는 통합 레이어)

### voltex_editor

- `BrowserView { List, Grid }` + `AssetBrowser.view: BrowserView` (기본
  List — 기존 동작/테스트 불변). 패널 상단 `[Grid]`/`[List]` 토글 버튼.
- `asset_browser_panel(ui, browser, rect, preview, thumbs)` — 신규 마지막
  파라미터 `thumbs: Option<&HashMap<String, u32>>` (현재 디렉토리 파일명 →
  `register_texture` 슬롯). 통합 레이어가 채움; None/미포함 파일은
  placeholder.
- Grid 렌더: 셀 64px 썸네일 + 한 줄 라벨(셀 폭만큼 truncate).
  - `grid_columns(avail_w, cell_w, pad) -> usize` 순수 함수 (min 1) —
    단위 테스트 대상.
  - 디렉토리 셀: placeholder + `[D]` 라벨, 클릭 = navigate (리스트와 동일
    의미). 파일 셀: thumbs 슬롯 있으면 `add_image`, 없으면 kind 접두 문자
    placeholder, 클릭 = 선택 (핸드오프 "셀 클릭 선택").
  - 그리드 아래 File Info / 썸네일 / Open 영역은 기존 그대로 (양 뷰 공통).
- 선택 하이라이트는 셀 배경.

### editor_demo

- 그리드 모드일 때 현재 디렉토리의 Texture 파일들에 대해 `thumb_cache`
  (v1.0.93, PathBuf → Option<u32>) 를 채워 이름→슬롯 맵 구성 후 패널에 전달.
- **프레임당 신규 디코드 1개 제한** — 큰 폴더 첫 진입 시 한 프레임 hitch
  방지 (다음 프레임들에서 채워짐). 실패 None 캐시/핫리로드 invalidation
  (v1.0.95) 은 기존 메커니즘 그대로 적용됨.

## 검증

- 단위: `grid_columns` 경계 (좁은 폭 → 1, 정확한 칸 수) / 토글이 view 를
  바꾸고 유지 / 그리드 셀 클릭 = 파일 선택 / 디렉토리 셀 클릭 = navigate /
  thumbs 슬롯 있는 파일 셀은 texture 커맨드, 없으면 placeholder.
  (begin_frame 2-프레임 마우스 주입 패턴.)
- build/clippy(-D warnings)/workspace test 그린.

## 비범위 (후속)

- 메시/씬 프리뷰 썸네일 (텍스처만), 썸네일 슬롯 LRU/해제(v1.0.93 영구-슬롯
  계약 유지), 스크롤(그리드가 패널을 넘치면 잘림 — 리스트와 동일 v1 한계),
  더블클릭 Open.

## CODEMAP 영향

- voltex_editor CODEMAP — asset_browser 항목에 grid 뷰 + 시그니처 변경 명기.
