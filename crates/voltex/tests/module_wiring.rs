//! Repo hygiene: every `.rs` file under `crates/*/src` and `games/*/src` must
//! be reachable from its crate's module tree. A file that no `mod x;`
//! declaration references is silently dead code — it never compiles, its
//! tests never run, and CODEMAPs drift (dogfood friction #1: `navmesh_builder/`
//! sat unwired for months with 7 never-compiled tests).
//!
//! Detection is declaration-based: for `src/**/foo.rs` we require a
//! `mod foo;` / `pub mod foo;` / `pub(crate) mod foo;` (optionally behind
//! `#[cfg(...)]`) somewhere in the same crate's `src/**/*.rs`. Crate roots
//! (`lib.rs` / `main.rs` / `mod.rs`), cargo-discovered `src/bin/*.rs`, and
//! files referenced by a `path = "..."` in the crate's `Cargo.toml` (extra
//! `[[bin]]`s) are exempt.

use std::path::{Path, PathBuf};

/// Files that intentionally live outside the module tree. Keep this empty
/// unless a file has a documented reason to be unwired.
const ALLOWLIST: &[&str] = &[];

fn workspace_root() -> PathBuf {
    // crates/voltex/tests → repo root is two levels up from CARGO_MANIFEST_DIR.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Does `haystack` contain a module declaration for `stem`?
/// Matches `mod stem;`, `pub mod stem;`, `pub(crate) mod stem;` at any
/// position (attributes like `#[cfg(test)]` live on preceding lines).
fn declares_module(haystack: &str, stem: &str) -> bool {
    for line in haystack.lines() {
        let t = line.trim_start();
        let t = t.strip_prefix("pub(crate) ").unwrap_or(t);
        let t = t.strip_prefix("pub ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("mod ") {
            let rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix(stem) {
                let after = after.trim_start();
                if after.starts_with(';') || after.starts_with('{') {
                    return true;
                }
            }
        }
    }
    false
}

#[test]
fn every_src_file_is_wired_into_its_module_tree() {
    let root = workspace_root();
    let mut crate_dirs: Vec<PathBuf> = Vec::new();
    for group in ["crates", "games"] {
        let Ok(entries) = std::fs::read_dir(root.join(group)) else { continue };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.join("src").is_dir() && p.join("Cargo.toml").is_file() {
                crate_dirs.push(p);
            }
        }
    }
    assert!(!crate_dirs.is_empty(), "no crates found under {}", root.display());

    let mut violations: Vec<String> = Vec::new();
    for crate_dir in &crate_dirs {
        let src = crate_dir.join("src");
        let mut files = Vec::new();
        collect_rs_files(&src, &mut files);

        // Concatenate all source text once per crate; a declaration anywhere
        // in the crate counts as wired.
        let all_text: String = files
            .iter()
            .filter_map(|f| std::fs::read_to_string(f).ok())
            .collect::<Vec<_>>()
            .join("\n");
        let cargo_toml =
            std::fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap_or_default();

        for file in &files {
            let rel = file.strip_prefix(&src).expect("under src");
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let stem = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();

            // Exemptions: crate roots, directory owners, cargo bin autodiscovery,
            // explicit [[bin]]/[lib] path references, allowlist.
            if matches!(stem.as_str(), "lib" | "main" | "mod" | "build") {
                continue;
            }
            if rel_str.starts_with("bin/") {
                continue;
            }
            if cargo_toml.contains(&format!("src/{rel_str}")) {
                continue;
            }
            let crate_rel = format!(
                "{}/src/{}",
                crate_dir.file_name().unwrap().to_string_lossy(),
                rel_str
            );
            if ALLOWLIST.contains(&crate_rel.as_str()) {
                continue;
            }

            if !declares_module(&all_text, &stem) {
                violations.push(crate_rel);
            }
        }
    }

    assert!(
        violations.is_empty(),
        "unwired .rs files (never compiled — wire them with `mod x;` or add a \
         documented ALLOWLIST entry):\n  {}",
        violations.join("\n  ")
    );
}
