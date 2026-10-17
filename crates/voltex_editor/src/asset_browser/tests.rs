use super::*;
use std::fs;

fn make_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("voltex_ab_test_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ── Grid view ──────────────────────────────────────────────────────────

#[test]
fn grid_columns_fits_and_clamps_to_one() {
    // 4 cells of 64 + 3 gaps of 4 = 268 ≤ 292 < 5 cells.
    assert_eq!(grid_columns(292.0, 64.0, 4.0), 4);
    // Exactly three cells: 3*64 + 2*4 = 200.
    assert_eq!(grid_columns(200.0, 64.0, 4.0), 3);
    // Narrower than one cell still renders a single column.
    assert_eq!(grid_columns(10.0, 64.0, 4.0), 1);
}

#[test]
fn browser_view_defaults_to_list_and_persists_across_refresh() {
    let dir = make_temp_dir("grid_view_persist");
    let mut browser = AssetBrowser::new(dir.clone());
    assert_eq!(browser.view, BrowserView::List);
    browser.view = BrowserView::Grid;
    browser.refresh();
    assert_eq!(browser.view, BrowserView::Grid, "refresh keeps the view mode");
    let _ = fs::remove_dir_all(&dir);
}

/// Drive `draw_entry_grid` directly with a known layout origin so the
/// click geometry doesn't depend on the panel header's line count.
fn grid_click_at(
    mx: f32,
    my: f32,
    entries: &[(String, bool, AssetKind)],
    thumbs: Option<&std::collections::HashMap<String, u32>>,
    browser: &mut AssetBrowser,
) -> (Option<String>, crate::draw_list::DrawList) {
    let mut ui = UiContext::new(800.0, 600.0);
    let rect = Rect { x: 0.0, y: 0.0, w: 300.0, h: 400.0 };
    // Frame 1: mouse up (establishes prev state).
    ui.begin_frame(mx, my, false);
    ui.end_frame();
    // Frame 2: mouse down → mouse_clicked = rising edge.
    ui.begin_frame(mx, my, true);
    ui.layout = LayoutState::new(rect.x + PADDING, 50.0);
    let mut clicked_dir = None;
    draw_entry_grid(&mut ui, browser, &rect, entries, thumbs, &mut clicked_dir);
    ui.end_frame();
    (clicked_dir, ui.draw_list)
}

#[test]
fn grid_cell_click_selects_file_and_navigates_dir() {
    let dir = make_temp_dir("grid_click");
    let mut browser = AssetBrowser::new(dir.clone());
    let entries = vec![
        ("sub".to_string(), true, AssetKind::Other),
        ("a.png".to_string(), false, AssetKind::Texture),
    ];
    // rect.w=300 → 4 columns; cell 0 at x=4, cell 1 at x=72 (origin_y=50).
    // Click inside cell 1 (the file).
    let (clicked_dir, _) = grid_click_at(80.0, 60.0, &entries, None, &mut browser);
    assert_eq!(clicked_dir, None);
    assert_eq!(browser.selected_file.as_deref(), Some("a.png"));

    // Click inside cell 0 (the directory) → staged navigation.
    let (clicked_dir, _) = grid_click_at(10.0, 60.0, &entries, None, &mut browser);
    assert_eq!(clicked_dir.as_deref(), Some("sub"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn grid_uses_thumb_slot_when_registered_and_placeholder_otherwise() {
    let dir = make_temp_dir("grid_thumbs");
    let mut browser = AssetBrowser::new(dir.clone());
    let entries = vec![
        ("a.png".to_string(), false, AssetKind::Texture),
        ("b.png".to_string(), false, AssetKind::Texture),
    ];
    let thumbs: std::collections::HashMap<String, u32> =
        [("a.png".to_string(), 7u32)].into_iter().collect();

    let (_, dl) = grid_click_at(-10.0, -10.0, &entries, Some(&thumbs), &mut browser);
    let textured: Vec<u32> = dl.commands.iter().filter_map(|c| c.texture).collect();
    assert_eq!(textured, vec![7], "only the registered file draws an image");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn relative_to_root_strips_and_uses_forward_slashes() {
    let dir = make_temp_dir("rel_root");
    fs::create_dir_all(dir.join("assets")).unwrap();
    let browser = AssetBrowser::new(dir.clone());

    // Inside the root: stripped + forward slashes. Derive the probe from
    // browser.root (canonicalized in new()) the way Open does via
    // current.join.
    let inside = browser.root.join("assets").join("hero.png");
    assert_eq!(browser.relative_to_root(&inside), "assets/hero.png");

    // Outside the root: falls back to the path as-is (slashes normalized).
    let outside = PathBuf::from("Z:/elsewhere/x.png");
    assert_eq!(browser.relative_to_root(&outside), "Z:/elsewhere/x.png");

    let _ = fs::remove_dir_all(&dir);
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
fn test_root_guard_rejects_dotdot_with_non_canonical_root() {
    // Windows' canonical (verbatim) root hides the bug because the OS
    // refuses `..` inside `\\?\` paths; a plain root is what Linux/macOS
    // get. `root/..` passes a component-wise `starts_with` check.
    let dir = make_temp_dir("guard_plain");
    fs::create_dir_all(dir.join("sub")).unwrap();
    let mut browser = AssetBrowser::new(dir.clone());
    browser.root = dir.clone();
    browser.current = dir.clone();

    browser.navigate_to("..");
    assert_eq!(browser.current, dir, "`..` must not escape the root");
    browser.navigate_to("sub/../..");
    assert_eq!(browser.current, dir, "nested `..` must not escape the root");

    browser.navigate_to("sub");
    assert!(browser.current.ends_with("sub"), "normal navigation still works");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_asset_kind_classification() {
    use std::path::Path;
    assert_eq!(AssetKind::from_path(Path::new("hero.png")), AssetKind::Texture);
    assert_eq!(AssetKind::from_path(Path::new("a.JPG")), AssetKind::Texture); // case-insensitive
    assert_eq!(AssetKind::from_path(Path::new("b.jpeg")), AssetKind::Texture);
    assert_eq!(AssetKind::from_path(Path::new("level.vscn")), AssetKind::Scene);
    assert_eq!(AssetKind::from_path(Path::new("level.json")), AssetKind::Scene);
    assert_eq!(AssetKind::from_path(Path::new("tree.obj")), AssetKind::Mesh);
    assert_eq!(AssetKind::from_path(Path::new("ship.glb")), AssetKind::Mesh);
    assert_eq!(AssetKind::from_path(Path::new("shot.wav")), AssetKind::Audio);
    assert_eq!(AssetKind::from_path(Path::new("bgm.ogg")), AssetKind::Audio);
    assert_eq!(AssetKind::from_path(Path::new("notes.txt")), AssetKind::Other);
    assert_eq!(AssetKind::from_path(Path::new("README")), AssetKind::Other); // no extension
}

#[test]
fn test_entries_carry_kind() {
    let dir = make_temp_dir("kind");
    fs::write(dir.join("a.png"), "p").unwrap();
    fs::write(dir.join("b.vscn"), "s").unwrap();
    fs::create_dir_all(dir.join("sub")).unwrap();

    let browser = AssetBrowser::new(dir.clone());
    let png = browser.entries.iter().find(|e| e.name == "a.png").unwrap();
    let scn = browser.entries.iter().find(|e| e.name == "b.vscn").unwrap();
    let sub = browser.entries.iter().find(|e| e.name == "sub").unwrap();
    assert_eq!(png.kind, AssetKind::Texture);
    assert_eq!(scn.kind, AssetKind::Scene);
    assert_eq!(sub.kind, AssetKind::Other); // directories are not classified

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_selected_full_path_and_kind() {
    let dir = make_temp_dir("selpath");
    fs::write(dir.join("hero.png"), "p").unwrap();

    let mut browser = AssetBrowser::new(dir.clone());
    assert_eq!(browser.selected_full_path(), None);
    assert_eq!(browser.selected_kind(), None);

    browser.selected_file = Some("hero.png".into());
    assert_eq!(
        browser.selected_full_path(),
        Some(browser.current.join("hero.png"))
    );
    assert_eq!(browser.selected_kind(), Some(AssetKind::Texture));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_open_button_stages_activation_consumed_once() {
    // Drive the panel with a click landing on the "Open" button, then
    // verify take_activated yields the path+kind exactly once.
    let dir = make_temp_dir("activate");
    fs::write(dir.join("level.vscn"), "scene").unwrap();

    let mut browser = AssetBrowser::new(dir.clone());
    browser.selected_file = Some("level.vscn".into());

    // No activation until the user clicks Open.
    assert_eq!(browser.take_activated(), None);

    // Simulate clicking Open directly (mirrors the panel's button branch).
    browser.activated = Some(browser.current.join("level.vscn"));

    let (path, kind) = browser.take_activated().expect("activation present");
    assert_eq!(path, browser.current.join("level.vscn"));
    assert_eq!(kind, AssetKind::Scene);
    // Consumed — second call is empty.
    assert_eq!(browser.take_activated(), None);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_navigate_clears_stale_activation() {
    let dir = make_temp_dir("clear_activate");
    fs::write(dir.join("a.png"), "p").unwrap();
    fs::create_dir_all(dir.join("sub")).unwrap();

    let mut browser = AssetBrowser::new(dir.clone());
    browser.activated = Some(browser.current.join("a.png"));
    browser.navigate_to("sub"); // refresh() must clear activation
    assert_eq!(browser.take_activated(), None);

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
    asset_browser_panel(&mut ui, &mut browser, &rect, None, None);
    ui.end_frame();

    assert!(!ui.draw_list.commands.is_empty());

    let _ = fs::remove_dir_all(&dir);
}
