//! `audio_sync_system` — bridges `AudioSource` ECS components to the
//! `voltex_audio::AudioSystem` mixer.
//!
//! `voltex_audio` has no ECS dependency by design, so this integration lives in
//! the runtime (which can depend on both). Each frame the system:
//!
//! 1. updates the mixer's listener from the listener entity's `Transform`, and
//! 2. issues `play` / `play_3d` / `stop` commands on the *edges* of each
//!    `AudioSource::playing` flag (tracked via `AudioSource::active`), so a held
//!    `playing == true` does not spawn a new voice every frame.

use voltex_audio::spatial::SpatialParams;
use voltex_audio::{AudioSource, AudioSystem};
use voltex_ecs::{Entity, Transform, World};
use voltex_math::{Vec3, Vec4};

/// Default 3D attenuation range used for spatial sources (per-source tuning is
/// deferred to v1.1).
const DEFAULT_MIN_DISTANCE: f32 = 1.0;
const DEFAULT_MAX_DISTANCE: f32 = 50.0;

/// What the sync system should tell the mixer for one source this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioAction {
    /// Do nothing — no playback edge this frame.
    None,
    /// Start the clip (`spatial` selects `play_3d` vs `play`).
    Play { spatial: bool },
    /// Stop the clip.
    Stop,
}

/// Pure edge detection: compare the desired `playing` flag against the
/// already-issued `active` flag. Extracted so the decision is unit-testable
/// without an audio device.
pub(crate) fn decide_action(playing: bool, active: bool, spatial: bool) -> AudioAction {
    match (playing, active) {
        (true, false) => AudioAction::Play { spatial },
        (false, true) => AudioAction::Stop,
        _ => AudioAction::None,
    }
}

/// One source's resolved command for this frame (owned, so the apply phase can
/// run after the World query borrow is released).
struct Plan {
    entity: Entity,
    action: AudioAction,
    clip_index: usize,
    volume: f32,
    looping: bool,
    position: Vec3,
}

/// Rotate a unit direction by a quaternion via its matrix form (voltex_math has
/// no direct `Quat * Vec3`).
fn rotate(rotation: voltex_math::Quat, dir: Vec3) -> Vec3 {
    let m = rotation.to_mat4();
    let v = m * Vec4::new(dir.x, dir.y, dir.z, 0.0);
    Vec3::new(v.x, v.y, v.z)
}

/// Drive the `AudioSystem` from `AudioSource` components for one frame.
///
/// `listener` is the entity whose `Transform` positions the 3D listener (e.g.
/// the camera or player); pass `None` to leave the listener unchanged.
pub fn audio_sync_system(world: &mut World, audio: &AudioSystem, listener: Option<Entity>) {
    // 1. Listener pose (forward = -Z, right = +X in the entity's local frame).
    if let Some(le) = listener {
        if let Some(t) = world.get::<Transform>(le) {
            let forward = rotate(t.rotation, Vec3::new(0.0, 0.0, -1.0));
            let right = rotate(t.rotation, Vec3::new(1.0, 0.0, 0.0));
            audio.set_listener(t.position, forward, right);
        }
    }

    // 2. Read phase: resolve each source's action. `world.get::<Transform>`
    //    nested inside the query is a second *shared* borrow, which is allowed.
    let mut plans: Vec<Plan> = Vec::new();
    for (entity, src) in world.query::<AudioSource>() {
        let action = decide_action(src.playing, src.active, src.spatial);
        if action == AudioAction::None {
            continue;
        }
        let position = world
            .get::<Transform>(entity)
            .map(|t| t.position)
            .unwrap_or(Vec3::ZERO);
        plans.push(Plan {
            entity,
            action,
            clip_index: src.clip_id.0 as usize,
            volume: src.volume,
            looping: src.looping,
            position,
        });
    }

    // 3. Apply phase: issue mixer commands + write back the `active` edge flag.
    for plan in plans {
        match plan.action {
            AudioAction::Play { spatial } => {
                if spatial {
                    let sp = SpatialParams::new(
                        plan.position,
                        DEFAULT_MIN_DISTANCE,
                        DEFAULT_MAX_DISTANCE,
                    );
                    audio.play_3d(plan.clip_index, plan.volume, plan.looping, sp);
                } else {
                    audio.play(plan.clip_index, plan.volume, plan.looping);
                }
                if let Some(src) = world.get_mut::<AudioSource>(plan.entity) {
                    src.active = true;
                }
            }
            AudioAction::Stop => {
                audio.stop(plan.clip_index);
                if let Some(src) = world.get_mut::<AudioSource>(plan.entity) {
                    src.active = false;
                }
            }
            AudioAction::None => {}
        }
    }
}

// ── Hot reload ──────────────────────────────────────────────────────────────

/// Poll `watcher` once and hot-reload every changed file that `watched` maps
/// to a clip index: re-decode via [`voltex_audio::load_clip_file`] and swap
/// the clip in place with [`AudioSystem::replace_clip`] (same index — looping
/// BGM picks the new data up on the next mix block; one-shots on next play).
///
/// Returns one `(path, outcome)` per *watched* changed file so the caller can
/// log / surface results. Changes to unwatched paths are ignored. A decode
/// failure leaves the old clip playing (the swap is skipped).
pub fn poll_audio_hot_reload(
    watcher: &mut voltex_asset::FileWatcher,
    watched: &std::collections::HashMap<std::path::PathBuf, usize>,
    audio: &AudioSystem,
) -> Vec<(std::path::PathBuf, Result<(), String>)> {
    let mut outcomes = Vec::new();
    for changed in watcher.poll_changes() {
        let Some(&clip_index) = watched.get(&changed) else {
            continue;
        };
        let outcome = match voltex_audio::load_clip_file(&changed) {
            Ok(clip) => {
                audio.replace_clip(clip_index, clip);
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        };
        outcomes.push((changed, outcome));
    }
    outcomes
}

// ── Scene serialization ─────────────────────────────────────────────────────
//
// `AudioSource` lives in voltex_audio, which has no voltex_ecs dependency, so it
// cannot `impl voltex_ecs::Component` (orphan rule + the deliberate layer
// boundary). The runtime registers it through the documented escape hatch,
// `ComponentRegistry::register_raw`, with the free functions below. Call
// [`register_audio_source`] right after `register_defaults()`.

/// Stable component name for [`AudioSource`] in serialized scenes. Changing it
/// breaks compatibility with existing scene files.
pub const AUDIO_SOURCE_COMPONENT: &str = "audio_source";

/// `register_raw` payload layout: clip_id (u32 LE) + volume (f32 LE) + three
/// `bool` flags (spatial / looping / playing) = 11 bytes. The transient mixer
/// state (`played_once`, `active`) is intentionally not persisted — it is
/// re-derived at play time.
const AUDIO_SOURCE_BYTES: usize = 11;

/// `SerializeFn` for [`AudioSource`] (see [`register_audio_source`]).
pub fn serialize_audio_source(world: &World, entity: Entity) -> Option<Vec<u8>> {
    let s = world.get::<AudioSource>(entity)?;
    let mut out = Vec::with_capacity(AUDIO_SOURCE_BYTES);
    out.extend_from_slice(&s.clip_id.0.to_le_bytes());
    out.extend_from_slice(&s.volume.to_le_bytes());
    out.push(s.spatial as u8);
    out.push(s.looping as u8);
    out.push(s.playing as u8);
    Some(out)
}

/// `DeserializeFn` for [`AudioSource`] (see [`register_audio_source`]).
pub fn deserialize_audio_source(
    world: &mut World,
    entity: Entity,
    data: &[u8],
) -> voltex_ecs::Result<()> {
    let bytes: [u8; AUDIO_SOURCE_BYTES] =
        data.get(..AUDIO_SOURCE_BYTES).and_then(|s| s.try_into().ok()).ok_or("AudioSource too short")?;
    let clip = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let volume = f32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let mut src = AudioSource::new(clip);
    src.volume = volume;
    src.spatial = bytes[8] != 0;
    src.looping = bytes[9] != 0;
    src.playing = bytes[10] != 0;
    // played_once / active stay at their `new()` defaults (transient).
    world.add(entity, src);
    Ok(())
}

/// Register [`AudioSource`] scene serialization on `registry` via the
/// `register_raw` escape hatch. Idempotent-ish: a later registration of the
/// same name overrides this one.
pub fn register_audio_source(registry: &mut voltex_ecs::ComponentRegistry) {
    registry.register_raw(
        AUDIO_SOURCE_COMPONENT,
        serialize_audio_source,
        deserialize_audio_source,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_action_edges() {
        // playing rising edge → Play (spatial flag forwarded)
        assert_eq!(
            decide_action(true, false, false),
            AudioAction::Play { spatial: false }
        );
        assert_eq!(
            decide_action(true, false, true),
            AudioAction::Play { spatial: true }
        );
        // held playing (already active) → nothing (no re-trigger every frame)
        assert_eq!(decide_action(true, true, false), AudioAction::None);
        // stop edge
        assert_eq!(decide_action(false, true, false), AudioAction::Stop);
        // idle
        assert_eq!(decide_action(false, false, false), AudioAction::None);
    }

    #[test]
    fn poll_audio_hot_reload_swaps_changed_watched_files_only() {
        use std::time::Duration;
        let dir = std::env::temp_dir();
        let watched_path = dir.join(format!("voltex_audio_hr_{}.wav", std::process::id()));
        let unwatched_path = dir.join(format!("voltex_audio_hr_un_{}.wav", std::process::id()));
        let wav = voltex_audio::generate_wav_bytes(&[0.0, 0.1, -0.1], 44100);
        std::fs::write(&watched_path, &wav).unwrap();
        std::fs::write(&unwatched_path, &wav).unwrap();

        let audio = AudioSystem::new(vec![voltex_audio::AudioClip::new(
            vec![0.0; 8],
            44100,
            1,
        )])
        .expect("audio system");
        let mut watcher = voltex_asset::FileWatcher::new(Duration::from_millis(0));
        watcher.watch(watched_path.clone());
        watcher.watch(unwatched_path.clone());
        let watched: std::collections::HashMap<std::path::PathBuf, usize> =
            [(watched_path.clone(), 0usize)].into_iter().collect();

        // First poll establishes the mtime baseline — no outcomes.
        assert!(poll_audio_hot_reload(&mut watcher, &watched, &audio).is_empty());

        // Touch both files; only the mapped one produces an outcome.
        std::thread::sleep(Duration::from_millis(20));
        let wav2 = voltex_audio::generate_wav_bytes(&[0.5, -0.5], 22050);
        std::fs::write(&watched_path, &wav2).unwrap();
        std::fs::write(&unwatched_path, &wav2).unwrap();
        let outcomes = poll_audio_hot_reload(&mut watcher, &watched, &audio);
        assert_eq!(outcomes.len(), 1, "only the mapped path is reloaded");
        assert_eq!(outcomes[0].0, watched_path);
        assert!(outcomes[0].1.is_ok(), "valid wav swaps Ok: {:?}", outcomes[0].1);

        let _ = std::fs::remove_file(&watched_path);
        let _ = std::fs::remove_file(&unwatched_path);
    }

    #[test]
    fn poll_audio_hot_reload_reports_decode_failure_without_panicking() {
        use std::time::Duration;
        let path = std::env::temp_dir()
            .join(format!("voltex_audio_hr_bad_{}.wav", std::process::id()));
        std::fs::write(&path, voltex_audio::generate_wav_bytes(&[0.0], 44100)).unwrap();

        let audio = AudioSystem::new(Vec::new()).expect("audio system");
        let mut watcher = voltex_asset::FileWatcher::new(Duration::from_millis(0));
        watcher.watch(path.clone());
        let watched: std::collections::HashMap<std::path::PathBuf, usize> =
            [(path.clone(), 0usize)].into_iter().collect();
        let _ = poll_audio_hot_reload(&mut watcher, &watched, &audio); // baseline

        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&path, b"RIFFcorrupt").unwrap();
        let outcomes = poll_audio_hot_reload(&mut watcher, &watched, &audio);
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].1.is_err(), "corrupt file reports Err, old clip kept");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sync_sets_active_on_play_and_clears_on_stop() {
        // AudioSystem::new spawns a background thread but needs no device to
        // accept fire-and-forget commands, so this runs headless.
        let audio = AudioSystem::new(Vec::new()).expect("audio system");
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut src = AudioSource::new(0u32);
        src.playing = true;
        world.add(e, src);

        // First sync: rising edge → active becomes true.
        audio_sync_system(&mut world, &audio, None);
        assert!(world.get::<AudioSource>(e).unwrap().active, "play edge sets active");

        // Second sync with still-playing: no change, stays active (idempotent).
        audio_sync_system(&mut world, &audio, None);
        assert!(world.get::<AudioSource>(e).unwrap().active);

        // Toggle off → stop edge clears active.
        world.get_mut::<AudioSource>(e).unwrap().playing = false;
        audio_sync_system(&mut world, &audio, None);
        assert!(!world.get::<AudioSource>(e).unwrap().active, "stop edge clears active");
    }

    #[test]
    fn non_spatial_source_without_transform_is_handled() {
        let audio = AudioSystem::new(Vec::new()).expect("audio system");
        let mut world = World::new();
        let e = world.spawn();
        // No Transform — a non-spatial UI/music source.
        let mut src = AudioSource::new(3u32);
        src.playing = true;
        world.add(e, src);
        audio_sync_system(&mut world, &audio, None);
        assert!(
            world.get::<AudioSource>(e).unwrap().active,
            "non-spatial source without Transform should still start"
        );
    }

    #[test]
    fn audio_source_serialize_round_trip() {
        let mut world = World::new();
        let e = world.spawn();
        let mut src = AudioSource::new(7u32).spatial().looping();
        src.volume = 0.42;
        src.playing = true;
        src.played_once = true; // transient — must NOT survive
        src.active = true; //       transient — must NOT survive
        world.add(e, src);

        let bytes = serialize_audio_source(&world, e).expect("serialize");
        assert_eq!(bytes.len(), AUDIO_SOURCE_BYTES);

        let mut world2 = World::new();
        let e2 = world2.spawn();
        deserialize_audio_source(&mut world2, e2, &bytes).expect("deserialize");

        let got = world2.get::<AudioSource>(e2).expect("round-tripped source");
        assert_eq!(got.clip_id.0, 7);
        assert!((got.volume - 0.42).abs() < 1e-6);
        assert!(got.spatial);
        assert!(got.looping);
        assert!(got.playing);
        assert!(!got.played_once, "transient played_once must reset");
        assert!(!got.active, "transient active must reset");
    }

    #[test]
    fn audio_source_deserialize_rejects_truncated() {
        let mut world = World::new();
        let e = world.spawn();
        assert!(deserialize_audio_source(&mut world, e, &[0u8; 4]).is_err());
    }

    #[test]
    fn register_audio_source_round_trips_through_registry() {
        // End-to-end: registering via register_raw makes the component show up
        // in a JSON scene and load back.
        let mut registry = voltex_ecs::ComponentRegistry::new();
        registry.register_defaults();
        register_audio_source(&mut registry);

        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::default());
        world.add(e, AudioSource::new(5u32).looping());

        let json = voltex_ecs::serialize_scene_json(&world, &registry);
        assert!(json.contains(AUDIO_SOURCE_COMPONENT), "scene JSON must list the audio source");

        let mut world2 = World::new();
        voltex_ecs::deserialize_scene_json(&mut world2, &json, &registry).expect("load scene");
        let (_, src) = world2.query::<AudioSource>().next().expect("audio source survived round-trip");
        assert_eq!(src.clip_id.0, 5);
        assert!(src.looping);
    }
}
