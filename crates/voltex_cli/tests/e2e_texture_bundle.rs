//! End-to-end test for the editor-asset → shipped-exe chain (v1.0.96~103):
//!
//!   scene carries `texture_path` + `audio_path` → `voltex build`
//!   auto-bundles the files into `.vpkg` → the dist exe resolves both
//!   through the Vfs from a directory where no loose asset files exist.
//!
//! Why it exists: v1.0.96~101 verified each link with unit tests, but the
//! chain as a whole had never been executed in one pass. Per the v1.0.3 /
//! v1.0.9 precedent, integration gaps only surface when the real flow runs.
//!
//! Why `#[ignore]`: like `e2e_new_build.rs`, this invokes the wrapper
//! crate's `cargo build --release` (links the whole engine) and then runs
//! the produced exe for a few seconds — it also opens a real window, so it
//! requires a desktop session with a GPU. CI/devs invoke explicitly with
//! `--ignored`.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::SystemTime;

use voltex_ecs::{ComponentRegistry, Entity, Transform, World};

// ── Scene authoring helpers ──────────────────────────────────────────────
// `register_raw` takes plain fn pointers (no closure capture), so marker
// components carry the payloads — same pattern as the unit tests in
// `src/build.rs`.

struct TexPathRef(String);
struct AudioPathRef(String);
struct PrimBytes(Vec<u8>);

fn ser_tex(world: &World, e: Entity) -> Option<Vec<u8>> {
    world.get::<TexPathRef>(e).map(|p| p.0.as_bytes().to_vec())
}

fn ser_audio(world: &World, e: Entity) -> Option<Vec<u8>> {
    world.get::<AudioPathRef>(e).map(|p| p.0.as_bytes().to_vec())
}

fn ser_prim(world: &World, e: Entity) -> Option<Vec<u8>> {
    world.get::<PrimBytes>(e).map(|p| p.0.clone())
}

fn deser_noop(_: &mut World, _: Entity, _: &[u8]) -> voltex_ecs::Result<()> {
    Ok(())
}

/// `primitive_mesh` wire format for a cube: tag byte 0 + edge size LE f32
/// (see `voltex_renderer_ecs::PrimitiveMesh::serialize` — mirrored here so
/// the CLI test does not pull the GPU stack).
fn prim_cube_bytes(size: f32) -> Vec<u8> {
    let mut data = vec![0u8];
    data.extend_from_slice(&size.to_le_bytes());
    data
}

/// One entity: identity Transform + unit cube + texture_path + audio_path
/// references. This is the shape the editor's apply-and-save workflow
/// persists.
fn make_scene_json(tex_path: &str, audio_path: &str) -> String {
    let mut registry = ComponentRegistry::new();
    registry.register::<Transform>();
    registry.register_raw("texture_path", ser_tex, deser_noop);
    registry.register_raw("audio_path", ser_audio, deser_noop);
    registry.register_raw("primitive_mesh", ser_prim, deser_noop);

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::default());
    world.add(e, TexPathRef(tex_path.to_string()));
    world.add(e, AudioPathRef(audio_path.to_string()));
    world.add(e, PrimBytes(prim_cube_bytes(1.0)));
    voltex_ecs::serialize_scene_json(&world, &registry)
}

/// 2x2 red/green checker PNG via the engine's own encoder, so the engine's
/// own decoder is guaranteed to read it back.
fn checker_png() -> Vec<u8> {
    let red = [255u8, 0, 0, 255];
    let green = [0u8, 255, 0, 255];
    let mut rgba = Vec::with_capacity(16);
    for px in [red, green, green, red] {
        rgba.extend_from_slice(&px);
    }
    voltex_image::encode_rgba8(2, 2, &rgba)
}

#[test]
#[ignore = "heavy cargo build + opens a window; run explicitly with --ignored"]
fn e2e_scene_texture_is_bundled_and_resolves_from_foreign_dir() {
    let voltex_bin = env!("CARGO_BIN_EXE_voltex");
    let workspace_root: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root from CARGO_MANIFEST_DIR")
        .to_path_buf();

    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let scratch = std::env::temp_dir().join(format!("voltex_e2e_tex_{}", stamp));
    let _ = std::fs::remove_dir_all(&scratch);

    // ── 1. voltex new ────────────────────────────────────────────────────
    let status = Command::new(voltex_bin)
        .args(["new", scratch.to_str().expect("scratch path utf-8")])
        .status()
        .expect("spawn voltex new");
    assert!(status.success(), "voltex new exit={:?}", status.code());

    // ── 2. assets OUTSIDE the declared assets dir + referencing scene ──
    // `textures/` and `sounds/` are not in the manifest's
    // `assets = ["assets/**/*"]`, so the files only reach the archive
    // through the v1.0.101/103 auto-bundle.
    let tex_rel = "textures/checker.png";
    std::fs::create_dir_all(scratch.join("textures")).expect("mkdir textures");
    std::fs::write(scratch.join(tex_rel), checker_png()).expect("write png");
    let audio_rel = "sounds/beep.wav";
    std::fs::create_dir_all(scratch.join("sounds")).expect("mkdir sounds");
    std::fs::write(
        scratch.join(audio_rel),
        voltex_audio::generate_wav_bytes(&[0.25, -0.25, 0.5, -0.5], 44_100),
    )
    .expect("write wav");
    std::fs::write(
        scratch.join("scene.vscn"),
        make_scene_json(tex_rel, audio_rel),
    )
    .expect("write scene");

    // ── 3. voltex build — capture stderr for the auto-bundle log ────────
    let output = Command::new(voltex_bin)
        .current_dir(&scratch)
        .args([
            "build",
            "--engine-path",
            workspace_root.to_str().expect("workspace root utf-8"),
        ])
        .output()
        .expect("spawn voltex build");
    let build_stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "voltex build exit={:?}\nstderr:\n{}",
        output.status.code(),
        build_stderr
    );
    assert!(
        build_stderr.contains("auto-bundled scene asset `textures/checker.png`"),
        "expected texture auto-bundle log in build stderr:\n{}",
        build_stderr
    );
    assert!(
        build_stderr.contains("auto-bundled scene asset `sounds/beep.wav`"),
        "expected audio auto-bundle log in build stderr:\n{}",
        build_stderr
    );

    // ── 4. archive really contains both assets under the scene's keys ───
    let vpkg_bytes = std::fs::read(scratch.join("dist").join("wrapper").join("voltex_archive.vpkg"))
        .expect("read voltex_archive.vpkg");
    let reader = voltex_asset::VpkgReader::parse(&vpkg_bytes).expect("parse vpkg");
    for key in [tex_rel, audio_rel] {
        assert!(
            reader.list().any(|k| k == key),
            "vpkg should contain {key}; got {:?}",
            reader.list().collect::<Vec<_>>()
        );
    }

    // ── 5. run the exe from a directory with NO loose assets ────────────
    // From a foreign cwd the runtime's fs fallback cannot find
    // `textures/checker.png`, so the absence of a resolve error proves the
    // texture came from the embedded .vpkg.
    let manifest_text =
        std::fs::read_to_string(scratch.join("voltex.toml")).expect("read manifest");
    let project_name = manifest_text
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("name")
                .map(|rest| rest.trim_start_matches([' ', '=', '"']).trim_end_matches('"').to_string())
        })
        .expect("name in manifest");
    let exe_name = format!("{}{}", project_name, if cfg!(windows) { ".exe" } else { "" });
    let foreign = scratch.join("foreign_run_dir");
    std::fs::create_dir_all(&foreign).expect("mkdir foreign dir");
    std::fs::copy(scratch.join("dist").join(&exe_name), foreign.join(&exe_name))
        .expect("copy exe to foreign dir");

    let mut child = Command::new(foreign.join(&exe_name))
        .current_dir(&foreign)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dist exe");
    // The first resolve only happens once the GPU pipelines finish their
    // cold compile — observed at ~6s after boot on an RTX 4080, so 12s
    // leaves headroom for slower machines.
    std::thread::sleep(std::time::Duration::from_secs(12));
    if let Some(status) = child.try_wait().expect("try_wait dist exe") {
        // Collect whatever it printed before dying for the panic message.
        let out = child.wait_with_output().expect("wait_with_output");
        panic!(
            "dist exe exited early ({:?})\nstdout:\n{}\nstderr:\n{}",
            status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    child.kill().expect("kill dist exe");
    let out = child.wait_with_output().expect("collect dist exe output");
    let run_log = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !run_log.contains("failed to resolve") && !run_log.contains("failed to load"),
        "runtime failed to resolve a scene asset — vpkg chain broken:\n{}",
        run_log
    );
    // Positive evidence — absence of errors alone can't distinguish "resolved
    // from vpkg" from "scene never loaded" (the initial-scene load degrades
    // to an empty world on failure).
    assert!(
        run_log.contains("initial scene `scene.vscn` loaded (1 entities)"),
        "expected initial-scene load log:\n{}",
        run_log
    );
    assert!(
        run_log.contains("resolved 1 texture path(s)"),
        "expected texture resolve log:\n{}",
        run_log
    );
    // Audio resolves in the tick's audio stage (no GPU needed) — only runs
    // because the wrapper enables audio (v1.0.106).
    assert!(
        run_log.contains("resolved 1 audio path(s)"),
        "expected audio resolve log:\n{}",
        run_log
    );

    let _ = std::fs::remove_dir_all(&scratch);
}
