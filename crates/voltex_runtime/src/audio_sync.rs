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
}
