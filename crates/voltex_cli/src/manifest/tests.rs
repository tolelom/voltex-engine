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
    assert!(
        matches!(
            err,
            CliError::MissingField {
                section: "project",
                field: "name"
            }
        ),
        "{err:?}"
    );
}

#[test]
fn missing_version_returns_missing_field() {
    let toml = r#"
[project]
name = "x"
"#;
    let err = Manifest::parse(toml).unwrap_err();
    assert!(
        matches!(
            err,
            CliError::MissingField {
                section: "project",
                field: "version"
            }
        ),
        "{err:?}"
    );
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
    // Line 5 (the empty first line of the raw string is line 1); `[` is the 11th character.
    assert!(
        matches!(err, CliError::ManifestParse { line: 5, column: 11, .. }),
        "{err:?}"
    );
}

/// (line, column) of the parse error for a one-line document.
fn parse_error_at(text: &str) -> (usize, usize) {
    match parse_toml(text) {
        Err(CliError::ManifestParse { line, column, .. }) => (line, column),
        other => panic!("expected ManifestParse, got {other:?}"),
    }
}

#[test]
fn parse_errors_point_at_the_offending_column() {
    // Unrecognised bare value, after indentation: `foo` starts at column 10.
    assert_eq!(parse_error_at("  name = foo"), (1, 10));
    // Bad array element: `abc` starts at column 22.
    assert_eq!(parse_error_at("window_size = [1280, abc]"), (1, 22));
    // Missing `]` on a header points one past the end of the line.
    assert_eq!(parse_error_at("[build"), (1, 7));
    // No `=`: the line start.
    assert_eq!(parse_error_at("just words"), (1, 1));
    // Empty key: the `=`.
    assert_eq!(parse_error_at("  = 3"), (1, 3));
}

#[test]
fn parse_error_column_counts_characters_not_bytes() {
    // 제목 is 2 characters but 6 bytes; `foo` is character 6.
    assert_eq!(parse_error_at("제목 = foo"), (1, 6));
}

#[test]
fn parse_error_display_includes_line_and_column() {
    let err = parse_toml("x = foo").unwrap_err();
    assert_eq!(
        err.to_string(),
        "voltex.toml parse error on line 1, column 5: unrecognised value `foo`"
    );
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

// ── v1.0.11: Schema validation (unknown keys + sections) ───────────────────

#[test]
fn unknown_key_in_project_is_rejected() {
    let toml = r#"
[project]
name = "g"
version = "0.1.0"
typo_field = "oops"
"#;
    let err = Manifest::parse(toml).unwrap_err();
    match err {
        CliError::UnknownField { section, field, .. } => {
            assert_eq!(section, "project");
            assert_eq!(field, "typo_field");
        }
        other => panic!("expected UnknownField, got {:?}", other),
    }
}

#[test]
fn unknown_key_in_build_includes_suggestion_for_typo() {
    // `windows_title` is 1 edit away from `window_title` (extra s).
    let toml = r#"
[project]
name = "g"
version = "0.1.0"
[build]
windows_title = "Game"
"#;
    let err = Manifest::parse(toml).unwrap_err();
    match err {
        CliError::UnknownField {
            field, suggestion, ..
        } => {
            assert_eq!(field, "windows_title");
            assert_eq!(suggestion, Some("window_title"));
        }
        other => panic!("expected UnknownField, got {:?}", other),
    }
}

#[test]
fn unknown_key_with_unrelated_name_omits_suggestion() {
    // `xyzzy_unrelated_42` has Levenshtein distance > 3 from every valid
    // key — we should not invent a misleading suggestion.
    let toml = r#"
[project]
name = "g"
version = "0.1.0"
[build]
xyzzy_unrelated_42 = "x"
"#;
    let err = Manifest::parse(toml).unwrap_err();
    match err {
        CliError::UnknownField { suggestion, .. } => assert!(suggestion.is_none()),
        other => panic!("expected UnknownField, got {:?}", other),
    }
}

#[test]
fn unknown_section_is_rejected() {
    let toml = r#"
[project]
name = "g"
version = "0.1.0"
[deploy]
url = "https://example.com"
"#;
    let err = Manifest::parse(toml).unwrap_err();
    match err {
        CliError::UnknownSection { name, .. } => assert_eq!(name, "deploy"),
        other => panic!("expected UnknownSection, got {:?}", other),
    }
}

#[test]
fn build_subsection_for_arbitrary_triple_is_accepted() {
    // Triples are open-ended (e.g. windows-x64, wasm32-unknown-unknown,
    // user's own triple) — we must not reject [build.<anything>].
    let toml = r#"
[project]
name = "g"
version = "0.1.0"
[build.aarch64-linux-android]
icon = "android.png"
"#;
    let m = Manifest::parse(toml).unwrap();
    assert!(m.target_overrides.contains_key("aarch64-linux-android"));
}

// ── v1.0.11: validate_paths ────────────────────────────────────────────────

#[test]
fn kind_defaults_to_scene_and_parses_rust() {
    let m = Manifest::parse(MINIMAL).unwrap();
    assert_eq!(m.build.kind, BuildKind::Scene);

    let m = Manifest::parse(
        r#"
[project]
name = "g"
version = "0.1.0"
[build]
kind = "rust"
"#,
    )
    .unwrap();
    assert_eq!(m.build.kind, BuildKind::Rust);
    // A Rust-logic game has no Lua wrapper scripts — the scripts default must
    // be empty so validate_paths doesn't demand a scripts/ dir.
    assert!(m.build.scripts.is_empty(), "rust kind defaults scripts to []");
    // Assets likewise: procedural games may have no assets/ dir at all —
    // the default glob must not force one into existence.
    assert!(m.build.assets.is_empty(), "rust kind defaults assets to []");

    // Explicit "scene" also accepted.
    let m = Manifest::parse(
        r#"
[project]
name = "g"
version = "0.1.0"
[build]
kind = "scene"
"#,
    )
    .unwrap();
    assert_eq!(m.build.kind, BuildKind::Scene);
}

#[test]
fn kind_with_unknown_value_is_rejected() {
    let err = Manifest::parse(
        r#"
[project]
name = "g"
version = "0.1.0"
[build]
kind = "cpp"
"#,
    )
    .unwrap_err();
    match err {
        CliError::InvalidValue { field, .. } => assert_eq!(field, "kind"),
        other => panic!("expected InvalidValue, got {:?}", other),
    }
}

#[test]
fn validate_paths_skips_scene_for_rust_kind() {
    // A Rust-logic game has no scene.vscn — `kind = "rust"` must not demand
    // one, while still validating the assets dirs it will bundle.
    let tmp = std::env::temp_dir().join(format!(
        "voltex_cli_validate_paths_rust_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    write_manifest_to_dir(
        &tmp,
        r#"
[project]
name = "g"
version = "0.1.0"
[build]
kind = "rust"
scripts = ["scripts/**/*.lua"]
assets = ["assets/**/*"]
"#,
    );
    std::fs::create_dir_all(tmp.join("scripts")).unwrap();
    std::fs::create_dir_all(tmp.join("assets")).unwrap();

    let m = Manifest::load_from_dir(&tmp).unwrap();
    m.validate_paths(&tmp).expect("rust kind must not require build.scene");

    // Assets dirs are still validated.
    std::fs::remove_dir_all(tmp.join("assets")).unwrap();
    let err = m.validate_paths(&tmp).unwrap_err();
    match err {
        CliError::ManifestPathMissing { field, .. } => assert_eq!(field, "build.assets"),
        other => panic!("expected ManifestPathMissing, got {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

fn write_manifest_to_dir(dir: &std::path::Path, body: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("voltex.toml"), body).unwrap();
}

#[test]
fn validate_paths_passes_when_all_present() {
    let tmp = std::env::temp_dir().join(format!(
        "voltex_cli_validate_paths_ok_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    write_manifest_to_dir(
        &tmp,
        r#"
[project]
name = "g"
version = "0.1.0"
[build]
scene = "scene.vscn"
scripts = ["scripts/**/*.lua"]
assets = ["assets/**/*"]
"#,
    );
    std::fs::write(tmp.join("scene.vscn"), "").unwrap();
    std::fs::create_dir_all(tmp.join("scripts")).unwrap();
    std::fs::create_dir_all(tmp.join("assets")).unwrap();

    let m = Manifest::load_from_dir(&tmp).unwrap();
    m.validate_paths(&tmp).unwrap();

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn validate_paths_rejects_missing_scene() {
    let tmp = std::env::temp_dir().join(format!(
        "voltex_cli_validate_paths_no_scene_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    write_manifest_to_dir(
        &tmp,
        r#"
[project]
name = "g"
version = "0.1.0"
[build]
scene = "missing.vscn"
scripts = ["scripts/**/*.lua"]
assets = ["assets/**/*"]
"#,
    );
    std::fs::create_dir_all(tmp.join("scripts")).unwrap();
    std::fs::create_dir_all(tmp.join("assets")).unwrap();

    let m = Manifest::load_from_dir(&tmp).unwrap();
    let err = m.validate_paths(&tmp).unwrap_err();
    match err {
        CliError::ManifestPathMissing { field, .. } => assert_eq!(field, "build.scene"),
        other => panic!("expected ManifestPathMissing, got {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn validate_paths_rejects_missing_scripts_dir() {
    let tmp = std::env::temp_dir().join(format!(
        "voltex_cli_validate_paths_no_scripts_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    write_manifest_to_dir(
        &tmp,
        r#"
[project]
name = "g"
version = "0.1.0"
[build]
scene = "scene.vscn"
scripts = ["script/**/*.lua"]
assets = ["assets/**/*"]
"#,
    );
    std::fs::write(tmp.join("scene.vscn"), "").unwrap();
    std::fs::create_dir_all(tmp.join("assets")).unwrap();
    // Note: missing "script" (typo for "scripts") dir.

    let m = Manifest::load_from_dir(&tmp).unwrap();
    let err = m.validate_paths(&tmp).unwrap_err();
    match err {
        CliError::ManifestPathMissing { field, .. } => assert_eq!(field, "build.scripts"),
        other => panic!("expected ManifestPathMissing, got {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn levenshtein_smoke_test() {
    assert_eq!(levenshtein("kitten", "sitting"), 3);
    assert_eq!(levenshtein("scene", "scene"), 0);
    assert_eq!(levenshtein("scrips", "scripts"), 1);
    assert_eq!(levenshtein("Window_Title", "window_title"), 0); // case-insensitive
}

#[test]
fn closest_match_returns_none_when_too_far() {
    assert_eq!(closest_match("totally_unrelated", VALID_BUILD_KEYS), None);
    assert_eq!(closest_match("scrips", VALID_BUILD_KEYS), Some("scripts"));
}

#[test]
fn unknown_field_display_includes_hint() {
    let err = CliError::UnknownField {
        section: "build".into(),
        field: "windows_title".into(),
        valid: VALID_BUILD_KEYS.to_vec(),
        suggestion: Some("window_title"),
    };
    let s = format!("{}", err);
    assert!(s.contains("windows_title"));
    assert!(s.contains("[build]"));
    assert!(s.contains("did you mean `window_title`"));
}

#[test]
fn unknown_section_display_lists_valid() {
    let err = CliError::UnknownSection {
        name: "deploy".into(),
        valid_prefixes: VALID_SECTION_PREFIXES.to_vec(),
    };
    let s = format!("{}", err);
    assert!(s.contains("deploy"));
    assert!(s.contains("valid sections"));
}

#[test]
fn manifest_path_missing_display_shows_path_and_base() {
    let err = CliError::ManifestPathMissing {
        field: "build.scene",
        path: PathBuf::from("/tmp/missing.vscn"),
        base_dir: PathBuf::from("/tmp"),
    };
    let s = format!("{}", err);
    assert!(s.contains("build.scene"));
    assert!(s.contains("missing.vscn"));
    assert!(s.contains("/tmp"));
}
