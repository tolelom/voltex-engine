//! Asset browser panel — filesystem tree view with click-to-navigate folders.
//!
//! State fields (current path, selection, expanded entries) are `pub` so
//! tests can introspect; we allow `missing_docs` and keep the narrative in
//! the [`asset_browser_panel`] entry point.
#![allow(missing_docs)]

use std::path::{Path, PathBuf};
use crate::ui_context::UiContext;
use crate::dock::Rect;
use crate::layout::LayoutState;

const COLOR_SELECTED: [u8; 4] = [0x44, 0x66, 0x88, 0xFF];
const COLOR_TEXT: [u8; 4] = [0xEE, 0xEE, 0xEE, 0xFF];
const COLOR_CELL_BG: [u8; 4] = [0x30, 0x30, 0x38, 0xFF];
const PADDING: f32 = 4.0;
/// Edge length of the selected-file preview thumbnail, in pixels.
const PREVIEW_SIZE: f32 = 96.0;
/// Edge length of one grid-view thumbnail cell, in pixels.
const GRID_CELL: f32 = 64.0;

/// Coarse classification of a file by extension, used to decide what
/// "Open" does in the integration layer (load a scene, apply a texture, …).
/// Pure / GPU-free so it can be unit-tested without an asset system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    /// `.png / .jpg / .jpeg` — a texture to load and apply to an entity.
    Texture,
    /// `.vscn / .json` — a scene to deserialize into the world.
    Scene,
    /// `.obj / .gltf / .glb` — a mesh (import not yet wired).
    Mesh,
    /// `.wav / .ogg` — an audio clip (import not yet wired).
    Audio,
    /// Anything else.
    Other,
}

impl AssetKind {
    /// Classify by lowercased file extension. Unknown / missing extension → `Other`.
    pub fn from_path(path: &Path) -> AssetKind {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase());
        match ext.as_deref() {
            Some("png" | "jpg" | "jpeg") => AssetKind::Texture,
            Some("vscn" | "json") => AssetKind::Scene,
            Some("obj" | "gltf" | "glb") => AssetKind::Mesh,
            Some("wav" | "ogg") => AssetKind::Audio,
            _ => AssetKind::Other,
        }
    }

    /// Single-character prefix shown in the browser list (`[T] [S] [M] [A]`,
    /// `Other` has none so it aligns with the existing blank-prefix files).
    fn list_prefix(self) -> &'static str {
        match self {
            AssetKind::Texture => "[T] ",
            AssetKind::Scene => "[S] ",
            AssetKind::Mesh => "[M] ",
            AssetKind::Audio => "[A] ",
            AssetKind::Other => "    ",
        }
    }
}

pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub kind: AssetKind,
}

/// The per-row fields the list/grid drawers need, copied out of
/// [`DirEntry`] for one frame.
struct EntryRow {
    name: String,
    is_dir: bool,
    kind: AssetKind,
}

/// How [`asset_browser_panel`] lays out the current directory's entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserView {
    /// One line per entry (the original layout).
    #[default]
    List,
    /// Thumbnail cells ([`GRID_CELL`]px) in as many columns as fit.
    Grid,
}

/// Number of grid columns that fit in `avail_w`, with `pad` between cells.
/// Always at least 1 so a too-narrow panel degrades to a single column.
pub fn grid_columns(avail_w: f32, cell_w: f32, pad: f32) -> usize {
    (((avail_w + pad) / (cell_w + pad)).floor() as usize).max(1)
}

pub struct AssetBrowser {
    pub root: PathBuf,
    pub current: PathBuf,
    pub entries: Vec<DirEntry>,
    pub selected_file: Option<String>,
    /// Current entry layout (list / thumbnail grid). Toggled from the panel,
    /// persists across navigation.
    pub view: BrowserView,
    /// Set when the user clicks "Open" on the selected file. Consumed by the
    /// integration layer via [`AssetBrowser::take_activated`]. Cleared whenever
    /// the directory changes so a stale activation can't leak across folders.
    activated: Option<PathBuf>,
}

pub(crate) fn format_size(bytes: u64) -> String {
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
            view: BrowserView::default(),
            activated: None,
        };
        browser.refresh();
        browser
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        self.selected_file = None;
        self.activated = None;
        if let Ok(read_dir) = std::fs::read_dir(&self.current) {
            for entry in read_dir.flatten() {
                let meta = entry.metadata().ok();
                let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
                let size = meta.as_ref().map_or(0, |m| m.len());
                let name = entry.file_name().to_string_lossy().to_string();
                let kind = if is_dir {
                    AssetKind::Other
                } else {
                    AssetKind::from_path(Path::new(&name))
                };
                self.entries.push(DirEntry { name, is_dir, size, kind });
            }
        }
        self.entries.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
        });
    }

    /// Enter `dir_name` under the current directory. The joined path is
    /// canonicalized before the root check — a component-wise `starts_with`
    /// accepts `root/..`, so `..` segments must be resolved first.
    pub fn navigate_to(&mut self, dir_name: &str) {
        let Ok(target) = std::fs::canonicalize(self.current.join(dir_name)) else {
            return;
        };
        let root = std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
        if target.starts_with(&root) && target.is_dir() {
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

    /// `path` relative to the browser root, with forward slashes — the
    /// canonical form for scene-persisted asset references (portable across
    /// machines and into `.vpkg` bundles; relative fs reads resolve against
    /// the CWD, which integration layers use as the browser root). Falls back
    /// to the path as-is when it lies outside the root (cannot happen through
    /// browser navigation — defensive).
    pub fn relative_to_root(&self, path: &Path) -> String {
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        rel.to_string_lossy().replace('\\', "/")
    }

    pub fn relative_path(&self) -> String {
        self.current
            .strip_prefix(&self.root)
            .unwrap_or(&self.current)
            .to_string_lossy()
            .to_string()
    }

    /// Absolute path of the currently selected file (`current` + selected name),
    /// or `None` if nothing is selected.
    pub fn selected_full_path(&self) -> Option<PathBuf> {
        self.selected_file.as_ref().map(|name| self.current.join(name))
    }

    /// Kind of the currently selected file, or `None` if nothing is selected.
    pub fn selected_kind(&self) -> Option<AssetKind> {
        let name = self.selected_file.as_deref()?;
        self.entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.kind)
    }

    /// Consume a pending "Open" activation. Returns the absolute path and the
    /// classified kind exactly once; subsequent calls return `None` until the
    /// user opens another file.
    pub fn take_activated(&mut self) -> Option<(PathBuf, AssetKind)> {
        let path = self.activated.take()?;
        let kind = AssetKind::from_path(&path);
        Some((path, kind))
    }
}

pub fn asset_browser_panel(
    ui: &mut UiContext,
    browser: &mut AssetBrowser,
    rect: &Rect,
    preview: Option<u32>,
    thumbs: Option<&std::collections::HashMap<String, u32>>,
) {
    ui.layout = LayoutState::new(rect.x + PADDING, rect.y + PADDING);

    let path_text = format!("Path: /{}", browser.relative_path());
    ui.text(&path_text);

    // View toggle — label shows the layout the click switches TO.
    let toggle_label = match browser.view {
        BrowserView::List => "[Grid]",
        BrowserView::Grid => "[List]",
    };
    if ui.button(toggle_label) {
        browser.view = match browser.view {
            BrowserView::List => BrowserView::Grid,
            BrowserView::Grid => BrowserView::List,
        };
    }

    // Go up button
    if browser.current != browser.root
        && ui.button("[..]") {
            browser.go_up();
            return;
        }

    // Snapshot the rows so the draw helpers can mutate `browser` (selection)
    // while iterating; a directory click is applied after drawing.
    let mut clicked_dir: Option<String> = None;
    let entries_snapshot: Vec<EntryRow> = browser.entries.iter()
        .map(|e| EntryRow { name: e.name.clone(), is_dir: e.is_dir, kind: e.kind })
        .collect();

    match browser.view {
        BrowserView::List => {
            draw_entry_list(ui, browser, rect, &entries_snapshot, &mut clicked_dir)
        }
        BrowserView::Grid => {
            draw_entry_grid(ui, browser, rect, &entries_snapshot, thumbs, &mut clicked_dir)
        }
    }

    if let Some(dir) = clicked_dir {
        browser.navigate_to(&dir);
    }

    // File info + Open action
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
        // Thumbnail — the integration layer decodes/uploads the selected
        // texture and passes its `UiRenderer::register_texture` slot in
        // (this panel stays GPU-free, same split as Open handling).
        if let Some(slot) = preview {
            ui.image(slot, PREVIEW_SIZE, PREVIEW_SIZE);
        }
        // Clicking "Open" stages an activation for the integration layer to
        // consume (load scene / apply texture). Cleared on directory change.
        if ui.button("Open") {
            browser.activated = Some(browser.current.join(file_name));
        }
    }
}

/// One line per entry (the original layout).
fn draw_entry_list(
    ui: &mut UiContext,
    browser: &mut AssetBrowser,
    rect: &Rect,
    entries: &[EntryRow],
    clicked_dir: &mut Option<String>,
) {
    let gh = ui.font.glyph_height as f32;
    let line_h = gh + PADDING;

    for EntryRow { name, is_dir, kind } in entries {
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
                *clicked_dir = Some(name.clone());
            } else {
                browser.selected_file = Some(name.clone());
            }
        }

        // Label — directories get [D], files get a kind prefix ([T]/[S]/…).
        let label = if *is_dir {
            format!("[D] {}", name)
        } else {
            format!("{}{}", kind.list_prefix(), name)
        };

        let text_y = y + (line_h - gh) * 0.5;
        ui.draw_list.add_text(&ui.font, &label, x, text_y, COLOR_TEXT);

        ui.layout.cursor_y += line_h;
    }
}

/// Thumbnail-cell grid: [`GRID_CELL`]px image (or kind placeholder) + one
/// truncated label line per entry. Clicking a file cell selects it; clicking
/// a directory cell stages navigation (same semantics as the list).
fn draw_entry_grid(
    ui: &mut UiContext,
    browser: &mut AssetBrowser,
    rect: &Rect,
    entries: &[EntryRow],
    thumbs: Option<&std::collections::HashMap<String, u32>>,
    clicked_dir: &mut Option<String>,
) {
    let gw = ui.font.glyph_width as f32;
    let gh = ui.font.glyph_height as f32;
    let cell_w = GRID_CELL;
    let cell_h = GRID_CELL + gh + PADDING;
    let avail_w = (rect.w - 2.0 * PADDING).max(cell_w);
    let cols = grid_columns(avail_w, cell_w, PADDING);
    let origin_x = rect.x + PADDING;
    let origin_y = ui.layout.cursor_y;
    let label_chars = (cell_w / gw).floor() as usize;

    for (i, EntryRow { name, is_dir, kind }) in entries.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let x = origin_x + col as f32 * (cell_w + PADDING);
        let y = origin_y + row as f32 * (cell_h + PADDING);

        // Cell background — selection highlight wins over the neutral bg.
        let selected =
            !is_dir && browser.selected_file.as_deref() == Some(name.as_str());
        let bg = if selected { COLOR_SELECTED } else { COLOR_CELL_BG };
        ui.draw_list.add_rect(x, y, cell_w, cell_h, bg);

        // Thumbnail area: registered slot → image, otherwise a kind/[D] tag
        // roughly centered in the square.
        match thumbs.and_then(|m| m.get(name.as_str())) {
            Some(&slot) if !is_dir => {
                ui.draw_list.add_image(x, y, cell_w, cell_w, slot);
            }
            _ => {
                let tag = if *is_dir { "[D]" } else { kind.list_prefix().trim() };
                let tx = x + (cell_w - tag.chars().count() as f32 * gw) * 0.5;
                let ty = y + (cell_w - gh) * 0.5;
                ui.draw_list.add_text(&ui.font, tag, tx, ty, COLOR_TEXT);
            }
        }

        // Label line under the thumbnail, truncated to the cell width.
        let label: String = name.chars().take(label_chars).collect();
        let ly = y + GRID_CELL + PADDING * 0.5;
        ui.draw_list.add_text(&ui.font, &label, x, ly, COLOR_TEXT);

        // Click
        if ui.mouse_clicked && ui.mouse_in_rect(x, y, cell_w, cell_h) {
            if *is_dir {
                *clicked_dir = Some(name.clone());
            } else {
                browser.selected_file = Some(name.clone());
            }
        }
    }

    // Advance the layout cursor past the grid so File Info renders below.
    if !entries.is_empty() {
        let rows = entries.len().div_ceil(cols);
        ui.layout.cursor_y = origin_y + rows as f32 * (cell_h + PADDING);
    }
}

#[cfg(test)]
mod tests;
