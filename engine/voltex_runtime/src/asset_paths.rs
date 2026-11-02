//! Resolve scene-persisted asset *paths* ([`TexturePath`] / [`MeshPath`])
//! into live GPU handles at runtime, so scenes authored in the editor render
//! their applied assets outside it too.
//!
//! The core ([`resolve_asset_paths_with`]) takes injected loaders and is
//! GPU-free for headless testing; [`resolve_asset_paths`] is the real wrapper
//! (fs read → upload). Both are idempotent: entities that already hold a
//! handle are skipped, so the per-frame call is cheap once resolved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voltex_ecs::{MeshHandle, World};
use voltex_renderer_ecs::{MeshPath, RenderAssets, TextureHandle, TexturePath};

/// Canonical cache-key form of an asset path: forward slashes, no leading
/// `./`. `with_audio_files` seeds, editor-persisted paths, and Vfs keys all
/// meet in the caches — without one spelling, the same file can occupy two
/// slots (double decode, hot-reload split). Loaders still receive the
/// original path (filesystem resolution handles either spelling).
pub(crate) fn normalize_path_key(path: &Path) -> PathBuf {
    let mut s = path.to_string_lossy().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_string();
    }
    PathBuf::from(s)
}

/// Shared-handle + failure cache for [`resolve_asset_paths_with`].
///
/// Success entries make entities pointing at the same file share one handle
/// and are kept for the app's lifetime. Failure entries (`None`) stop the
/// resolver from re-reading a broken path every frame; clear them with
/// [`Self::clear_failures`] when a scene (re)load warrants a retry.
#[derive(Default)]
pub struct AssetPathCache {
    textures: HashMap<PathBuf, Option<TextureHandle>>,
    meshes: HashMap<PathBuf, Option<MeshHandle>>,
}

impl AssetPathCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop failure entries (`None`) so the next resolve pass retries them.
    /// Successful handles are kept — they stay valid for the app's lifetime.
    pub fn clear_failures(&mut self) {
        self.textures.retain(|_, v| v.is_some());
        self.meshes.retain(|_, v| v.is_some());
    }
}

/// Injected-loader core of [`resolve_asset_paths`]. For every entity carrying
/// a [`TexturePath`] / [`MeshPath`] but **no** corresponding handle, load the
/// file through `load_texture` / `load_mesh` (each called at most once per
/// distinct path — results are cached, failures as `None`) and attach the
/// handle. Returns `(textures_attached, meshes_attached)`.
pub fn resolve_asset_paths_with(
    world: &mut World,
    cache: &mut AssetPathCache,
    mut load_texture: impl FnMut(&Path) -> Option<TextureHandle>,
    mut load_mesh: impl FnMut(&Path) -> Option<MeshHandle>,
) -> (usize, usize) {
    // Texture pass.
    let tex_targets: Vec<(voltex_ecs::Entity, PathBuf)> = world
        .query::<TexturePath>()
        .filter(|(e, _)| world.get::<TextureHandle>(*e).is_none())
        .map(|(e, p)| (e, PathBuf::from(&p.0)))
        .collect();
    let mut tex_n = 0;
    for (entity, path) in tex_targets {
        let slot = cache
            .textures
            .entry(normalize_path_key(&path))
            .or_insert_with(|| load_texture(&path));
        if let Some(h) = *slot {
            world.add(entity, h);
            tex_n += 1;
        }
    }

    // Mesh pass — Bounds is left untouched (the scene restores it).
    let mesh_targets: Vec<(voltex_ecs::Entity, PathBuf)> = world
        .query::<MeshPath>()
        .filter(|(e, _)| world.get::<MeshHandle>(*e).is_none())
        .map(|(e, p)| (e, PathBuf::from(&p.0)))
        .collect();
    let mut mesh_n = 0;
    for (entity, path) in mesh_targets {
        let slot = cache
            .meshes
            .entry(normalize_path_key(&path))
            .or_insert_with(|| load_mesh(&path));
        if let Some(h) = *slot {
            world.add(entity, h);
            mesh_n += 1;
        }
    }

    (tex_n, mesh_n)
}

/// Real resolver: reads asset bytes **Vfs-first with fs fallback** (the same
/// contract as scene/script loading, so `.vpkg`-bundled assets work without
/// loose files) and uploads them (PNG/JPG textures via
/// `RenderAssets::add_texture_png`, OBJ/glTF meshes via `Mesh::from_obj` /
/// `from_gltf`). Failures are logged once and cached as `None` — see
/// [`AssetPathCache`].
pub fn resolve_asset_paths(
    world: &mut World,
    assets: &mut RenderAssets,
    cache: &mut AssetPathCache,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    vfs: Option<&voltex_asset::Vfs>,
    asset_root: Option<&Path>,
) -> (usize, usize) {
    // The two loader closures need `assets` mutably but never concurrently —
    // resolve_asset_paths_with runs the texture pass before the mesh pass.
    // Split via RefCell to satisfy the borrow checker without unsafe.
    let assets = std::cell::RefCell::new(assets);
    let (tex_n, mesh_n) = resolve_asset_paths_with(
        world,
        cache,
        |path| match read_asset(vfs, asset_root, path).and_then(|bytes| {
            assets
                .borrow_mut()
                .add_texture_png(device, queue, &bytes)
                .map_err(|e| e.to_string())
        }) {
            Ok(h) => Some(h),
            Err(e) => {
                crate::log_error!(
                    "voltex_runtime: texture {} failed to resolve: {e}",
                    path.display()
                );
                None
            }
        },
        |path| match read_asset(vfs, asset_root, path).and_then(|bytes| decode_mesh(device, path, &bytes)) {
            Ok(mesh) => Some(assets.borrow_mut().add_mesh(mesh)),
            Err(e) => {
                crate::log_error!(
                    "voltex_runtime: mesh {} failed to resolve: {e}",
                    path.display()
                );
                None
            }
        },
    );
    // Positive evidence for hands-off verification (dist-exe E2E greps this).
    // Idempotence keeps this quiet after the first resolving frame.
    if tex_n > 0 || mesh_n > 0 {
        crate::log_info!(
            "voltex_runtime: resolved {tex_n} texture path(s), {mesh_n} mesh path(s)"
        );
    }
    (tex_n, mesh_n)
}

/// Read asset bytes: the Vfs (when present **and** it has the entry) wins,
/// then a **relative** path is tried under `asset_root` (cwd-independent game
/// assets, friction #6), then plain fs as the last resort. The Vfs key is the
/// path's forward-slash string form — matching the editor's persisted format
/// (v1.0.99).
pub(crate) fn read_asset(
    vfs: Option<&voltex_asset::Vfs>,
    asset_root: Option<&Path>,
    path: &Path,
) -> Result<Vec<u8>, String> {
    if let Some(vfs) = vfs {
        let key = path.to_string_lossy().replace('\\', "/");
        if vfs.exists(&key) {
            return vfs.read(&key).map_err(|e| e.to_string());
        }
    }
    if path.is_relative() {
        if let Some(root) = asset_root {
            let rooted = root.join(path);
            match std::fs::read(&rooted) {
                Ok(bytes) => return Ok(bytes),
                // 파일이 root에 존재하는데 다른 이유(권한/공유 위반)로 못
                // 읽었다면 그 에러를 그대로 — cwd 폴백은 진단을 어긋나게
                // 하고, cwd의 동명 파일을 잘못 로드할 수 있다.
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(format!("{}: {e}", rooted.display()));
                }
                Err(_) => {}
            }
        }
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

/// Decode mesh bytes by the path's extension (`.obj` / `.gltf` / `.glb`).
fn decode_mesh(
    device: &wgpu::Device,
    path: &Path,
    bytes: &[u8],
) -> Result<voltex_renderer::Mesh, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("obj") => {
            let source = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            Ok(voltex_renderer::Mesh::from_obj(device, source))
        }
        Some("gltf" | "glb") => voltex_renderer::Mesh::from_gltf(device, bytes),
        _ => Err(format!("unsupported mesh extension: {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_asset::Handle;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;

    fn tex_handle(id: u32) -> TextureHandle {
        TextureHandle(Handle::from_raw_parts(id, 0))
    }

    fn mesh_handle(id: u32) -> MeshHandle {
        MeshHandle(Handle::from_raw_parts(id, 0))
    }

    #[test]
    fn attaches_handles_for_path_entities_and_shares_per_path() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        for e in [a, b] {
            world.add(e, Transform::from_position(Vec3::ZERO));
            world.add(e, TexturePath("tex.png".into()));
        }
        let m = world.spawn();
        world.add(m, MeshPath("ship.obj".into()));

        let mut cache = AssetPathCache::new();
        let mut tex_calls = 0;
        let (tex_n, mesh_n) = resolve_asset_paths_with(
            &mut world,
            &mut cache,
            |_| {
                tex_calls += 1;
                Some(tex_handle(7))
            },
            |_| Some(mesh_handle(3)),
        );

        assert_eq!((tex_n, mesh_n), (2, 1));
        assert_eq!(tex_calls, 1, "two entities, same path → one load");
        assert_eq!(world.get::<TextureHandle>(a).unwrap().0.id(), 7);
        assert_eq!(world.get::<TextureHandle>(b).unwrap().0.id(), 7);
        assert_eq!(world.get::<MeshHandle>(m).unwrap().0.id(), 3);
    }

    #[test]
    fn entities_with_handles_are_skipped_idempotent() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, TexturePath("tex.png".into()));
        world.add(e, tex_handle(1)); // already resolved

        let mut cache = AssetPathCache::new();
        let mut calls = 0;
        let (tex_n, _) = resolve_asset_paths_with(
            &mut world,
            &mut cache,
            |_| {
                calls += 1;
                Some(tex_handle(9))
            },
            |_| None,
        );
        assert_eq!(tex_n, 0);
        assert_eq!(calls, 0, "loader must not run for resolved entities");
        assert_eq!(world.get::<TextureHandle>(e).unwrap().0.id(), 1, "untouched");
    }

    #[test]
    fn read_asset_prefers_vfs_and_falls_back_to_fs() {
        let dir = std::env::temp_dir().join(format!("voltex_read_asset_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let loose = dir.join("loose.png");
        std::fs::write(&loose, b"from-fs").unwrap();

        // Archive vfs with one entry under the editor's canonical key form.
        let bytes = voltex_asset::write_vpkg(&[(
            "assets/hero.png".to_string(),
            b"from-vfs".to_vec(),
        )]);
        let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let vfs = voltex_asset::Vfs::archive_from_bytes(leaked).expect("vpkg parses");

        // In the archive → vfs wins (no fs file needed).
        assert_eq!(
            read_asset(Some(&vfs), None, Path::new("assets/hero.png")).unwrap(),
            b"from-vfs"
        );
        // Backslash spelling normalizes onto the same archive key.
        assert_eq!(
            read_asset(Some(&vfs), None, Path::new("assets\\hero.png")).unwrap(),
            b"from-vfs"
        );
        // Not in the archive → fs fallback.
        assert_eq!(read_asset(Some(&vfs), None, &loose).unwrap(), b"from-fs");
        // Present in neither → Err.
        assert!(read_asset(Some(&vfs), None, Path::new("missing.png")).is_err());
        // No vfs at all → plain fs behaviour.
        assert_eq!(read_asset(None, None, &loose).unwrap(), b"from-fs");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_asset_resolves_relative_paths_against_asset_root() {
        let root =
            std::env::temp_dir().join(format!("voltex_asset_root_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("textures")).unwrap();
        std::fs::write(root.join("textures/hero.png"), b"rooted").unwrap();

        // Relative path + asset root → resolves under the root, regardless of
        // cwd (the file does not exist relative to the test cwd).
        assert_eq!(
            read_asset(None, Some(&root), Path::new("textures/hero.png")).unwrap(),
            b"rooted"
        );
        // Absolute paths bypass the root.
        assert_eq!(
            read_asset(None, Some(&root), &root.join("textures/hero.png")).unwrap(),
            b"rooted"
        );
        // Missing everywhere → Err.
        assert!(read_asset(None, Some(&root), Path::new("textures/nope.png")).is_err());
        // Vfs still wins over the root for bundled entries.
        let bytes = voltex_asset::write_vpkg(&[(
            "textures/hero.png".to_string(),
            b"from-vfs".to_vec(),
        )]);
        let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let vfs = voltex_asset::Vfs::archive_from_bytes(leaked).expect("vpkg parses");
        assert_eq!(
            read_asset(Some(&vfs), Some(&root), Path::new("textures/hero.png")).unwrap(),
            b"from-vfs"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn failures_cache_and_dont_retry_until_cleared() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, MeshPath("missing.obj".into()));

        let mut cache = AssetPathCache::new();
        let mut calls = 0;
        let run = |world: &mut World, cache: &mut AssetPathCache, calls: &mut u32| {
            resolve_asset_paths_with(world, cache, |_| None, |_| {
                *calls += 1;
                None
            })
        };

        assert_eq!(run(&mut world, &mut cache, &mut calls), (0, 0));
        assert_eq!(run(&mut world, &mut cache, &mut calls), (0, 0));
        assert_eq!(calls, 1, "second pass must hit the failure cache");

        cache.clear_failures();
        assert_eq!(run(&mut world, &mut cache, &mut calls), (0, 0));
        assert_eq!(calls, 2, "clear_failures re-enables the retry");
    }
}
