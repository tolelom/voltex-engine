# 디렉토리 감시 + 에셋 브라우저 자동 새로고침 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.87~90 핫리로드 시리즈의 비범위 후속

## 목표

에셋 브라우저가 보고 있는 디렉토리에 파일이 추가/삭제/이름변경되면 목록이
자동 갱신된다. 지금은 에디터 재시작 또는 디렉토리 재진입으로만 갱신.

### 현황 근거 (코드 확인 완료)

- `FileWatcher` 는 `fs::metadata(path).modified()` 폴링 — **디렉토리에도 그대로
  동작**: 파일 추가/삭제/이름변경 시 디렉토리 mtime 이 갱신된다 (NTFS/ext4).
  단, 포함된 파일의 **내용 수정**은 디렉토리 mtime 을 바꾸지 않음(개별 파일
  감시의 역할 — 기존 핫리로드가 담당).
- `AssetBrowser::refresh()` 이미 존재 (`new`/`navigate_to`/`go_up` 이 호출).

## 유닛 1 — `voltex_asset`: 디렉토리 감시 보장

코드 변경 없음(메커니즘이 이미 동작). 회귀 테스트 + doc 으로 계약화:
- `watch(dir)` → 파일 생성 → poll 이 dir 보고.
- 파일 삭제 → poll 이 dir 보고.
- 모듈 doc 에 "디렉토리 mtime 감시 = 추가/삭제/이름변경 감지, 내용 수정은
  미감지" 명시.

## 유닛 2 — `editor_demo`: 브라우저 자동 새로고침

- `AppState.watched_browser_dir: Option<PathBuf>` — 현재 감시 중인 브라우저
  디렉토리. 기존 메인 `watcher` 재사용(경로가 파일 맵들과 충돌 불가).
- 매 프레임 패널 호출 후: `browser.current != watched_browser_dir` 이면 옛
  디렉토리 unwatch → 새 디렉토리 watch (탐색 추적).
- poll 분기 추가: changed == watched_browser_dir → `browser.refresh()` +
  상태 텍스트. refresh 가 selection/activation 을 비우는 것은 의도된 동작
  (목록이 바뀌어 선택이 무효일 수 있음).

## 검증

- voltex_asset 단위 +2 (디렉토리 생성/삭제 감지).
- editor_demo build. build / clippy(`-D warnings`) / test 그린.

## 비범위 (후속)

- 재귀(하위 디렉토리) 감시, OS 네이티브 알림, 브라우저 목록의 부분 갱신
  (전체 re-read 로 충분 — 디렉토리당 수백 엔트리 규모).

## CODEMAP 영향

- `crates/voltex_asset/CODEMAP.md` — FileWatcher 디렉토리 감시 계약 명시.
