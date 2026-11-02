//! Every `.vscn` shipped in this repository must load through the same
//! format dispatch the runtime uses (binary `VSCN` magic, otherwise JSON).
//! `voltex build` packs the manifest's scene as the dist exe's initial scene,
//! so a scene that only works because a game seeds its world in Rust still
//! breaks the packaged build.

use std::path::{Path, PathBuf};

use voltex_ecs::{deserialize_scene_binary, deserialize_scene_json, ComponentRegistry, World};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_scenes(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if matches!(name.to_str(), Some("target" | "dist" | ".git" | "fuzz")) {
                continue;
            }
            collect_scenes(&path, out);
        } else if path.extension().is_some_and(|e| e == "vscn") {
            out.push(path);
        }
    }
}

#[test]
fn every_repo_scene_file_loads() {
    let mut scenes = Vec::new();
    for top in ["games", "examples", "engine", "tools", "assets", "docs"] {
        collect_scenes(&repo_root().join(top), &mut scenes);
    }
    assert!(!scenes.is_empty(), "scene discovery found nothing — walk is broken");

    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    let mut failures = Vec::new();
    for path in &scenes {
        let bytes = std::fs::read(path).unwrap();
        let mut world = World::new();
        let result = if bytes.starts_with(b"VSCN") {
            deserialize_scene_binary(&mut world, &bytes, &registry).map(|_| ())
        } else {
            match std::str::from_utf8(&bytes) {
                Ok(text) => deserialize_scene_json(&mut world, text, &registry).map(|_| ()),
                Err(e) => Err(e.to_string().into()),
            }
        };
        if let Err(e) = result {
            failures.push(format!("{}: {e}", path.display()));
        }
    }
    assert!(failures.is_empty(), "scenes that fail to load:\n{}", failures.join("\n"));
}
