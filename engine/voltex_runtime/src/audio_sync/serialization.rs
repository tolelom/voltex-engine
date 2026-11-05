//! Scene (de)serialization for [`AudioSource`].
//!
//! `AudioSource` lives in voltex_audio, which has no voltex_ecs dependency, so it
//! cannot `impl voltex_ecs::Component` (orphan rule + the deliberate layer
//! boundary). The runtime registers it through the documented escape hatch,
//! `ComponentRegistry::register_raw`, with the free functions below. Call
//! [`register_audio_source`] right after `register_defaults()`.

use voltex_audio::AudioSource;
use voltex_ecs::{Entity, World};

/// Stable component name for [`AudioSource`] in serialized scenes. Changing it
/// breaks compatibility with existing scene files.
pub const AUDIO_SOURCE_COMPONENT: &str = "audio_source";

/// `register_raw` payload layout: clip_id (u32 LE) + volume (f32 LE) + three
/// `bool` flags (spatial / looping / playing) = 11 bytes. The transient mixer
/// state (`played_once`, `active`) is intentionally not persisted — it is
/// re-derived at play time.
pub(super) const AUDIO_SOURCE_BYTES: usize = 11;

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
