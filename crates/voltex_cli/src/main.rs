//! voltex_cli — prototype CLI ("B1-spike").
//!
//! Single subcommand: `voltex build <project_dir>`. Generates a tiny wrapper
//! crate, runs `cargo build --release`, and copies the resulting exe along
//! with `scene.vscn`, `scripts/`, `assets/` next to it.
//!
//! Production-grade work (voltex.toml manifest, `voltex new`, `voltex run`,
//! cross-OS, asset packing) lives in Cycle 8+.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const VOLTEX_REPO_ENV: &str = "VOLTEX_REPO";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("voltex (prototype) — usage: voltex build <project_dir>");
        return ExitCode::from(2);
    }
    match args[1].as_str() {
        "build" => {
            if args.len() < 3 {
                eprintln!("voltex build: missing <project_dir>");
                return ExitCode::from(2);
            }
            match build(Path::new(&args[2])) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {}", e);
                    ExitCode::FAILURE
                }
            }
        }
        other => {
            eprintln!("voltex: unknown subcommand `{}`. only `build` is supported in the prototype.", other);
            ExitCode::from(2)
        }
    }
}

fn build(project_dir: &Path) -> Result<(), String> {
    let project_dir = project_dir
        .canonicalize()
        .map_err(|e| format!("cannot resolve project dir: {}", e))?;
    if !project_dir.is_dir() {
        return Err(format!("not a directory: {}", project_dir.display()));
    }
    let name = project_name(&project_dir)?;
    let voltex_repo = std::env::var(VOLTEX_REPO_ENV)
        .map_err(|_| format!("environment variable `{}` not set — point it at the voltex engine repo root", VOLTEX_REPO_ENV))?;
    let voltex_repo = PathBuf::from(voltex_repo)
        .canonicalize()
        .map_err(|e| format!("{} resolves to invalid path: {}", VOLTEX_REPO_ENV, e))?;

    eprintln!("voltex build: project = {} (name = {})", project_dir.display(), name);
    eprintln!("voltex build: engine = {}", voltex_repo.display());

    let dist_dir = project_dir.join("dist");
    let wrapper_dir = dist_dir.join("wrapper");
    std::fs::create_dir_all(&wrapper_dir).map_err(|e| format!("cannot create wrapper dir: {}", e))?;
    std::fs::create_dir_all(wrapper_dir.join("src"))
        .map_err(|e| format!("cannot create wrapper src dir: {}", e))?;

    write_wrapper_manifest(&wrapper_dir, &name, &voltex_repo)?;
    write_wrapper_main(&wrapper_dir, &name)?;

    eprintln!("voltex build: invoking cargo build --release ...");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&wrapper_dir)
        .status()
        .map_err(|e| format!("failed to spawn cargo: {}", e))?;
    if !status.success() {
        return Err(format!("cargo build failed with status {}", status));
    }

    let exe_src = wrapper_dir
        .join("target")
        .join("release")
        .join(if cfg!(windows) {
            format!("{}.exe", "wrapper")
        } else {
            "wrapper".to_string()
        });
    let exe_dst = dist_dir.join(if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.clone()
    });
    std::fs::copy(&exe_src, &exe_dst)
        .map_err(|e| format!("cannot copy exe from {}: {}", exe_src.display(), e))?;
    eprintln!("voltex build: wrote {}", exe_dst.display());

    copy_optional_file(&project_dir.join("scene.vscn"), &dist_dir.join("scene.vscn"))?;
    copy_optional_dir(&project_dir.join("scripts"), &dist_dir.join("scripts"))?;
    copy_optional_dir(&project_dir.join("assets"), &dist_dir.join("assets"))?;

    eprintln!("voltex build: done.");
    Ok(())
}

fn project_name(project_dir: &Path) -> Result<String, String> {
    project_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "project dir has no usable file name".to_string())
}

fn write_wrapper_manifest(wrapper_dir: &Path, _name: &str, voltex_repo: &Path) -> Result<(), String> {
    let runtime_path = voltex_repo.join("crates").join("voltex_runtime");
    // Forward slashes so the path is valid TOML on Windows too.
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
    std::fs::write(wrapper_dir.join("Cargo.toml"), manifest)
        .map_err(|e| format!("write wrapper Cargo.toml: {}", e))
}

fn write_wrapper_main(wrapper_dir: &Path, name: &str) -> Result<(), String> {
    // Window title is the only knob the prototype passes through. Scene /
    // script loading happens in B2+ once VFS exists.
    let main = format!(
        r#"use voltex_runtime::{{RuntimeApp, RuntimeConfig}};

fn main() -> voltex_runtime::RuntimeResult<()> {{
    RuntimeApp::new(
        RuntimeConfig::new().with_window_title("{}"),
    )?
    .run()
}}
"#,
        escape_for_rust_string(name)
    );
    std::fs::write(wrapper_dir.join("src").join("main.rs"), main)
        .map_err(|e| format!("write wrapper main.rs: {}", e))
}

fn escape_for_rust_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn copy_optional_file(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {}", parent.display(), e))?;
    }
    std::fs::copy(src, dst).map_err(|e| format!("copy {}: {}", src.display(), e))?;
    eprintln!("voltex build: copied {}", src.display());
    Ok(())
}

fn copy_optional_dir(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    if !src.is_dir() {
        return Err(format!("{} exists but is not a directory", src.display()));
    }
    copy_dir_recursive(src, dst)?;
    eprintln!("voltex build: copied {}/ → {}/", src.display(), dst.display());
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {}", dst.display(), e))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("read_dir {}: {}", src.display(), e))? {
        let entry = entry.map_err(|e| format!("dir entry in {}: {}", src.display(), e))?;
        let ty = entry.file_type().map_err(|e| format!("file_type: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if ty.is_file() {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("copy {}: {}", src_path.display(), e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_name_extracts_last_path_component() {
        let tmp = std::env::temp_dir().join("voltex_cli_test_project_name");
        std::fs::create_dir_all(&tmp).unwrap();
        let n = project_name(&tmp).unwrap();
        assert_eq!(n, "voltex_cli_test_project_name");
    }

    #[test]
    fn escape_for_rust_string_escapes_quotes_and_backslashes() {
        assert_eq!(escape_for_rust_string("a\"b"), "a\\\"b");
        assert_eq!(escape_for_rust_string("c\\d"), "c\\\\d");
        assert_eq!(escape_for_rust_string("plain"), "plain");
    }

    #[test]
    fn copy_optional_file_no_op_when_missing() {
        let tmp = std::env::temp_dir().join("voltex_cli_test_missing_src");
        let dst = std::env::temp_dir().join("voltex_cli_test_missing_dst");
        // src does not exist → no-op, no error
        assert!(copy_optional_file(&tmp, &dst).is_ok());
        assert!(!dst.exists());
    }
}
