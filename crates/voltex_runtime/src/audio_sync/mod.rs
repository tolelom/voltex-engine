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
use voltex_audio::{AudioSource, AudioSystem, VoiceId};
use voltex_ecs::{Entity, Transform, World};
use voltex_math::{Vec3, Vec4};

/// Tracks the mixer voice issued for each entity's `AudioSource`, so the sync
/// system can stop a voice whose entity died without a `playing = false` edge
/// (despawn / `world.clear()`) — otherwise looping voices leak in the mixer
/// forever (dogfood friction #9). Owned by the caller (`RuntimeApp`) because it
/// must outlive the entities it tracks.
#[derive(Debug, Default)]
pub struct AudioVoiceTracker {
    voices: std::collections::HashMap<Entity, VoiceId>,
}

impl AudioVoiceTracker {
    /// Number of currently tracked (entity, voice) pairs.
    pub fn len(&self) -> usize {
        self.voices.len()
    }

    /// `true` when no voices are tracked.
    pub fn is_empty(&self) -> bool {
        self.voices.is_empty()
    }

    /// Stop + untrack every tracked voice. Call when the world is replaced
    /// wholesale (`load_scene`) to silence the previous scene immediately,
    /// instead of waiting for the next sync's orphan sweep.
    pub fn stop_all(&mut self, audio: &AudioSystem) {
        for voice in self.voices.values() {
            audio.stop_voice(*voice);
        }
        self.voices.clear();
    }

    /// Stop + untrack every voice whose entity no longer exists or no longer
    /// carries an `AudioSource`. Since v1.0.120 `World::clear()` bumps every
    /// slot's generation, so entities from before a clear are always caught
    /// here too.
    fn sweep_orphans(&mut self, world: &World, audio: &AudioSystem) {
        self.voices.retain(|&entity, voice| {
            let alive = world.is_alive(entity) && world.get::<AudioSource>(entity).is_some();
            if !alive {
                audio.stop_voice(*voice);
            }
            alive
        });
    }
}

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
///
/// `voices` tracks the mixer voice issued per entity: play edges register,
/// stop edges release, and entities that disappeared since the last call
/// (despawn / component removal) get their orphan voice stopped automatically
/// — no manual `src.stop()` before despawn needed, including after
/// `world.clear()` (handles from before a clear are permanently stale).
/// `RuntimeApp::load_scene` additionally calls [`AudioVoiceTracker::stop_all`]
/// so the old scene goes silent at the swap rather than one sync later.
pub fn audio_sync_system(
    world: &mut World,
    audio: &AudioSystem,
    listener: Option<Entity>,
    voices: &mut AudioVoiceTracker,
) {
    // 0. Orphan sweep: stop voices whose entity vanished without a stop edge.
    voices.sweep_orphans(world, audio);

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
                let voice = if spatial {
                    let sp = SpatialParams::new(
                        plan.position,
                        DEFAULT_MIN_DISTANCE,
                        DEFAULT_MAX_DISTANCE,
                    );
                    audio.play_3d(plan.clip_index, plan.volume, plan.looping, sp)
                } else {
                    audio.play(plan.clip_index, plan.volume, plan.looping)
                };
                // 같은 엔티티 키를 덮어쓸 때 이전 voice를 반드시 stop —
                // one-shot 재트리거 등 같은 엔티티의 Play 엣지가 반복될 때의
                // 누수 상한선.
                if let Some(old) = voices.voices.insert(plan.entity, voice) {
                    audio.stop_voice(old);
                }
                if let Some(src) = world.get_mut::<AudioSource>(plan.entity) {
                    src.active = true;
                }
            }
            AudioAction::Stop => {
                // Stop exactly the voice we issued for this entity; fall back
                // to clip-wide stop for sources never tracked (defensive).
                match voices.voices.remove(&plan.entity) {
                    Some(voice) => audio.stop_voice(voice),
                    None => audio.stop(plan.clip_index),
                }
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
mod tests;
