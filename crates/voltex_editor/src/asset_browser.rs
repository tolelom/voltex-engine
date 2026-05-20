use std::path::PathBuf;
use crate::ui_context::UiContext;
use crate::dock::Rect;
use crate::layout::LayoutState;

const COLOR_SELECTED: [u8; 4] = [0x44, 0x66, 0x88, 0xFF];
const COLOR_TEXT: [u8; 4] = [0xEE, 0xEE, 0xEE, 0xFF];
const PADDING: f32 = 4.0;

pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

pub struct AssetBrowser {
    pub root: PathBuf,
    pub current: PathBuf,
    pub entries: Vec<DirEntry>,
    pub selected_file: Option<String>,
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

impl AssetBrowser {
    pub fn new(root: PathBuf) -> Self {
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        let current = root.clone();
        let mut browser = AssetBrowser {
            root,
            current,
            entries: Vec::new(),
            selected_file: None,
        };
        browser.refresh();
        browser
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        self.selected_file = None;
        if let Ok(read_dir) = std::fs::read_dir(&self.current) {
            for entry in read_dir.flatten() {
                let meta = entry.metadata().ok();
                let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
                let size = meta.as_ref().map_or(0, |m| m.len());
                let name = entry.file_name().to_string_lossy().to_string();
                self.entries.push(DirEntry { name, is_dir, size });
            }
        }
        self.entries.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
        });
    }

    pub fn navigate_to(&mut self, dir_name: &str) {
        let target = self.current.join(dir_name);
        if target.starts_with(&self.root) && target.is_dir() {
            self.current = target;
            self.refresh();
        }
    }

    pub fn go_up(&mut self) {
        if self.current != self.root {
            if let Some(parent) = self.current.parent() {
                let parent = parent.to_path_buf();
                if parent.starts_with(&self.root) || parent == self.root {
                    self.current = parent;
                    self.refresh();
                }
            }
        }
    }

    pub fn relative_path(&self) -> String {
        self.current
            .strip_prefix(&self.root)
            .unwrap_or(&self.current)
            .to_string_lossy()
            .to_string()
    }
}

pub fn asset_browser_panel(
    ui: &mut UiContext,
    browser: &mut AssetBrowser,
    rect: &Rect,
) {
    ui.layout = LayoutState::new(rect.x + PADDING, rect.y + PADDING);

    let path_text = format!("Path: /{}", browser.relative_path());
    ui.text(&path_text);

    // Go up button
    if browser.current != browser.root
        && ui.button("[..]") {
            browser.go_up();
            return;
        }

    let gw = ui.font.glyph_width as f32;
    let gh = ui.font.glyph_height as f32;
    let line_h = gh + PADDING;

    // Collect click action to avoid borrow conflict
    let mut clicked_dir: Option<String> = None;
    let entries_snapshot: Vec<(String, bool)> = browser.entries.iter()
        .map(|e| (e.name.clone(), e.is_dir))
        .collect();

    for (name, is_dir) in &entries_snapshot {
        let y = ui.layout.cursor_y;
        let x = rect.x + PADDING;

        // Highlight selected file
        if !is_dir
            && browser.selected_file.as_deref() == Some(name.as_str()) {
                ui.draw_list.add_rect(rect.x, y, rect.w, line_h, COLOR_SELECTED);
            }

        // Click
        if ui.mouse_clicked && ui.mouse_in_rect(rect.x, y, rect.w, line_h) {
            if *is_dir {
                clicked_dir = Some(name.clone());
            } else {
                browser.selected_file = Some(name.clone());
            }
        }

        // Label
        let label = if *is_dir {
            format!("[D] {}", name)
        } else {
            format!("    {}", name)
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

    if let Some(dir) = clicked_dir {
        browser.navigate_to(&dir);
    }

    // File info
    if let Some(ref file_name) = browser.selected_file.clone() {
        ui.text("-- File Info --");
        ui.text(&format!("Name: {}", file_name));
        if let Some(entry) = browser.entries.iter().find(|e| e.name == *file_name) {
            ui.text(&format!("Size: {}", format_size(entry.size)));
            if let Some(dot_pos) = file_name.rfind('.') {
                let ext = &file_name[dot_pos..];
                ui.text(&format!("Type: {}", ext));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("voltex_ab_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_new_scans_entries() {
        let dir = make_temp_dir("scan");
        fs::write(dir.join("file1.txt"), "hello").unwrap();
        fs::write(dir.join("file2.png"), "data").unwrap();
        fs::create_dir_all(dir.join("subdir")).unwrap();

        let browser = AssetBrowser::new(dir.clone());
        assert_eq!(browser.entries.len(), 3);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_entries_sorted_dirs_first() {
        let dir = make_temp_dir("sort");
        fs::write(dir.join("zebra.txt"), "z").unwrap();
        fs::write(dir.join("alpha.txt"), "a").unwrap();
        fs::create_dir_all(dir.join("middle_dir")).unwrap();

        let browser = AssetBrowser::new(dir.clone());
        // Dir should come first
        assert!(browser.entries[0].is_dir);
        assert_eq!(browser.entries[0].name, "middle_dir");
        // Then files alphabetically
        assert_eq!(browser.entries[1].name, "alpha.txt");
        assert_eq!(browser.entries[2].name, "zebra.txt");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_navigate_to() {
        let dir = make_temp_dir("nav");
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub").join("inner.txt"), "x").unwrap();

        let mut browser = AssetBrowser::new(dir.clone());
        browser.navigate_to("sub");
        assert!(browser.current.ends_with("sub"));
        assert_eq!(browser.entries.len(), 1);
        assert_eq!(browser.entries[0].name, "inner.txt");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_go_up() {
        let dir = make_temp_dir("goup");
        fs::create_dir_all(dir.join("child")).unwrap();

        let mut browser = AssetBrowser::new(dir.clone());
        browser.navigate_to("child");
        assert!(browser.current.ends_with("child"));

        browser.go_up();
        assert_eq!(browser.current, std::fs::canonicalize(&dir).unwrap());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_go_up_at_root() {
        let dir = make_temp_dir("goup_root");
        let browser_root = std::fs::canonicalize(&dir).unwrap();
        let mut browser = AssetBrowser::new(dir.clone());
        browser.go_up(); // should be no-op
        assert_eq!(browser.current, browser_root);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_root_guard() {
        let dir = make_temp_dir("guard");
        let mut browser = AssetBrowser::new(dir.clone());
        browser.navigate_to(".."); // should be rejected
        assert_eq!(browser.current, std::fs::canonicalize(&dir).unwrap());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
    }

    #[test]
    fn test_panel_draws_commands() {
        let dir = make_temp_dir("panel");
        fs::write(dir.join("test.txt"), "hello").unwrap();

        let mut browser = AssetBrowser::new(dir.clone());
        let mut ui = UiContext::new(800.0, 600.0);
        let rect = Rect { x: 0.0, y: 0.0, w: 300.0, h: 400.0 };

        ui.begin_frame(0.0, 0.0, false);
        asset_browser_panel(&mut ui, &mut browser, &rect);
        ui.end_frame();

        assert!(ui.draw_list.commands.len() > 0);

        let _ = fs::remove_dir_all(&dir);
    }
}
