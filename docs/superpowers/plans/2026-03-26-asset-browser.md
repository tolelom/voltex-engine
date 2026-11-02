# Asset Browser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a file system asset browser panel to the editor that displays files/folders in a flat list with navigation.

**Architecture:** `AssetBrowser` struct manages directory state (current path, entries cache). `asset_browser_panel` function renders the UI. Uses `std::fs::read_dir` for file listing. No external dependencies needed.

**Tech Stack:** Rust, std::fs, voltex_editor (UiContext, Rect, LayoutState, widgets)

**Spec:** `docs/superpowers/specs/2026-03-26-asset-browser-design.md`

---

### Task 1: AssetBrowser struct + logic + tests

**Files:**
- Create: `crates/voltex_editor/src/asset_browser.rs`
- Modify: `crates/voltex_editor/src/lib.rs`

- [ ] **Step 1: Write tests and implementation**

Create `crates/voltex_editor/src/asset_browser.rs` with the full AssetBrowser implementation and tests. The struct handles directory scanning, navigation, and path safety.

Key implementation details:
- `new(root)` → canonicalize root, set current=root, refresh
- `refresh()` → read_dir, collect into Vec<DirEntry>, sort (dirs first, then by name)
- `navigate_to(dir_name)` → join path, check starts_with(root) && is_dir, refresh
- `go_up()` → parent path, check >= root, refresh
- `relative_path()` → strip_prefix(root) display
- `format_size(bytes)` → "B" / "KB" / "MB" formatting

Tests using `std::fs` temp directories:
- test_new_scans_entries
- test_navigate_to
- test_go_up
- test_go_up_at_root
- test_root_guard
- test_entries_sorted (dirs before files)
- test_format_size

- [ ] **Step 2: Add module to lib.rs**

```rust
pub mod asset_browser;
pub use asset_browser::{AssetBrowser, asset_browser_panel};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p voltex_editor --lib asset_browser -- --nocapture`
Expected: all tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/voltex_editor/src/asset_browser.rs crates/voltex_editor/src/lib.rs
git commit -m "feat(editor): add AssetBrowser with directory navigation and file listing"
```

---

### Task 2: asset_browser_panel UI function

**Files:**
- Modify: `crates/voltex_editor/src/asset_browser.rs`

- [ ] **Step 1: Write test**

```rust
#[test]
fn test_panel_draws_commands() {
    let dir = std::env::temp_dir().join("voltex_ab_ui_test");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("test.txt"), "hello").unwrap();

    let mut browser = AssetBrowser::new(dir.clone());
    let mut ui = UiContext::new(800.0, 600.0);
    let rect = Rect { x: 0.0, y: 0.0, w: 300.0, h: 400.0 };

    ui.begin_frame(0.0, 0.0, false);
    asset_browser_panel(&mut ui, &mut browser, &rect);
    ui.end_frame();

    assert!(ui.draw_list.commands.len() > 0);

    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: Implement asset_browser_panel**

```rust
pub fn asset_browser_panel(
    ui: &mut UiContext,
    browser: &mut AssetBrowser,
    rect: &Rect,
) {
    ui.layout = LayoutState::new(rect.x + PADDING, rect.y + PADDING);

    // Current path
    let path_text = format!("Path: /{}", browser.relative_path());
    ui.text(&path_text);

    // Go up button (not at root)
    if browser.current != browser.root {
        if ui.button("[..]") {
            browser.go_up();
            return; // entries changed, redraw next frame
        }
    }

    // Entry list
    let gw = ui.font.glyph_width as f32;
    let gh = ui.font.glyph_height as f32;
    let line_h = gh + PADDING;
    let mut clicked_dir: Option<String> = None;

    for entry in browser.entries() {
        let y = ui.layout.cursor_y;
        let x = rect.x + PADDING;

        // Highlight selected file
        if !entry.is_dir {
            if browser.selected_file.as_deref() == Some(&entry.name) {
                ui.draw_list.add_rect(rect.x, y, rect.w, line_h, COLOR_SELECTED);
            }
        }

        // Click detection
        if ui.mouse_clicked && ui.mouse_in_rect(rect.x, y, rect.w, line_h) {
            if entry.is_dir {
                clicked_dir = Some(entry.name.clone());
            } else {
                browser.selected_file = Some(entry.name.clone());
            }
        }

        // Draw label
        let label = if entry.is_dir {
            format!("[D] {}", entry.name)
        } else {
            format!("    {}", entry.name)
        };

        let text_y = y + (line_h - gh) * 0.5;
        let mut cx = x;
        for ch in label.chars() {
            let (u0, v0, u1, v1) = ui.font.glyph_uv(ch);
            ui.draw_list.add_rect_uv(cx, text_y, gw, gh, u0, v0, u1, v1, COLOR_TEXT);
            cx += gw;
        }

        ui.layout.cursor_y += line_h;
    }

    // Navigate after iteration (avoids borrow conflict)
    if let Some(dir) = clicked_dir {
        browser.navigate_to(&dir);
    }

    // Selected file info
    if let Some(ref file_name) = browser.selected_file.clone() {
        ui.text("-- File Info --");
        ui.text(&format!("Name: {}", file_name));
        if let Some(entry) = browser.entries.iter().find(|e| e.name == *file_name) {
            ui.text(&format!("Size: {}", format_size(entry.size)));
            if let Some(ext) = file_name.rsplit('.').next() {
                if file_name.contains('.') {
                    ui.text(&format!("Type: .{}", ext));
                }
            }
        }
    }
}
```

Note: `selected_file` needs to be `pub(crate)` or accessed via a method. Also `entries` field needs to be accessible. Simplest: make `selected_file` and `entries` pub.

- [ ] **Step 3: Run tests**

Run: `cargo test -p voltex_editor --lib asset_browser -- --nocapture`
Expected: all tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/voltex_editor/src/asset_browser.rs
git commit -m "feat(editor): add asset_browser_panel UI function"
```

---

### Task 3: Integrate into editor_demo

**Files:**
- Modify: `examples/editor_demo/src/main.rs`

- [ ] **Step 1: Replace Console panel with AssetBrowser**

Add import:
```rust
use voltex_editor::{..., AssetBrowser, asset_browser_panel};
```

Add to AppState:
```rust
asset_browser: AssetBrowser,
```

In `resumed`, initialize:
```rust
let asset_browser = AssetBrowser::new(std::env::current_dir().unwrap_or_default());
```

In the panel loop, replace panel 3 (Console):
```rust
3 => {
    asset_browser_panel(&mut state.ui, &mut state.asset_browser, rect);
}
```

- [ ] **Step 2: Build and test**

Run: `cargo build -p editor_demo`
Run: `cargo test -p voltex_editor`

- [ ] **Step 3: Commit**

```bash
git add examples/editor_demo/src/main.rs
git commit -m "feat(editor): integrate asset browser into editor_demo"
```

---

### Task 4: Update docs

**Files:**
- Modify: `docs/dev/STATUS.md`
- Modify: `docs/dev/DEFERRED.md`

- [ ] **Step 1: Update STATUS.md**

Add: `- voltex_editor: AssetBrowser (file system browsing, directory navigation)`
Update test count.

- [ ] **Step 2: Update DEFERRED.md**

```
- ~~**에셋 브라우저**~~ ✅ 파일 목록 + 디렉토리 탐색 완료. 에셋 로딩/프리뷰 미구현.
```

- [ ] **Step 3: Commit**

```bash
git add docs/dev/STATUS.md docs/dev/DEFERRED.md
git commit -m "docs: update STATUS.md and DEFERRED.md with asset browser"
```
