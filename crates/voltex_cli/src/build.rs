//! `voltex build` — manifest-driven build.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{CliError, Result};
use crate::manifest::{host_triple, BuildKind, Manifest};
use crate::util::{
    canonicalize_friendly, copy_optional_dir, copy_optional_file, escape_for_rust_string,
};

const VOLTEX_REPO_ENV: &str = "VOLTEX_REPO";

pub fn cmd(args: &[String]) -> Result<()> {
    let manifest_dir = parse_manifest_path(args)?;
    let target_arg = parse_target_arg(args);
    let skip_path_validation = args.iter().any(|a| a == "--no-validate-paths");

    let manifest = Manifest::load_from_dir(&manifest_dir)?;
    // kind="rust"는 엔진 경로를 라이선스 동봉에만 쓴다 — 게임 crate가 이미
    // voltex를 의존으로 갖고 있으므로, 미설정이면 라이선스만 건너뛰고 진행.
    let engine_path = match resolve_engine_path(args) {
        Ok(p) => Some(p),
        Err(_) if manifest.build.kind == BuildKind::Rust => {
            eprintln!(
                "voltex build: note: engine path not set — skipping license \
                 bundling (set VOLTEX_REPO or --engine-path to include it)"
            );
            None
        }
        Err(e) => return Err(e),
    };
    if !skip_path_validation {
        manifest.validate_paths(&manifest_dir)?;
    }
    let target = validate_target(&manifest, target_arg.as_deref(), host_triple())?;

    build_target(&manifest_dir, engine_path.as_deref(), &manifest, &target)
}

/// Pick the build target and reject impossible requests up front (B3).
///
/// Policy: `voltex build` only produces native binaries. Cross-compiling
/// to a different OS/arch is explicitly out of scope — toolchain setup
/// (Apple frameworks, Windows SDK, ALSA headers) makes it impractical for
/// an engine that links three different audio backends at runtime.
///
/// Resolution order:
/// 1. If `target_arg` is set, it must equal `host`.
/// 2. Otherwise, if the manifest lists targets, the host triple must be in
///    that list.
/// 3. Otherwise, fall back to `host`.
pub fn validate_target(
    manifest: &Manifest,
    target_arg: Option<&str>,
    host: &str,
) -> Result<String> {
    if let Some(req) = target_arg {
        if req != host {
            return Err(CliError::CrossCompileNotSupported {
                host: host.to_string(),
                requested: req.to_string(),
            });
        }
        return Ok(req.to_string());
    }
    let listed = &manifest.build.targets;
    if listed.is_empty() {
        return Ok(host.to_string());
    }
    if listed.iter().any(|t| t == host) {
        return Ok(host.to_string());
    }
    Err(CliError::HostNotInManifestTargets {
        host: host.to_string(),
        listed: listed.clone(),
    })
}

fn parse_manifest_path(args: &[String]) -> Result<PathBuf> {
    if let Some(idx) = args.iter().position(|a| a == "--manifest-path") {
        let value = args
            .get(idx + 1)
            .ok_or(CliError::MissingArg("--manifest-path <path>"))?;
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
    let env = std::env::var(VOLTEX_REPO_ENV).map_err(|_| {
        CliError::Io(format!(
            "engine path unknown — set `{env}` or pass `--engine-path <repo-root>`.\n\
         \n\
         Typical fix in a scaffolded project (`voltex new my_game && cd my_game`):\n  \
           voltex build --engine-path /path/to/voltex\n\
         or persist for the shell session:\n  \
           export {env}=/path/to/voltex   # bash/zsh\n  \
           $env:{env}='C:/path/to/voltex' # PowerShell",
            env = VOLTEX_REPO_ENV
        ))
    })?;
    Ok(PathBuf::from(env))
}

pub fn build_target(
    manifest_dir: &Path,
    engine_path: Option<&Path>,
    manifest: &Manifest,
    target: &str,
) -> Result<()> {
    let manifest_dir = canonicalize_friendly(manifest_dir)?;
    let engine_path = match engine_path {
        Some(p) => Some(canonicalize_friendly(p)?),
        None => None,
    };
    eprintln!(
        "voltex build: project={} target={}",
        manifest_dir.display(),
        target
    );

    if manifest.build.kind == BuildKind::Rust {
        return build_rust_target(&manifest_dir, engine_path.as_deref(), manifest);
    }
    // scene kind의 wrapper 코드젠은 엔진 경로가 필수.
    let engine_path = engine_path.ok_or_else(|| {
        CliError::Io("engine path required for scene builds — set VOLTEX_REPO or --engine-path".into())
    })?;

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
        .join(if cfg!(windows) {
            "wrapper.exe"
        } else {
            "wrapper"
        });
    let exe_dst = dist_dir.join(if cfg!(windows) {
        format!("{}.exe", manifest.project.name)
    } else {
        manifest.project.name.clone()
    });
    std::fs::copy(&exe_src, &exe_dst)?;
    eprintln!("voltex build: wrote {}", exe_dst.display());

    copy_assets(&manifest_dir, &dist_dir, manifest)?;

    let licenses = copy_license_files(&engine_path, &dist_dir)?;
    if !licenses.is_empty() {
        eprintln!("voltex build: bundled {} license file(s)", licenses.len());
    }

    Ok(())
}

/// `kind = "rust"` build path (dogfood friction #8): the game is a normal
/// cargo package whose `Cargo.toml` sits next to `voltex.toml`. Instead of
/// generating a wrapper, we `cargo build --release` the game itself, copy the
/// produced binary into `dist/`, and lay its declared asset dirs loose next
/// to it — the shipped layout `RuntimeConfig::with_asset_root("assets")`
/// expects. No `.vpkg` is packed: a Rust game owns its runtime config and can
/// opt into a Vfs itself.
fn build_rust_target(
    manifest_dir: &Path,
    engine_path: Option<&Path>,
    manifest: &Manifest,
) -> Result<()> {
    if !manifest_dir.join("Cargo.toml").is_file() {
        return Err(CliError::Io(format!(
            "kind = \"rust\" expects a Cargo.toml next to voltex.toml (looked in {})",
            manifest_dir.display()
        )));
    }

    let cargo = which_cargo()?;
    eprintln!("voltex build: running `cargo build --release` (game crate) ...");
    let output = Command::new(cargo)
        .args([
            "build",
            "--release",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(manifest_dir)
        // Diagnostics render to stderr (inherited); artifacts stream as JSON here.
        .output()
        .map_err(CliError::from)?;
    std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).ok();
    if !output.status.success() {
        return Err(CliError::CargoBuildFailed(output.status.code().unwrap_or(-1)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let exes = extract_executables(&stdout);
    let exe_src = pick_executable(&exes, &manifest.project.name)?;
    if exes.len() > 1 {
        eprintln!(
            "voltex build: note — {} executables built; shipping {}",
            exes.len(),
            exe_src.display()
        );
    }

    let dist_dir = manifest_dir.join("dist");
    std::fs::create_dir_all(&dist_dir)?;
    let exe_dst = dist_dir.join(if cfg!(windows) {
        format!("{}.exe", manifest.project.name)
    } else {
        manifest.project.name.clone()
    });
    std::fs::copy(exe_src, &exe_dst)?;
    eprintln!("voltex build: wrote {}", exe_dst.display());

    copy_assets(manifest_dir, &dist_dir, manifest)?;

    if let Some(engine_path) = engine_path {
        let licenses = copy_license_files(engine_path, &dist_dir)?;
        if !licenses.is_empty() {
            eprintln!("voltex build: bundled {} license file(s)", licenses.len());
        }
    }

    Ok(())
}

/// Choose which built executable to ship: the one whose file stem matches the
/// manifest's project name (multi-bin crates build asset generators etc. too).
/// Without a name match, a single artifact is still unambiguous; several are
/// an error rather than a guess.
fn pick_executable<'a>(exes: &'a [PathBuf], project_name: &str) -> Result<&'a PathBuf> {
    if let Some(exe) = exes
        .iter()
        .find(|p| p.file_stem().and_then(|s| s.to_str()) == Some(project_name))
    {
        return Ok(exe);
    }
    match exes {
        [] => Err(CliError::Io(
            "cargo build succeeded but produced no executable — is the game crate a [[bin]]?"
                .to_string(),
        )),
        [only] => Ok(only),
        _ => Err(CliError::AmbiguousExecutable {
            project: project_name.to_string(),
            candidates: exes.to_vec(),
        }),
    }
}

/// Pull every non-null `"executable":"…"` value out of cargo's
/// `--message-format=json` stdout (one JSON object per line). A hand-rolled
/// scan — the CLI's zero-dependency policy rules out serde, and the field is
/// a plain JSON string (backslashes/quotes arrive `\\`/`\"`-escaped).
fn extract_executables(cargo_json_stdout: &str) -> Vec<PathBuf> {
    const KEY: &str = "\"executable\":\"";
    let mut out = Vec::new();
    for line in cargo_json_stdout.lines() {
        let Some(start) = line.find(KEY) else { continue };
        let rest = &line[start + KEY.len()..];
        let mut path = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some('\\') => path.push('\\'),
                    Some('"') => path.push('"'),
                    Some('/') => path.push('/'),
                    Some(other) => {
                        path.push('\\');
                        path.push(other);
                    }
                    None => break,
                },
                other => path.push(other),
            }
        }
        if !path.is_empty() {
            out.push(PathBuf::from(path));
        }
    }
    out
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

    // scripts / assets: dist 레이아웃은 manifest 기준 상대 경로를 보존한다
    // (`assets/levels` → `dist/assets/levels`). file_name 평탄화는
    // 중첩 선언을 잘못된 위치로 옮기고 (`dist/levels`), 마지막 컴포넌트가
    // 같은 두 선언을 합쳐버린다 — .vpkg 키(strip_prefix 보존)와도 어긋남.
    for dir in manifest
        .resolved_script_dirs(manifest_dir)
        .into_iter()
        .chain(manifest.resolved_assets_dirs(manifest_dir))
    {
        let rel = dir.strip_prefix(manifest_dir).unwrap_or(&dir);
        let dst = dist_dir.join(rel);
        copy_optional_dir(&dir, &dst)?;
    }
    Ok(())
}

/// v1.0.12: copy `THIRD-PARTY-LICENSES.md` and `LICENSE-*` files from the
/// engine repo into `dist_dir` so the shipped artifact carries its own
/// dependency-license attribution. Missing files are logged but never
/// fail the build (older engine checkouts predate the inventory).
///
/// Returns the list of license files actually copied (relative to
/// `dist_dir`) for caller-side logging / testing.
pub fn copy_license_files(engine_path: &Path, dist_dir: &Path) -> Result<Vec<PathBuf>> {
    let candidates = ["THIRD-PARTY-LICENSES.md", "LICENSE-MIT", "LICENSE-APACHE"];
    std::fs::create_dir_all(dist_dir)?;
    let mut copied = Vec::new();
    for name in candidates {
        let src = engine_path.join(name);
        if src.is_file() {
            let dst = dist_dir.join(name);
            std::fs::copy(&src, &dst)?;
            copied.push(PathBuf::from(name));
        } else {
            eprintln!(
                "voltex build: note — license file {} not found at {}",
                name,
                src.display()
            );
        }
    }
    Ok(copied)
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

/// Walk the manifest's scene + scripts + assets dirs and write a `.vpkg`.
/// Paths inside the archive use forward slashes and are relative to the
/// manifest directory (so `with_initial_scene("scene.vscn")` works).
///
/// The scene is additionally scanned for persisted asset references
/// (`texture_path` / `mesh_path` — the editor's apply-and-save workflow);
/// referenced files outside the declared asset dirs are auto-bundled, and
/// references missing from disk produce a warning (the build continues — the
/// runtime falls back to the white default texture / skips the mesh).
pub fn pack_archive(manifest_dir: &Path, manifest: &Manifest, out_path: &Path) -> Result<()> {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

    // 1. scene
    let scene_path = manifest_dir.join(&manifest.build.scene);
    let mut scene_bytes: Option<Vec<u8>> = None;
    if scene_path.exists() {
        let bytes = std::fs::read(&scene_path)?;
        entries.push((manifest.build.scene.clone(), bytes.clone()));
        scene_bytes = Some(bytes);
    }

    // 2. scripts
    for dir in manifest.resolved_script_dirs(manifest_dir) {
        collect_into_entries(&dir, manifest_dir, &mut entries)?;
    }
    // 3. assets
    for dir in manifest.resolved_assets_dirs(manifest_dir) {
        collect_into_entries(&dir, manifest_dir, &mut entries)?;
    }

    // 4. scene-referenced assets not covered by the declared dirs.
    if let Some(bytes) = scene_bytes {
        for path in scan_scene_asset_paths(&bytes) {
            // 아카이브 키는 forward-slash 규약 (collect_into_entries / 런타임
            // read_asset과 동일) — 백슬래시 표기 씬 경로도 같은 키로 정규화.
            let key = path.replace('\\', "/");
            if entries.iter().any(|(k, _)| *k == key) {
                continue; // already bundled via an assets dir
            }
            let on_disk = manifest_dir.join(&path);
            match std::fs::read(&on_disk) {
                Ok(data) => {
                    eprintln!("voltex build: auto-bundled scene asset `{}`", key);
                    entries.push((key, data));
                }
                Err(_) => eprintln!(
                    "voltex build: warning: scene references missing asset `{}` \
                     (not in the archive nor at {})",
                    path,
                    on_disk.display()
                ),
            }
        }
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

/// Marker components the raw deserializers below attach so the scan can
/// collect paths through a `World` query (`register_raw` takes plain fn
/// pointers — no closure capture).
struct TexPathRef(String);
struct MeshPathRef(String);
struct AudioPathRef(String);

fn deser_path_ref<T: 'static>(
    make: fn(String) -> T,
) -> impl Fn(&mut voltex_ecs::World, voltex_ecs::Entity, &[u8]) -> voltex_ecs::Result<()> {
    move |world, entity, data| {
        let s = std::str::from_utf8(data)
            .map_err(|_| "asset path: invalid UTF-8")?
            .to_string();
        world.add(entity, make(s));
        Ok(())
    }
}

fn deser_tex(
    world: &mut voltex_ecs::World,
    entity: voltex_ecs::Entity,
    data: &[u8],
) -> voltex_ecs::Result<()> {
    deser_path_ref(TexPathRef)(world, entity, data)
}

fn deser_mesh(
    world: &mut voltex_ecs::World,
    entity: voltex_ecs::Entity,
    data: &[u8],
) -> voltex_ecs::Result<()> {
    deser_path_ref(MeshPathRef)(world, entity, data)
}

fn deser_audio(
    world: &mut voltex_ecs::World,
    entity: voltex_ecs::Entity,
    data: &[u8],
) -> voltex_ecs::Result<()> {
    deser_path_ref(AudioPathRef)(world, entity, data)
}

/// Extract the `texture_path` / `mesh_path` / `audio_path` component payloads
/// from a scene file. Format auto-detection: `VSCN` magic → binary, first
/// non-whitespace `{` → JSON, anything else → legacy `.vscn` text (which
/// never carries registry components — empty result is correct). Unknown
/// components in the scene are skipped by the registry contract; parse
/// failures yield an empty list (the build warns nothing — a malformed scene
/// already fails at runtime with a clearer error).
pub fn scan_scene_asset_paths(scene_bytes: &[u8]) -> Vec<String> {
    let mut registry = voltex_ecs::ComponentRegistry::new();
    registry.register_raw("texture_path", |_, _| None, deser_tex);
    registry.register_raw("mesh_path", |_, _| None, deser_mesh);
    registry.register_raw("audio_path", |_, _| None, deser_audio);

    let mut world = voltex_ecs::World::new();
    if scene_bytes.starts_with(b"VSCN") {
        let _ = voltex_ecs::deserialize_scene_binary(&mut world, scene_bytes, &registry);
    } else if let Ok(text) = std::str::from_utf8(scene_bytes) {
        if text.trim_start().starts_with('{') {
            let _ = voltex_ecs::deserialize_scene_json(&mut world, text, &registry);
        }
        // else: legacy text scenes carry no registry components.
    }

    let mut paths: Vec<String> = world
        .query::<TexPathRef>()
        .map(|(_, p)| p.0.clone())
        .chain(world.query::<MeshPathRef>().map(|(_, p)| p.0.clone()))
        .chain(world.query::<AudioPathRef>().map(|(_, p)| p.0.clone()))
        .collect();
    paths.sort();
    paths.dedup();
    paths
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

    #[test]
    fn pick_executable_prefers_project_name_match() {
        let exes = vec![
            PathBuf::from(r"t\release\voltex_depths.exe"),
            PathBuf::from(r"t\release\gen_sfx.exe"),
        ];
        // Name match wins regardless of artifact order.
        assert_eq!(pick_executable(&exes, "voltex_depths").unwrap(), &exes[0]);
    }

    #[test]
    fn pick_executable_single_unmatched_artifact_is_shipped() {
        // [[bin]] renamed away from project.name — the only candidate is
        // still unambiguous.
        let exes = vec![PathBuf::from(r"t\release\game_main.exe")];
        assert_eq!(pick_executable(&exes, "mygame").unwrap(), &exes[0]);
    }

    #[test]
    fn pick_executable_errors_when_no_name_match_among_several() {
        // Guessing here could ship an asset generator as the game.
        let exes = vec![
            PathBuf::from(r"t\release\game_main.exe"),
            PathBuf::from(r"t\release\gen_sfx.exe"),
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
}
