use super::wrapper::render_wrapper_main;
use super::*;
use crate::manifest::{BuildConfig, BuildKind, Manifest, ProjectInfo};
use std::collections::HashMap;

fn dummy(window_title: &str, w: u32, h: u32) -> Manifest {
    Manifest {
        project: ProjectInfo {
            name: "demo".into(),
            version: "0.1.0".into(),
        },
        build: BuildConfig {
            kind: BuildKind::Scene,
            scene: "scene.vscn".into(),
            scripts: vec!["scripts/**/*.lua".into()],
            assets: vec!["assets/**/*".into()],
            window_title: window_title.into(),
            window_size: (w, h),
            targets: vec!["windows-x64".into()],
        },
        target_overrides: HashMap::new(),
    }
}

#[test]
fn extract_executables_parses_cargo_json_messages() {
    // Two compiler-artifact lines (dep without executable, bin with one),
    // Windows-style escaped path, plus noise lines that must be ignored.
    let out = concat!(
        r#"{"reason":"compiler-artifact","target":{"name":"dep"},"executable":null}"#,
        "\n",
        r#"{"reason":"compiler-artifact","target":{"name":"game"},"executable":"E:\\proj\\target\\release\\game.exe"}"#,
        "\n",
        r#"{"reason":"build-finished","success":true}"#,
        "\n",
    );
    let exes = extract_executables(out);
    assert_eq!(exes, vec![PathBuf::from(r"E:\proj\target\release\game.exe")]);

    // Unix-style path without escapes.
    let out = r#"{"reason":"compiler-artifact","executable":"/tmp/target/release/game"}"#;
    assert_eq!(extract_executables(out), vec![PathBuf::from("/tmp/target/release/game")]);

    // No artifacts → empty.
    assert!(extract_executables("garbage\n").is_empty());
}

/// `t/release/<name>` with the host's executable suffix.
fn release_exe(name: &str) -> PathBuf {
    Path::new("t")
        .join("release")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

#[test]
fn pick_executable_prefers_project_name_match() {
    let exes = vec![
        release_exe("depths"),
        release_exe("gen_sfx"),
    ];
    // Name match wins regardless of artifact order.
    assert_eq!(pick_executable(&exes, "depths").unwrap(), &exes[0]);
}

#[test]
fn pick_executable_single_unmatched_artifact_is_shipped() {
    // [[bin]] renamed away from project.name — the only candidate is
    // still unambiguous.
    let exes = vec![release_exe("game_main")];
    assert_eq!(pick_executable(&exes, "mygame").unwrap(), &exes[0]);
}

#[test]
fn pick_executable_errors_when_no_name_match_among_several() {
    // Guessing here could ship an asset generator as the game.
    let exes = vec![
        release_exe("game_main"),
        release_exe("gen_sfx"),
    ];
    match pick_executable(&exes, "mygame") {
        Err(CliError::AmbiguousExecutable { project, candidates }) => {
            assert_eq!(project, "mygame");
            assert_eq!(candidates, exes);
        }
        other => panic!("expected AmbiguousExecutable, got {other:?}"),
    }
}

#[test]
fn pick_executable_errors_when_nothing_built() {
    assert!(matches!(pick_executable(&[], "x"), Err(CliError::Io(_))));
}

#[test]
fn wrapper_main_includes_window_title_from_manifest() {
    let m = dummy("Hello \"World\"", 800, 600);
    let body = render_wrapper_main(&m);
    // Title is escaped properly.
    assert!(body.contains(r#"with_window_title("Hello \"World\"")"#));
    assert!(body.contains("with_window_size(800, 600)"));
    assert!(body.contains("with_initial_scene"));
    assert!(body.contains("include_bytes!"));
    assert!(body.contains("with_vfs"));
    // Shipped builds always run the audio stage (AudioPath resolve +
    // script-spawned sources live behind it).
    assert!(body.contains("with_audio(Vec::new())"));
}

#[test]
fn pack_archive_collects_scripts_and_assets() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("voltex_cli_pack_test_{}", stamp));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    std::fs::create_dir_all(root.join("assets").join("sub")).unwrap();
    std::fs::write(root.join("scene.vscn"), "scene-bytes").unwrap();
    std::fs::write(root.join("scripts").join("a.lua"), "a").unwrap();
    std::fs::write(root.join("assets").join("sub").join("b.png"), "b").unwrap();

    let m = dummy("X", 100, 100);
    let archive_path = root.join("out.vpkg");
    pack_archive(&root, &m, &archive_path).unwrap();
    let bytes = std::fs::read(&archive_path).unwrap();
    let reader = voltex_asset::VpkgReader::parse(&bytes).unwrap();
    let mut paths: Vec<String> = reader.list().map(|s| s.to_string()).collect();
    paths.sort();
    assert!(paths.contains(&"scene.vscn".to_string()));
    assert!(paths.contains(&"scripts/a.lua".to_string()));
    assert!(paths.contains(&"assets/sub/b.png".to_string()));
    let _ = std::fs::remove_dir_all(&root);
}

/// Project whose `assets = ["./**/*"]` resolves to the project root, with
/// leftovers from a previous build in `dist/` and cargo's `target/`.
fn root_glob_project(tag: &str) -> (PathBuf, Manifest) {
    let root = std::env::temp_dir().join(format!("voltex_cli_root_glob_{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("art")).unwrap();
    std::fs::create_dir_all(root.join("dist").join("wrapper")).unwrap();
    std::fs::create_dir_all(root.join("target").join("release")).unwrap();
    std::fs::write(root.join("art").join("hero.png"), "h").unwrap();
    std::fs::write(root.join("dist").join("wrapper").join("old.vpkg"), "old").unwrap();
    std::fs::write(root.join("target").join("release").join("game.exe"), "x").unwrap();
    let mut m = dummy("X", 100, 100);
    m.build.assets = vec!["./**/*".to_string()];
    (root, m)
}

#[test]
fn pack_archive_root_glob_skips_build_outputs() {
    let (root, m) = root_glob_project("pack");
    let archive_path = root.join("out.vpkg");
    pack_archive(&root, &m, &archive_path).unwrap();
    let bytes = std::fs::read(&archive_path).unwrap();
    let reader = voltex_asset::VpkgReader::parse(&bytes).unwrap();
    let keys: Vec<String> = reader.list().map(|s| s.to_string()).collect();
    assert!(keys.iter().any(|k| k.ends_with("art/hero.png")), "{keys:?}");
    assert!(
        !keys.iter().any(|k| k.contains("dist/") || k.contains("target/")),
        "build outputs must not be packed: {keys:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn copy_assets_root_glob_does_not_copy_dist_into_itself() {
    let (root, m) = root_glob_project("copy");
    let dist = root.join("dist");
    copy_assets(&root, &dist, &m).unwrap();
    assert!(dist.join("art").join("hero.png").exists());
    assert!(!dist.join("dist").exists(), "dist copied into itself");
    assert!(!dist.join("target").exists(), "cargo target copied into dist");
    let _ = std::fs::remove_dir_all(&root);
}

// ── v1.0.101: scene asset auto-bundle ──────────────────────────────────

/// Build a JSON scene whose one entity carries texture_path / mesh_path /
/// audio_path payloads, using the same registry round-trip the editor uses.
fn make_scene_json(tex: &str, mesh: &str, audio: &str) -> String {
    fn ser_tex(world: &voltex_ecs::World, e: voltex_ecs::Entity) -> Option<Vec<u8>> {
        world.get::<TexPathRef>(e).map(|p| p.0.as_bytes().to_vec())
    }
    fn ser_mesh(world: &voltex_ecs::World, e: voltex_ecs::Entity) -> Option<Vec<u8>> {
        world.get::<MeshPathRef>(e).map(|p| p.0.as_bytes().to_vec())
    }
    fn ser_audio(world: &voltex_ecs::World, e: voltex_ecs::Entity) -> Option<Vec<u8>> {
        world.get::<AudioPathRef>(e).map(|p| p.0.as_bytes().to_vec())
    }
    let mut registry = voltex_ecs::ComponentRegistry::new();
    registry.register::<voltex_ecs::Transform>();
    registry.register_raw("texture_path", ser_tex, deser_tex);
    registry.register_raw("mesh_path", ser_mesh, deser_mesh);
    registry.register_raw("audio_path", ser_audio, deser_audio);

    let mut world = voltex_ecs::World::new();
    let e = world.spawn();
    // Transform anchors the entity in the serialized scene.
    world.add(e, voltex_ecs::Transform::default());
    world.add(e, TexPathRef(tex.to_string()));
    world.add(e, MeshPathRef(mesh.to_string()));
    world.add(e, AudioPathRef(audio.to_string()));
    voltex_ecs::serialize_scene_json(&world, &registry)
}

#[test]
fn scan_scene_asset_paths_extracts_from_json_and_ignores_text() {
    let json = make_scene_json("assets/hero.png", "assets/ship.obj", "assets/boom.wav");
    let paths = scan_scene_asset_paths(json.as_bytes());
    assert_eq!(
        paths,
        vec!["assets/boom.wav", "assets/hero.png", "assets/ship.obj"]
    );

    // Legacy text scenes carry no registry components → empty, no panic.
    assert!(scan_scene_asset_paths(b"scene v1\nentity 0\n").is_empty());
    // Garbage bytes → empty, no panic.
    assert!(scan_scene_asset_paths(&[0xFF, 0x00, 0x12]).is_empty());
}

#[test]
fn scan_scene_asset_paths_sees_through_utf8_bom() {
    // A BOM-prefixed JSON scene used to be classified as legacy text and
    // silently skipped by auto-bundling.
    let json = make_scene_json("assets/hero.png", "assets/ship.obj", "assets/boom.wav");
    let with_bom = format!("\u{FEFF}{json}");
    assert_eq!(
        scan_scene_asset_paths(with_bom.as_bytes()),
        vec!["assets/boom.wav", "assets/hero.png", "assets/ship.obj"]
    );
}

#[test]
fn pack_archive_auto_bundles_scene_referenced_assets_and_warns_on_missing() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("voltex_cli_autobundle_{}", stamp));
    let _ = std::fs::remove_dir_all(&root);
    // `textures/` and `sounds/` are OUTSIDE the manifest's declared
    // assets dirs.
    std::fs::create_dir_all(root.join("textures")).unwrap();
    std::fs::write(root.join("textures").join("extra.png"), "px").unwrap();
    std::fs::create_dir_all(root.join("sounds")).unwrap();
    std::fs::write(root.join("sounds").join("boom.wav"), "au").unwrap();

    // Scene references two on-disk files and one missing file.
    let json = make_scene_json("textures/extra.png", "models/gone.obj", "sounds/boom.wav");
    std::fs::write(root.join("scene.vscn"), json).unwrap();

    let m = dummy("X", 100, 100);
    let archive_path = root.join("out.vpkg");
    pack_archive(&root, &m, &archive_path).unwrap();

    let bytes = std::fs::read(&archive_path).unwrap();
    let reader = voltex_asset::VpkgReader::parse(&bytes).unwrap();
    let paths: Vec<String> = reader.list().map(|s| s.to_string()).collect();
    assert!(
        paths.contains(&"textures/extra.png".to_string()),
        "referenced asset outside declared dirs is auto-bundled: {paths:?}"
    );
    assert!(
        paths.contains(&"sounds/boom.wav".to_string()),
        "scene-referenced audio is auto-bundled too: {paths:?}"
    );
    assert!(
        !paths.contains(&"models/gone.obj".to_string()),
        "missing reference is warned about, not packed"
    );
    let _ = std::fs::remove_dir_all(&root);
}

// ── v1.0.12: license bundle ────────────────────────────────────────────

fn make_engine_with_licenses(third_party: bool, mit: bool, apache: bool) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "voltex_cli_license_engine_{}_{}_{}",
        third_party as u8, mit as u8, stamp
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    if third_party {
        std::fs::write(root.join("THIRD-PARTY-LICENSES.md"), "third-party body").unwrap();
    }
    if mit {
        std::fs::write(root.join("LICENSE-MIT"), "MIT body").unwrap();
    }
    if apache {
        std::fs::write(root.join("LICENSE-APACHE"), "Apache body").unwrap();
    }
    root
}

#[test]
fn copy_license_files_copies_all_present() {
    let engine = make_engine_with_licenses(true, true, true);
    let dist = engine.join("dist");
    let copied = copy_license_files(&engine, &dist).unwrap();
    assert_eq!(copied.len(), 3);
    assert!(dist.join("THIRD-PARTY-LICENSES.md").is_file());
    assert!(dist.join("LICENSE-MIT").is_file());
    assert!(dist.join("LICENSE-APACHE").is_file());
    // Body preserved.
    let body = std::fs::read_to_string(dist.join("THIRD-PARTY-LICENSES.md")).unwrap();
    assert_eq!(body, "third-party body");
    let _ = std::fs::remove_dir_all(&engine);
}

#[test]
fn copy_license_files_skips_silently_when_missing() {
    // No license files at all → empty copied list, no error, no panic.
    let engine = make_engine_with_licenses(false, false, false);
    let dist = engine.join("dist");
    let copied = copy_license_files(&engine, &dist).unwrap();
    assert_eq!(copied.len(), 0);
    // dist dir still created (caller may want to write other artifacts).
    assert!(dist.is_dir());
    let _ = std::fs::remove_dir_all(&engine);
}

#[test]
fn copy_license_files_partial_only_copies_present() {
    // Only LICENSE-MIT present — the other two should be skipped.
    let engine = make_engine_with_licenses(false, true, false);
    let dist = engine.join("dist");
    let copied = copy_license_files(&engine, &dist).unwrap();
    assert_eq!(copied.len(), 1);
    assert!(dist.join("LICENSE-MIT").is_file());
    assert!(!dist.join("THIRD-PARTY-LICENSES.md").exists());
    assert!(!dist.join("LICENSE-APACHE").exists());
    let _ = std::fs::remove_dir_all(&engine);
}

#[test]
fn copy_license_files_resolves_against_actual_repo() {
    // The repo this test runs in IS the engine. THIRD-PARTY-LICENSES.md
    // was created in v1.0.7 — assert that helper finds it when given the
    // workspace root, so the build flow we just wired into build_target
    // works end-to-end (without invoking cargo).
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let dist =
        std::env::temp_dir().join(format!("voltex_cli_license_real_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dist);
    let copied = copy_license_files(&workspace_root, &dist).unwrap();
    assert!(
        copied
            .iter()
            .any(|p| p.to_str() == Some("THIRD-PARTY-LICENSES.md")),
        "expected THIRD-PARTY-LICENSES.md to be bundled from {} — got {:?}",
        workspace_root.display(),
        copied,
    );
    let _ = std::fs::remove_dir_all(&dist);
}

#[test]
fn wrapper_manifest_isolates_from_ancestor_workspaces() {
    let dir = std::env::temp_dir().join(format!("voltex_wrapper_ws_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    write_wrapper_manifest(&dir, Path::new("/engine")).unwrap();
    let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    // 없으면 워크스페이스 안(예: 이 레포의 examples/*)에서 스캐폴드한
    // 프로젝트의 wrapper 빌드가 카고 워크스페이스 검사로 통째 실패한다.
    assert!(manifest.contains("[workspace]"), "manifest:\n{manifest}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn validate_target_accepts_host_match_via_arg() {
    let mut m = dummy("X", 100, 100);
    m.build.targets = vec!["windows-x64".into(), "linux-x64".into()];
    let t = validate_target(&m, Some("windows-x64"), "windows-x64").unwrap();
    assert_eq!(t, "windows-x64");
}

#[test]
fn validate_target_accepts_host_from_manifest_when_arg_absent() {
    let mut m = dummy("X", 100, 100);
    m.build.targets = vec!["windows-x64".into(), "linux-x64".into()];
    let t = validate_target(&m, None, "linux-x64").unwrap();
    assert_eq!(t, "linux-x64");
}

#[test]
fn validate_target_falls_back_to_host_when_manifest_empty() {
    let mut m = dummy("X", 100, 100);
    m.build.targets = vec![];
    let t = validate_target(&m, None, "macos-arm64").unwrap();
    assert_eq!(t, "macos-arm64");
}

#[test]
fn validate_target_rejects_cross_compile_via_arg() {
    let m = dummy("X", 100, 100);
    let err = validate_target(&m, Some("macos-arm64"), "windows-x64").unwrap_err();
    match err {
        CliError::CrossCompileNotSupported { host, requested } => {
            assert_eq!(host, "windows-x64");
            assert_eq!(requested, "macos-arm64");
        }
        other => panic!("unexpected error: {:?}", other),
    }
}

#[test]
fn validate_target_rejects_host_missing_from_manifest_list() {
    let mut m = dummy("X", 100, 100);
    m.build.targets = vec!["windows-x64".into(), "macos-arm64".into()];
    let err = validate_target(&m, None, "linux-x64").unwrap_err();
    match err {
        CliError::HostNotInManifestTargets { host, listed } => {
            assert_eq!(host, "linux-x64");
            assert_eq!(listed, vec!["windows-x64", "macos-arm64"]);
        }
        other => panic!("unexpected error: {:?}", other),
    }
}

#[test]
fn cross_compile_error_message_lists_both_triples() {
    let e = CliError::CrossCompileNotSupported {
        host: "windows-x64".into(),
        requested: "macos-arm64".into(),
    };
    let s = format!("{}", e);
    assert!(s.contains("windows-x64"));
    assert!(s.contains("macos-arm64"));
    assert!(s.contains("cross-compile"));
}

/// End-to-end `kind = "rust"` build on a scaffolded dependency-free bin
/// crate. Spawns a real `cargo build` (~seconds) — run explicitly:
/// `cargo test -p voltex_cli -- --ignored rust_kind`.
#[test]
#[ignore]
fn rust_kind_build_ships_game_exe_and_loose_assets() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("voltex_cli_rust_kind_{}", stamp));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"mini_game\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(root.join("assets/level.txt"), "map").unwrap();

    let mut m = dummy("Mini", 100, 100);
    m.project.name = "mini_game".into();
    m.build.kind = BuildKind::Rust;

    // engine_path only feeds license copying; the repo root works.
    let engine = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    build_target(&root, Some(&engine), &m, host_triple()).expect("rust-kind build succeeds");

    let exe = root.join("dist").join(if cfg!(windows) {
        "mini_game.exe"
    } else {
        "mini_game"
    });
    assert!(exe.is_file(), "game exe shipped to dist/: {}", exe.display());
    assert!(
        root.join("dist/assets/level.txt").is_file(),
        "declared assets laid out loose next to the exe"
    );
    // No wrapper artifacts for rust kind.
    assert!(!root.join("dist/wrapper").exists(), "no wrapper codegen for kind=rust");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn build_skips_missing_assets_silently() {
    // Construct a manifest whose script / asset dirs do not exist.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("voltex_cli_build_missing_assets_{}", stamp));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let dist = root.join("dist");
    std::fs::create_dir_all(&dist).unwrap();
    let m = dummy("X", 100, 100);
    // copy_assets should not fail just because directories are absent.
    copy_assets(&root, &dist, &m).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
