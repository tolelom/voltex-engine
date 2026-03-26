# Asset Browser Design

## Overview

에디터 도킹 패널에 파일 시스템 탐색기를 추가한다. 지정된 에셋 루트 디렉토리에서 파일/폴더를 플랫 리스트로 표시하고, 폴더 이동/뒤로가기를 지원. 읽기 전용.

## Scope

- 에셋 루트 디렉토리 기준 파일/폴더 목록 표시
- 폴더 클릭 → 해당 폴더로 이동
- 뒤로가기 (부모 디렉토리, root까지만)
- 파일 선택 시 이름/크기/확장자 정보 표시
- 로딩/프리뷰 없음 (읽기 전용)

## Data Model

```rust
pub struct AssetBrowser {
    root: PathBuf,
    current: PathBuf,
    entries: Vec<DirEntry>,
    selected_file: Option<String>,  // 선택된 파일 이름
}

struct DirEntry {
    name: String,
    is_dir: bool,
    size: u64,
}
```

## API

```rust
impl AssetBrowser {
    /// 에셋 루트 디렉토리로 초기화. entries를 즉시 스캔.
    pub fn new(root: PathBuf) -> Self;

    /// 현재 디렉토리의 파일/폴더를 다시 스캔.
    pub fn refresh(&mut self);

    /// 하위 폴더로 이동. root 바깥으로 나가지 못하게 가드.
    /// 실제 디렉토리가 아니면 무시.
    pub fn navigate_to(&mut self, dir_name: &str);

    /// 부모 디렉토리로 이동. root에서는 무시.
    pub fn go_up(&mut self);

    /// 현재 경로 (root 기준 상대 경로 문자열).
    pub fn relative_path(&self) -> String;

    /// 현재 entries 접근.
    pub fn entries(&self) -> &[DirEntry];
}

/// 에셋 브라우저 패널 UI 렌더링.
pub fn asset_browser_panel(
    ui: &mut UiContext,
    browser: &mut AssetBrowser,
    rect: &Rect,
);
```

## refresh 로직

```rust
fn refresh(&mut self) {
    self.entries.clear();
    self.selected_file = None;
    if let Ok(read_dir) = std::fs::read_dir(&self.current) {
        for entry in read_dir.flatten() {
            let meta = entry.metadata().ok();
            let is_dir = meta.as_ref().map_or(false, |m| m.is_dir());
            let size = meta.as_ref().map_or(0, |m| m.len());
            let name = entry.file_name().to_string_lossy().to_string();
            self.entries.push(DirEntry { name, is_dir, size });
        }
    }
    // 정렬: 폴더 먼저, 그 다음 파일. 각각 이름순.
    self.entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
    });
}
```

## navigate_to 로직

```rust
fn navigate_to(&mut self, dir_name: &str) {
    let target = self.current.join(dir_name);
    // root 바깥 가드: target이 root의 하위여야 함
    if target.starts_with(&self.root) && target.is_dir() {
        self.current = target;
        self.refresh();
    }
}
```

## go_up 로직

```rust
fn go_up(&mut self) {
    if self.current != self.root {
        if let Some(parent) = self.current.parent() {
            if parent.starts_with(&self.root) || parent == self.root {
                self.current = parent.to_path_buf();
                self.refresh();
            }
        }
    }
}
```

## UI 렌더링

```
asset_browser_panel(ui, browser, rect):
    ui.layout = LayoutState::new(rect.x + 4, rect.y + 4)

    // 현재 경로
    ui.text(&format!("Path: {}", browser.relative_path()))

    // 뒤로가기 (root가 아닐 때)
    if current != root:
        if ui.button("[..]"):
            browser.go_up()

    // 항목 목록
    for entry in browser.entries():
        if entry.is_dir:
            label = format!("[D] {}", entry.name)
        else:
            label = format!("    {}", entry.name)

        // 클릭 가능한 텍스트 행
        하이라이트 if selected
        if clicked:
            if entry.is_dir:
                browser.navigate_to(&entry.name)
            else:
                browser.selected_file = Some(entry.name.clone())

    // 선택된 파일 정보
    if let Some(file) = &browser.selected_file:
        ui.text("-- File Info --")
        ui.text(&format!("Name: {}", file))
        // entries에서 찾아서 크기 표시
        if found:
            ui.text(&format!("Size: {}", format_size(size)))
            ui.text(&format!("Type: {}", extension))
```

크기 포맷: `format_size(bytes)` → "123 B", "4.5 KB", "1.2 MB"

## File Structure

- `crates/voltex_editor/src/asset_browser.rs` — AssetBrowser, DirEntry, asset_browser_panel, format_size
- `crates/voltex_editor/src/lib.rs` — 모듈 추가
- `examples/editor_demo/src/main.rs` — Console 패널(panel 3)을 AssetBrowser로 교체

## Testing

### AssetBrowser 로직 (파일 시스템 사용, GPU 불필요)
- `test_new_scans_entries`: 임시 디렉토리에 파일/폴더 생성 → new() → entries 확인
- `test_refresh`: 파일 추가 후 refresh → entries 업데이트 확인
- `test_navigate_to`: 하위 폴더로 이동 → current 변경, entries 변경 확인
- `test_go_up`: 하위에서 go_up → 부모로 이동 확인
- `test_go_up_at_root`: root에서 go_up → current 변경 안 됨
- `test_root_guard`: root 바깥 경로 시도 → 무시
- `test_entries_sorted`: 폴더가 파일보다 먼저, 이름순

### format_size
- `test_format_size`: 0, 500, 1024, 1500000 등

### UI (draw_list 검증)
- `test_panel_draws_commands`: browser + UiContext → draw_list에 커맨드 생성 확인
