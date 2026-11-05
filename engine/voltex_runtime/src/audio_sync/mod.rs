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
//!
//! Clip hot reload lives in [`hot_reload`], `AudioSource` scene persistence in
//! [`serialization`]; both are re-exported here.

use voltex_audio::spatial::SpatialParams;
use voltex_audio::{AudioSource, AudioSystem, VoiceId};
use voltex_ecs::{Entity, Transform, World};
use voltex_math::{Vec3, Vec4};

mod hot_reload;
mod serialization;

pub use hot_reload::poll_audio_hot_reload;
pub use serialization::{
    deserialize_audio_source, register_audio_source, serialize_audio_source,
    AUDIO_SOURCE_COMPONENT,
};
#[cfg(test)]
use serialization::AUDIO_SOURCE_BYTES;

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

#[cfg(test)]
mod tests;
