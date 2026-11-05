//! Boot steps of [`RuntimeApp::new`], one function per job, in the order `new`
//! runs them: component registry → initial World → audio from config →
//! (app struct) → Lua scripts seeded from the Vfs.

use std::path::PathBuf;

use voltex_audio::{AudioClip, AudioSystem};
use voltex_ecs::{ComponentRegistry, World};

use super::RuntimeApp;
use crate::config::{RegistrySetupFn, RuntimeConfig};
use crate::error::{RuntimeError, RuntimeResult};

/// Component registry for scene (de)serialisation: ECS defaults, the
/// runtime-owned components, renderables, then the game's own
/// `with_registry_setup` callback (last, so it can override).
pub(super) fn build_registry(register_components: Option<RegistrySetupFn>) -> ComponentRegistry {
    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    // AudioSource lives in voltex_audio (no ECS dep), so it is registered
    // here via the register_raw escape hatch rather than register_defaults.
    crate::audio_sync::register_audio_source(&mut registry);
    // AudioPath (scene-persisted clip file path) is runtime-owned.
    registry.register::<crate::audio_paths::AudioPath>();
    // Lifetime — 미등록이면 저장 씬에서 소멸 조건만 떨어져 나가
    // 로드 시 one-shot 엔티티가 불멸이 된다 (감사 발견).
    registry.register::<crate::lifetime::Lifetime>();
    // Declarative renderables (PrimitiveMesh / MaterialDesc) live in
    // voltex_renderer_ecs (L3), so they register here rather than in the
    // L1 register_defaults. Materialized to GPU resources in gpu_init.
    voltex_renderer_ecs::register_renderable_components(&mut registry);
    if let Some(setup) = register_components {
        setup(&mut registry);
    }
    registry
}

/// The World the app starts with: `config.initial_scene` (if any) loaded via
/// Vfs-or-disk, then the `with_scene_builder` callback run on top.
///
/// A broken initial scene degrades to an empty world (the builder may still
/// populate it) but is logged — never silent.
pub(super) fn load_initial_world(
    config: &mut RuntimeConfig,
    registry: &mut ComponentRegistry,
) -> World {
    let mut world = World::new();
    if let Some(scene_path) = config.initial_scene.as_ref() {
        crate::crash::set_crash_scene(scene_path.as_str());
        // The dist-exe E2E reads these two lines as its load evidence — keep
        // the wording.
        match load_scene_via_vfs_or_disk(&mut world, registry, scene_path, config.vfs.as_ref()) {
            Ok(()) => crate::log_info!(
                "voltex_runtime: initial scene `{}` loaded ({} entities)",
                scene_path,
                world.entity_count()
            ),
            Err(e) => crate::log_error!(
                "voltex_runtime: initial scene `{}` failed to load: {e}",
                scene_path
            ),
        }
    }
    if let Some(build) = config.build_scene.take() {
        build(&mut world, registry);
    }
    world
}

/// Result of [`load_configured_audio`].
#[derive(Default)]
pub(super) struct ConfiguredAudio {
    /// The mixer, if audio is enabled and its thread started.
    pub system: Option<AudioSystem>,
    /// `(resolved file path, clip index)` per `with_audio_files` entry — to
    /// register for hot reload once the app exists.
    pub hot_reload_watches: Vec<(PathBuf, usize)>,
    /// `(path as configured, clip index)` per `with_audio_files` entry — to
    /// seed the `AudioPath` cache so scenes naming the same file reuse it.
    pub path_cache_seeds: Vec<(PathBuf, usize)>,
}

/// Build the audio mixer up front (it only spawns a thread, no GPU / window
/// needed); failure degrades to silent rather than aborting boot. With audio
/// disabled, `config.audio_clips` / `audio_files` are left untouched.
///
/// `with_audio_files` paths are decoded here and appended after the
/// pre-loaded `with_audio` clips (ClipId = clip count + file position). A
/// relative path is anchored at `asset_root` only when the rooted file exists
/// (see [`crate::asset_io::root_relative_if_file`] — not the `read_asset`
/// rule, and no Vfs lookup).
pub(super) fn load_configured_audio(
    config: &mut RuntimeConfig,
    asset_root: Option<&std::path::Path>,
) -> ConfiguredAudio {
    if !config.enable_audio {
        return ConfiguredAudio::default();
    }
    let mut out = ConfiguredAudio::default();
    let mut clips = std::mem::take(&mut config.audio_clips);
    for (file_index, configured_path) in
        std::mem::take(&mut config.audio_files).into_iter().enumerate()
    {
        let clip_index = clips.len();
        let path = crate::asset_io::root_relative_if_file(asset_root, &configured_path);
        out.path_cache_seeds.push((configured_path, clip_index));
        match voltex_audio::load_clip_file(&path) {
            Ok(clip) => clips.push(clip),
            Err(e) => {
                // Silent placeholder keeps every later file's ClipId
                // stable; fixing the file hot-reloads it in place.
                crate::log_error!(
                    "voltex_runtime: audio file {} failed to decode ({e}); \
                     clip {} is a silent placeholder until the file is fixed",
                    path.display(),
                    file_index
                );
                clips.push(silent_placeholder_clip());
            }
        }
        out.hot_reload_watches.push((path, clip_index));
    }
    out.system = match AudioSystem::new(clips) {
        Ok(audio) => Some(audio),
        Err(e) => {
            crate::log_error!("voltex_runtime: audio init failed: {e}");
            None
        }
    };
    out
}

/// One silent mono sample at 44.1 kHz — stands in for an audio file that
/// failed to decode so later clip ids don't shift.
fn silent_placeholder_clip() -> AudioClip {
    AudioClip::new(vec![0.0], 44_100, 1)
}

impl RuntimeApp {
    /// Vfs(.vpkg)의 Lua 스크립트를 풀에 시드 — 씬/텍스처/오디오와 달리
    /// 스크립트만 Vfs를 우회해 dist exe가 루즈 .lua 없이는 무동작이던
    /// 출하 격차 해소 (감사 발견). No-op with scripts disabled or no Vfs.
    pub(super) fn seed_scripts_from_vfs(&mut self) {
        if !self.config.enable_scripts {
            return;
        }
        let Some(vfs) = self.config.vfs.as_ref() else { return };
        let mut seeded = 0usize;
        for key in vfs.list() {
            if key.ends_with(".lua") {
                if let Ok(bytes) = vfs.read(&key) {
                    self.script_pool
                        .seed_source(&key, String::from_utf8_lossy(&bytes).into_owned());
                    seeded += 1;
                }
            }
        }
        if seeded > 0 {
            crate::log_info!("voltex_runtime: seeded {seeded} Lua script(s) from Vfs");
        }
    }
}

/// Read a scene from the Vfs (if it has `path`) or disk, then decode it into
/// `world`: binary VSCN first, JSON as fallback. `asset_root` is not
/// consulted — scene paths resolve against the cwd.
pub(super) fn load_scene_via_vfs_or_disk(
    world: &mut World,
    registry: &ComponentRegistry,
    path: &str,
    vfs: Option<&voltex_asset::Vfs>,
) -> RuntimeResult<()> {
    let data = match vfs {
        Some(v) if v.exists(path) => v
            .read(path)
            .map_err(|e| RuntimeError::SceneLoad(format!("{}", e)))?,
        _ => std::fs::read(path).map_err(|e| RuntimeError::SceneLoad(e.to_string()))?,
    };
    // Try binary first, then JSON.
    if voltex_ecs::deserialize_scene_binary(world, &data, registry).is_ok() {
        return Ok(());
    }
    let text = std::str::from_utf8(&data).map_err(|e| RuntimeError::SceneLoad(e.to_string()))?;
    voltex_ecs::deserialize_scene_json(world, text, registry)
        .map_err(|e| RuntimeError::SceneLoad(format!("{}", e)))?;
    Ok(())
}
