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
            .entry(path.clone())
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
            .entry(path.clone())
            .or_insert_with(|| load_mesh(&path));
        if let Some(h) = *slot {
            world.add(entity, h);
            mesh_n += 1;
        }
    }

    (tex_n, mesh_n)
}

/// Real resolver: reads files from disk and uploads them (PNG/JPG textures via
/// `RenderAssets::add_texture_png`, OBJ/glTF meshes via `Mesh::from_obj` /
/// `from_gltf`). Failures are logged once and cached as `None` — see
/// [`AssetPathCache`]. Vfs-aware reads are future work (design doc).
pub fn resolve_asset_paths(
    world: &mut World,
    assets: &mut RenderAssets,
    cache: &mut AssetPathCache,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (usize, usize) {
    // The two loader closures need `assets` mutably but never concurrently —
    // resolve_asset_paths_with runs the texture pass before the mesh pass.
    // Split via RefCell to satisfy the borrow checker without unsafe.
    let assets = std::cell::RefCell::new(assets);
    resolve_asset_paths_with(
        world,
        cache,
        |path| match std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
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
        |path| match load_mesh_file(device, path) {
            Ok(mesh) => Some(assets.borrow_mut().add_mesh(mesh)),
            Err(e) => {
                crate::log_error!(
                    "voltex_runtime: mesh {} failed to resolve: {e}",
                    path.display()
                );
                None
            }
        },
    )
}

/// Decode a mesh file (`.obj` / `.gltf` / `.glb`) into a GPU mesh.
fn load_mesh_file(device: &wgpu::Device, path: &Path) -> Result<voltex_renderer::Mesh, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("obj") => {
            let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            Ok(voltex_renderer::Mesh::from_obj(device, &source))
        }
        Some("gltf" | "glb") => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            voltex_renderer::Mesh::from_gltf(device, &bytes)
        }
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
