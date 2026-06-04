//! Resolve scene-persisted audio *paths* ([`AudioPath`]) into live
//! [`AudioSystem`](voltex_audio::AudioSystem) clip ids at runtime, so scenes
//! authored in the editor play their applied clips outside it too.
//!
//! Mirror of [`asset_paths`](crate::asset_paths) for audio: an
//! `AudioSource::clip_id` is a runtime clip-table index (meaningless across
//! runs, like a `TextureHandle`), while the [`AudioPath`] survives in scenes
//! and is re-resolved after load. The core
//! ([`resolve_audio_paths_with`]) takes an injected loader and runs headless;
//! [`resolve_audio_paths`] is the real wrapper (Vfs-first read → decode →
//! `add_clip`). Both are idempotent: a source already bound to its path's
//! clip id is skipped, so the per-frame call is cheap once resolved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voltex_audio::{AudioSource, AudioSystem};
use voltex_ecs::World;

/// ECS component: the source *file path* (`.wav` / `.ogg`) of the clip an
/// entity's [`AudioSource`] plays. Same persistence contract as
/// `TexturePath` / `MeshPath`: the clip id is a runtime slot, the path
/// survives in scenes and is re-resolved after load.
///
/// **Binding contract**: while an entity carries an `AudioPath`, its
/// `AudioSource::clip_id` is owned by the resolver — game code that wants to
/// drive `clip_id` manually must remove the `AudioPath` first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPath(pub String);

voltex_ecs::path_component!(AudioPath, "audio_path");

/// Shared-id + failure cache for [`resolve_audio_paths_with`].
///
/// Success entries make entities pointing at the same file share one clip id
/// and are kept for the app's lifetime (`AudioSystem` clips are append-only).
/// Failure entries (`None`) stop the resolver from re-reading a broken path
/// every frame; clear them with [`Self::clear_failures`] when a scene
/// (re)load warrants a retry.
#[derive(Default)]
pub struct AudioClipCache {
    clips: HashMap<PathBuf, Option<u32>>,
}

impl AudioClipCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop failure entries (`None`) so the next resolve pass retries them.
    pub fn clear_failures(&mut self) {
        self.clips.retain(|_, v| v.is_some());
    }
}

/// Injected-loader core of [`resolve_audio_paths`]. For every entity carrying
/// an [`AudioPath`], load the file through `load_clip` (called at most once
/// per distinct path — results are cached, failures as `None`) and bind the
/// clip id:
///
/// - no [`AudioSource`] yet → attach `AudioSource::new(id)` (stopped — the
///   scene's persisted `playing` flag or game code decides playback),
/// - existing source with a different `clip_id` (e.g. a stale id persisted by
///   the editor) → rebind to the resolved id.
///
/// Returns the number of sources attached or rebound — 0 from the second
/// call on (idempotent).
pub fn resolve_audio_paths_with(
    world: &mut World,
    cache: &mut AudioClipCache,
    mut load_clip: impl FnMut(&Path) -> Option<u32>,
) -> usize {
    let targets: Vec<(voltex_ecs::Entity, PathBuf)> = world
        .query::<AudioPath>()
        .map(|(e, p)| (e, PathBuf::from(&p.0)))
        .collect();
    let mut bound = 0;
    for (entity, path) in targets {
        let slot = cache
            .clips
            .entry(path.clone())
            .or_insert_with(|| load_clip(&path));
        let Some(id) = *slot else { continue };
        match world.get_mut::<AudioSource>(entity) {
            Some(src) => {
                if src.clip_id.0 != id {
                    src.clip_id = voltex_audio::ClipId(id);
                    bound += 1;
                }
            }
            None => {
                world.add(entity, AudioSource::new(id));
                bound += 1;
            }
        }
    }
    bound
}

/// Real resolver: reads clip bytes **Vfs-first with fs fallback** (the same
/// contract as scene/texture loading, so `.vpkg`-bundled audio works without
/// loose files), decodes by extension
/// ([`decode_clip_bytes`](voltex_audio::decode_clip_bytes)) and registers the
/// clip with [`AudioSystem::add_clip`]. Failures are logged once and cached
/// as `None` — see [`AudioClipCache`].
pub fn resolve_audio_paths(
    world: &mut World,
    audio: &mut AudioSystem,
    cache: &mut AudioClipCache,
    vfs: Option<&voltex_asset::Vfs>,
) -> usize {
    let n = resolve_audio_paths_with(world, cache, |path| {
        match crate::asset_paths::read_asset(vfs, path)
            .and_then(|bytes| {
                voltex_audio::decode_clip_bytes(path, &bytes).map_err(|e| e.to_string())
            }) {
            Ok(clip) => Some(audio.add_clip(clip) as u32),
            Err(e) => {
                crate::log_error!(
                    "voltex_runtime: audio {} failed to resolve: {e}",
                    path.display()
                );
                None
            }
        }
    });
    // Positive evidence for hands-off verification (same contract as the
    // texture/mesh resolver). Idempotence keeps this quiet after the first
    // resolving frame.
    if n > 0 {
        crate::log_info!("voltex_runtime: resolved {n} audio path(s)");
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::Transform;

    fn world_with_audio_path(path: &str) -> (World, voltex_ecs::Entity) {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::default());
        world.add(e, AudioPath(path.to_string()));
        (world, e)
    }

    #[test]
    fn attaches_source_and_shares_clip_id_per_path() {
        let (mut world, a) = world_with_audio_path("sounds/boom.wav");
        let b = world.spawn();
        world.add(b, AudioPath("sounds/boom.wav".to_string()));
        let c = world.spawn();
        world.add(c, AudioPath("sounds/zap.wav".to_string()));

        let mut cache = AudioClipCache::new();
        let mut loads = 0;
        let n = resolve_audio_paths_with(&mut world, &mut cache, |_| {
            loads += 1;
            Some(loads - 1) // boom → 0, zap → 1
        });
        assert_eq!(n, 3, "three sources bound");
        assert_eq!(loads, 2, "one load per distinct path");
        assert_eq!(world.get::<AudioSource>(a).unwrap().clip_id.0, 0);
        assert_eq!(world.get::<AudioSource>(b).unwrap().clip_id.0, 0, "same path shares id");
        assert_eq!(world.get::<AudioSource>(c).unwrap().clip_id.0, 1);
        assert!(!world.get::<AudioSource>(a).unwrap().playing, "attached source starts stopped");

        // Second pass: everything bound — no loads, no rebinds.
        let n2 = resolve_audio_paths_with(&mut world, &mut cache, |_| {
            panic!("cache hit expected — loader must not run")
        });
        assert_eq!(n2, 0, "idempotent");
    }

    #[test]
    fn rebinds_stale_persisted_clip_id_and_keeps_flags() {
        // Editor persisted clip_id 7 — meaningless in this run, where the
        // path resolves to clip 0. Volume/looping/playing must survive.
        let (mut world, e) = world_with_audio_path("music/bgm.ogg");
        let mut src = AudioSource::new(7u32).looping();
        src.volume = 0.6;
        src.playing = true;
        world.add(e, src);

        let mut cache = AudioClipCache::new();
        let n = resolve_audio_paths_with(&mut world, &mut cache, |_| Some(0));
        assert_eq!(n, 1, "stale id rebound");
        let got = world.get::<AudioSource>(e).unwrap();
        assert_eq!(got.clip_id.0, 0);
        assert!(got.looping && got.playing, "playback flags untouched");
        assert!((got.volume - 0.6).abs() < 1e-6);
    }

    #[test]
    fn failure_is_cached_until_cleared() {
        let (mut world, e) = world_with_audio_path("missing.wav");
        let mut cache = AudioClipCache::new();
        let mut loads = 0;
        let fail = |loads: &mut u32| {
            *loads += 1;
            None
        };
        assert_eq!(resolve_audio_paths_with(&mut world, &mut cache, |_| fail(&mut loads)), 0);
        assert_eq!(resolve_audio_paths_with(&mut world, &mut cache, |_| fail(&mut loads)), 0);
        assert_eq!(loads, 1, "failure cached — no per-frame retry");
        assert!(world.get::<AudioSource>(e).is_none());

        cache.clear_failures();
        let n = resolve_audio_paths_with(&mut world, &mut cache, |_| Some(3));
        assert_eq!(n, 1, "clear_failures retries the path");
        assert_eq!(world.get::<AudioSource>(e).unwrap().clip_id.0, 3);
    }

    #[test]
    fn real_resolver_reads_wav_from_vpkg_vfs() {
        // The path exists only inside the archive — fs fallback cannot serve
        // it, proving the Vfs branch (mirrors the texture resolver test).
        let wav = voltex_audio::generate_wav_bytes(&[0.5, -0.5, 0.25], 44_100);
        let archive = voltex_asset::write_vpkg(&[("sounds/beep.wav".to_string(), wav)]);
        // archive_from_bytes wants `&'static` (include_bytes! shape) — leak
        // the tiny test buffer, same as the voltex_asset vfs tests.
        let bytes: &'static [u8] = Box::leak(archive.into_boxed_slice());
        let vfs = voltex_asset::Vfs::archive_from_bytes(bytes).expect("vfs");

        let (mut world, e) = world_with_audio_path("sounds/beep.wav");
        // Headless: AudioSystem::new spawns its thread without a device.
        let mut audio = AudioSystem::new(Vec::new()).expect("audio system");
        let mut cache = AudioClipCache::new();

        let n = resolve_audio_paths(&mut world, &mut audio, &mut cache, Some(&vfs));
        assert_eq!(n, 1, "clip resolved from vpkg");
        assert_eq!(world.get::<AudioSource>(e).unwrap().clip_id.0, 0);

        // Missing path (nowhere) → failure cached, source absent, no panic.
        let m = world.spawn();
        world.add(m, AudioPath("sounds/gone.wav".to_string()));
        let n2 = resolve_audio_paths(&mut world, &mut audio, &mut cache, Some(&vfs));
        assert_eq!(n2, 0);
        assert!(world.get::<AudioSource>(m).is_none());
    }
}
