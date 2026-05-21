//! End-to-end smoke test for the user's first 30 seconds with the CLI:
//!
//!   `voltex new my_game && cd my_game && voltex build`
//!
//! Why it exists: the quickstart path is the single most user-visible
//! feature, and prior incidents (v1.0.3 manifest validation, v1.0.9
//! OGG/Vorbis wire mismatch) only got caught because someone happened to
//! re-run the flow. This test makes the regression detectable in CI.
//!
//! Why `#[ignore]`: invokes the wrapper crate's `cargo build --release`,
//! which links the entire engine — ~60 seconds even on a warm cache and
//! 5+ minutes cold. Default `cargo test` skips it; CI invokes explicitly
//! with `--ignored`.

use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

#[test]
#[ignore = "heavy ~60s cargo build; run explicitly with --ignored"]
fn e2e_new_then_build_produces_dist_exe() {
    let voltex_bin = env!("CARGO_BIN_EXE_voltex");

    // Workspace root = three levels up from this test file
    // (crates/voltex_cli/tests/e2e_new_build.rs → repo root).
    let workspace_root: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root from CARGO_MANIFEST_DIR")
        .to_path_buf();

    // Unique scratch dir so concurrent CI matrix jobs don't collide.
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let scratch = std::env::temp_dir().join(format!("voltex_e2e_{}", stamp));
    let _ = std::fs::remove_dir_all(&scratch);

    // ── 1. voltex new ────────────────────────────────────────────────────
    let status = Command::new(voltex_bin)
        .args(["new", scratch.to_str().expect("scratch path utf-8")])
        .status()
        .expect("spawn voltex new");
    assert!(status.success(), "voltex new exit={:?}", status.code());

    assert!(scratch.join("voltex.toml").exists(), "voltex.toml missing");
    assert!(scratch.join("scene.vscn").exists(), "scene.vscn missing");
    assert!(
        scratch.join("scripts").join("spinner.lua").exists(),
        "default script missing"
    );

    // ── 2. voltex build ──────────────────────────────────────────────────
    let status = Command::new(voltex_bin)
        .current_dir(&scratch)
        .args([
            "build",
            "--engine-path",
            workspace_root.to_str().expect("workspace root utf-8"),
        ])
        .status()
        .expect("spawn voltex build");
    assert!(status.success(), "voltex build exit={:?}", status.code());

    // ── 3. Verify dist artifact ──────────────────────────────────────────
    let exe_name = if cfg!(windows) {
        "my_game.exe"
    } else {
        "my_game"
    };
    // `voltex new <scratch>` uses the directory's `file_name()` as the
    // project name — scratch is `voltex_e2e_<stamp>`, so we need to read
    // the manifest to learn the real name.
    let manifest_text =
        std::fs::read_to_string(scratch.join("voltex.toml")).expect("read manifest");
    let actual_name = manifest_text
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("name") {
                let value = rest.trim_start_matches([' ', '=', '"']).trim_end_matches('"');
                Some(value.to_string())
            } else {
                None
            }
        })
        .expect("name in manifest");
    let exe = scratch.join("dist").join(format!(
        "{}{}",
        actual_name,
        if cfg!(windows) { ".exe" } else { "" }
    ));
    assert!(
        exe.exists(),
        "expected exe at {} (default exe_name guess was {})",
        exe.display(),
        exe_name
    );

    // ── 4. Verify license bundle (v1.0.12 contract) ──────────────────────
    let dist = scratch.join("dist");
    assert!(
        dist.join("LICENSE-MIT").exists() && dist.join("LICENSE-APACHE").exists(),
        "license files missing from dist/"
    );

    let _ = std::fs::remove_dir_all(&scratch);
}
