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
    let skip = build_output_dirs(manifest_dir);
    for dir in manifest
        .resolved_script_dirs(manifest_dir)
        .into_iter()
        .chain(manifest.resolved_assets_dirs(manifest_dir))
    {
        let rel = dir.strip_prefix(manifest_dir).unwrap_or(&dir);
        let dst = dist_dir.join(rel);
        copy_optional_dir(&dir, &dst, &skip)?;
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
    let skip = build_output_dirs(manifest_dir);
    for dir in manifest.resolved_script_dirs(manifest_dir) {
        collect_into_entries(&dir, manifest_dir, &skip, &mut entries)?;
    }
    // 3. assets
    for dir in manifest.resolved_assets_dirs(manifest_dir) {
        collect_into_entries(&dir, manifest_dir, &skip, &mut entries)?;
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
/// non-whitespace `{` (after an optional UTF-8 BOM) → JSON, anything else →
/// legacy `.vscn` text (which
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
        if text.trim_start_matches('\u{FEFF}').trim_start().starts_with('{') {
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

/// Build output directories under the project root. A root-level glob
/// (`assets = ["./**/*"]`) resolves to the project root itself, so walks
/// must skip these or they pack/copy previous builds (and `dist/` into
/// itself until the stack overflows).
fn build_output_dirs(manifest_dir: &Path) -> Vec<PathBuf> {
    vec![manifest_dir.join("dist"), manifest_dir.join("target")]
}

fn collect_into_entries(
    dir: &Path,
    relative_to: &Path,
    skip: &[PathBuf],
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
            if skip.contains(&path) {
                continue;
            }
            collect_into_entries(&path, relative_to, skip, out)?;
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
mod tests;
