use super::*;

const MINIMAL: &str = r#"
[project]
name = "my_game"
version = "0.1.0"
"#;

const FULL: &str = r#"
# Top comment
[project]
name = "my_game"   # inline comment
version = "0.1.0"

[build]
scene = "level1.vscn"
scripts = ["scripts/**/*.lua", "extra/*.lua"]
assets = ["assets/**/*"]
window_title = "My Game"
window_size = [1920, 1080]
targets = ["windows-x64", "linux-x64"]

[build.windows-x64]
icon = "assets/icon.ico"
"#;

#[test]
fn parses_minimal_manifest() {
    let m = Manifest::parse(MINIMAL).unwrap();
    assert_eq!(m.project.name, "my_game");
    assert_eq!(m.project.version, "0.1.0");
    assert_eq!(m.build.scene, "scene.vscn");
    assert_eq!(m.build.window_size, (1280, 720));
    assert_eq!(m.build.window_title, "my_game");
    assert_eq!(m.build.scripts, vec!["scripts/**/*.lua"]);
    assert_eq!(m.target_overrides.len(), 0);
}

#[test]
fn parses_full_manifest() {
    let m = Manifest::parse(FULL).unwrap();
    assert_eq!(m.project.name, "my_game");
    assert_eq!(m.build.scene, "level1.vscn");
    assert_eq!(m.build.window_title, "My Game");
    assert_eq!(m.build.window_size, (1920, 1080));
    assert_eq!(m.build.scripts.len(), 2);
    assert_eq!(m.build.assets, vec!["assets/**/*"]);
    assert_eq!(m.build.targets, vec!["windows-x64", "linux-x64"]);
    let win = m.target_overrides.get("windows-x64").unwrap();
    assert_eq!(win.icon.as_deref(), Some("assets/icon.ico"));
}

#[test]
fn missing_name_returns_missing_field() {
    let toml = r#"
[project]
version = "0.1.0"
"#;
    let err = Manifest::parse(toml).unwrap_err();
    matches!(err, CliError::MissingField { section: "project", field: "name" });
}

#[test]
fn missing_version_returns_missing_field() {
    let toml = r#"
[project]
name = "x"
"#;
    let err = Manifest::parse(toml).unwrap_err();
    matches!(err, CliError::MissingField { section: "project", field: "version" });
}

#[test]
fn invalid_window_size_array_returns_error() {
    let toml = r#"
[project]
name = "x"
version = "0.1.0"

[build]
window_size = [1280, 720, 30]
"#;
    let err = Manifest::parse(toml).unwrap_err();
    match err {
        CliError::InvalidValue { field, .. } => assert_eq!(field, "window_size"),
        other => panic!("unexpected error: {:?}", other),
    }
}

#[test]
fn array_of_strings_parses() {
    let toml = r#"
[project]
name = "x"
version = "0.1.0"

[build]
scripts = ["a.lua", "b.lua", "c.lua"]
"#;
    let m = Manifest::parse(toml).unwrap();
    assert_eq!(m.build.scripts, vec!["a.lua", "b.lua", "c.lua"]);
}

#[test]
fn comments_and_blank_lines_are_skipped() {
    let toml = r#"
# Header comment

[project]   # section comment
# inner comment
name = "ok"  # trailing comment
version = "0.1.0"
"#;
    let m = Manifest::parse(toml).unwrap();
    assert_eq!(m.project.name, "ok");
}

#[test]
fn subsection_header_parses() {
    let toml = r#"
[project]
name = "x"
version = "0.1.0"

[build.linux-x64]
icon = "linux.png"
"#;
    let m = Manifest::parse(toml).unwrap();
    let linux = m.target_overrides.get("linux-x64").unwrap();
    assert_eq!(linux.icon.as_deref(), Some("linux.png"));
}

#[test]
fn unterminated_array_is_parse_error() {
    let toml = r#"
[project]
name = "x"
version = "0.1.0"
scripts = ["a.lua"
"#;
    let err = Manifest::parse(toml).unwrap_err();
    matches!(err, CliError::ManifestParse { .. });
}

#[test]
fn strip_glob_returns_dir_prefix() {
    assert_eq!(strip_glob("scripts/**/*.lua"), "scripts");
    assert_eq!(strip_glob("assets/*"), "assets");
    assert_eq!(strip_glob("plain"), "plain");
}

#[test]
fn host_triple_matches_current_os() {
    let t = host_triple();
    let expected_prefix = match std::env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "macos",
        _ => "windows",
    };
    assert!(t.starts_with(expected_prefix), "got {}", t);
}
