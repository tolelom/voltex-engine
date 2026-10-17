//! Template of the generated `wrapper` crate that `voltex build` compiles around the game archive.

use std::path::Path;

use crate::error::Result;
use crate::manifest::Manifest;
use crate::util::escape_for_rust_string;

pub(super) fn write_wrapper_manifest(wrapper_dir: &Path, engine_path: &Path) -> Result<()> {
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

# 조상 디렉토리의 카고 워크스페이스에 흡수되지 않도록 격리 — 없으면
# 워크스페이스 안에서 스캐폴드한 프로젝트의 빌드가 "believes it's in a
# workspace" 에러로 통째 실패한다.
[workspace]
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
    // Audio is always on in shipped builds: scene-persisted AudioPath
    // resolution and script-spawned sources both live behind it, and a
    // failed device init degrades to silence gracefully.
    let mut cfg = RuntimeConfig::new()
        .with_window_title("{title}")
        .with_window_size({w}, {h})
        .with_audio(Vec::new())
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

pub(super) fn write_wrapper_main(wrapper_dir: &Path, manifest: &Manifest) -> Result<()> {
    let body = render_wrapper_main(manifest);
    std::fs::write(wrapper_dir.join("src").join("main.rs"), body)?;
    Ok(())
}
