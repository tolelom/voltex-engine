//! `voltex build` — manifest-driven build.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{CliError, Result};
use crate::manifest::{host_triple, Manifest};
use crate::util::{copy_optional_dir, copy_optional_file, escape_for_rust_string};

const VOLTEX_REPO_ENV: &str = "VOLTEX_REPO";

pub fn cmd(args: &[String]) -> Result<()> {
    let manifest_dir = parse_manifest_path(args)?;
    let target_arg = parse_target_arg(args);
    let engine_path = resolve_engine_path(args)?;

    let manifest = Manifest::load_from_dir(&manifest_dir)?;
    let target = target_arg
        .or_else(|| manifest.build.targets.first().cloned())
        .unwrap_or_else(|| host_triple().to_string());

    build_target(&manifest_dir, &engine_path, &manifest, &target)
}

fn parse_manifest_path(args: &[String]) -> Result<PathBuf> {
    if let Some(idx) = args.iter().position(|a| a == "--manifest-path") {
        let value = args.get(idx + 1).ok_or(CliError::MissingArg("--manifest-path <path>"))?;
        Ok(PathBuf::from(value))
    } else {
        Ok(std::env::current_dir()?)
    }
}

fn parse_target_arg(args: &[String]) -> Option<String> {
    let idx = args.iter().position(|a| a == "--target")?;
    args.get(idx + 1).cloned()
}

fn resolve_engine_path(args: &[String]) -> Result<PathBuf> {
    if let Some(idx) = args.iter().position(|a| a == "--engine-path") {
        let value = args
            .get(idx + 1)
            .ok_or(CliError::MissingArg("--engine-path <path>"))?;
        return Ok(PathBuf::from(value));
    }
    let env = std::env::var(VOLTEX_REPO_ENV).map_err(|_| CliError::Io(format!(
        "engine path unknown — set `{}` or pass `--engine-path <repo-root>`",
        VOLTEX_REPO_ENV
    )))?;
    Ok(PathBuf::from(env))
}

pub fn build_target(
    manifest_dir: &Path,
    engine_path: &Path,
    manifest: &Manifest,
    target: &str,
) -> Result<()> {
    let manifest_dir = manifest_dir.canonicalize()?;
    let engine_path = engine_path.canonicalize()?;
    eprintln!(
        "voltex build: project={} target={}",
        manifest_dir.display(),
        target
    );

    let dist_dir = manifest_dir.join("dist");
    let wrapper_dir = dist_dir.join("wrapper");
    std::fs::create_dir_all(wrapper_dir.join("src"))?;

    // Build the .vpkg archive *before* writing wrapper main.rs so the
    // codegen can reference it via include_bytes!.
    let archive_path = wrapper_dir.join("voltex_archive.vpkg");
    pack_archive(&manifest_dir, manifest, &archive_path)?;

    write_wrapper_manifest(&wrapper_dir, &engine_path)?;
    write_wrapper_main(&wrapper_dir, manifest)?;

    let cargo = which_cargo()?;
    eprintln!("voltex build: running `cargo build --release` ...");
    let status = Command::new(cargo)
        .args(["build", "--release"])
        .current_dir(&wrapper_dir)
        .status()
        .map_err(CliError::from)?;
    if !status.success() {
        return Err(CliError::CargoBuildFailed(status.code().unwrap_or(-1)));
    }

    let exe_src = wrapper_dir
        .join("target")
        .join("release")
        .join(if cfg!(windows) { "wrapper.exe" } else { "wrapper" });
    let exe_dst = dist_dir.join(if cfg!(windows) {
        format!("{}.exe", manifest.project.name)
    } else {
        manifest.project.name.clone()
    });
    std::fs::copy(&exe_src, &exe_dst)?;
    eprintln!("voltex build: wrote {}", exe_dst.display());

    copy_assets(&manifest_dir, &dist_dir, manifest)?;

    Ok(())
}

fn which_cargo() -> Result<&'static str> {
    // We only check for the binary lazily — actual spawn failure surfaces a
    // friendlier message via CargoMissing.
    if Command::new("cargo").arg("--version").output().is_err() {
        return Err(CliError::CargoMissing);
    }
    Ok("cargo")
}

fn copy_assets(manifest_dir: &Path, dist_dir: &Path, manifest: &Manifest) -> Result<()> {
    // scene
    let scene = manifest_dir.join(&manifest.build.scene);
    let scene_dst = dist_dir.join(&manifest.build.scene);
    copy_optional_file(&scene, &scene_dst)?;

    // scripts (each glob → its directory prefix)
    for dir in manifest.resolved_script_dirs(manifest_dir) {
        let dst = dist_dir.join(dir.file_name().unwrap_or_default());
        copy_optional_dir(&dir, &dst)?;
    }
    // assets (same pattern)
    for dir in manifest.resolved_assets_dirs(manifest_dir) {
        let dst = dist_dir.join(dir.file_name().unwrap_or_default());
        copy_optional_dir(&dir, &dst)?;
    }
    Ok(())
}

fn write_wrapper_manifest(wrapper_dir: &Path, engine_path: &Path) -> Result<()> {
    let runtime_path = engine_path.join("crates").join("voltex_runtime");
    let runtime_path_str = runtime_path.to_string_lossy().replace('\\', "/");
    let manifest = format!(
        r#"[package]
name = "wrapper"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "wrapper"
path = "src/main.rs"

[dependencies]
voltex_runtime = {{ path = "{}" }}
"#,
        runtime_path_str
    );
    std::fs::write(wrapper_dir.join("Cargo.toml"), manifest)?;
    Ok(())
}

pub fn render_wrapper_main(manifest: &Manifest) -> String {
    // Wrapper embeds `voltex_archive.vpkg` next to itself via include_bytes!.
    // The runtime reads the scene through Vfs::Archive, so the exe is fully
    // self-contained — no `scene.vscn` next to it required.
    let scene_lit = escape_for_rust_string(&manifest.build.scene);
    let title_lit = escape_for_rust_string(&manifest.build.window_title);
    let (w, h) = manifest.build.window_size;
    format!(
        r#"use voltex_runtime::{{RuntimeApp, RuntimeConfig, Vfs}};

const ARCHIVE_BYTES: &[u8] = include_bytes!("../voltex_archive.vpkg");

fn main() -> voltex_runtime::RuntimeResult<()> {{
    let vfs = Vfs::archive_from_bytes(ARCHIVE_BYTES)
        .map_err(|e| voltex_runtime::RuntimeError::Generic(format!("vfs: {{}}", e)))?;
    let mut cfg = RuntimeConfig::new()
        .with_window_title("{title}")
        .with_window_size({w}, {h})
        .with_vfs(vfs);
    cfg = cfg.with_initial_scene("{scene}");
    RuntimeApp::new(cfg)?.run()
}}
"#,
        title = title_lit,
        w = w,
        h = h,
        scene = scene_lit,
    )
}

/// Walk the manifest's scene + scripts + assets dirs and write a `.vpkg`.
/// Paths inside the archive use forward slashes and are relative to the
/// manifest directory (so `with_initial_scene("scene.vscn")` works).
pub fn pack_archive(
    manifest_dir: &Path,
    manifest: &Manifest,
    out_path: &Path,
) -> Result<()> {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

    // 1. scene
    let scene_path = manifest_dir.join(&manifest.build.scene);
    if scene_path.exists() {
        let bytes = std::fs::read(&scene_path)?;
        entries.push((manifest.build.scene.clone(), bytes));
    }

    // 2. scripts
    for dir in manifest.resolved_script_dirs(manifest_dir) {
        collect_into_entries(&dir, manifest_dir, &mut entries)?;
    }
    // 3. assets
    for dir in manifest.resolved_assets_dirs(manifest_dir) {
        collect_into_entries(&dir, manifest_dir, &mut entries)?;
    }

    let archive = voltex_asset::write_vpkg(&entries);
    std::fs::write(out_path, archive)?;
    eprintln!(
        "voltex build: packed {} entries → {}",
        entries.len(),
        out_path.display()
    );
    Ok(())
}

fn collect_into_entries(
    dir: &Path,
    relative_to: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let path = entry.path();
        if ty.is_dir() {
            collect_into_entries(&path, relative_to, out)?;
        } else if ty.is_file() {
            let rel = path
                .strip_prefix(relative_to)
                .map_err(|e| CliError::Io(format!("strip_prefix: {}", e)))?;
            let key = rel.to_string_lossy().replace('\\', "/");
            let bytes = std::fs::read(&path)?;
            out.push((key, bytes));
        }
    }
    Ok(())
}

fn write_wrapper_main(wrapper_dir: &Path, manifest: &Manifest) -> Result<()> {
    let body = render_wrapper_main(manifest);
    std::fs::write(wrapper_dir.join("src").join("main.rs"), body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{BuildConfig, Manifest, ProjectInfo};
    use std::collections::HashMap;

    fn dummy(window_title: &str, w: u32, h: u32) -> Manifest {
        Manifest {
            project: ProjectInfo {
                name: "demo".into(),
                version: "0.1.0".into(),
            },
            build: BuildConfig {
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
    fn wrapper_main_includes_window_title_from_manifest() {
        let m = dummy("Hello \"World\"", 800, 600);
        let body = render_wrapper_main(&m);
        // Title is escaped properly.
        assert!(body.contains(r#"with_window_title("Hello \"World\"")"#));
        assert!(body.contains("with_window_size(800, 600)"));
        assert!(body.contains("with_initial_scene"));
        assert!(body.contains("include_bytes!"));
        assert!(body.contains("with_vfs"));
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
}
