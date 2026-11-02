//! [`Lifetime`] — a built-in "despawn after N seconds" component. Attach it to
//! any entity that should clean itself up: projectiles, timed pickups, spawned
//! effects, temporary enemies. [`update_lifetimes`] runs each runtime tick,
//! counting down and despawning entities whose time is up.

use voltex_ecs::{despawn_recursive, Component, Entity, World};

/// Time-to-live for an entity, in seconds. When `remaining` reaches `0`,
/// [`update_lifetimes`] despawns the entity (recursively, so children go too).
#[derive(Debug, Clone, Copy)]
pub struct Lifetime {
    /// Seconds left before the entity is despawned.
    pub remaining: f32,
}

impl Lifetime {
    /// A lifetime of `seconds`.
    pub fn new(seconds: f32) -> Self {
        Self { remaining: seconds }
    }
}

/// Scene serialization: `remaining` as 4-byte LE f32. Registered by
/// `RuntimeApp::new` — without it, saving a scene mid-play silently strips
/// the only despawn condition from in-flight one-shot entities (SFX,
/// particles with audio), leaving them immortal on load.
impl Component for Lifetime {
    const NAME: &'static str = "lifetime";

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let l = world.get::<Lifetime>(entity)?;
        Some(l.remaining.to_le_bytes().to_vec())
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> voltex_ecs::Result<()> {
        let bytes: [u8; 4] = data
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or("lifetime: expected 4 bytes")?;
        world.add(entity, Lifetime { remaining: f32::from_le_bytes(bytes) });
        Ok(())
    }
}

/// Decrement every [`Lifetime`] by `dt` and despawn (recursively) any entity
/// whose time has run out. No-op when there are no `Lifetime` entities.
pub fn update_lifetimes(world: &mut World, dt: f32) {
    let ents: Vec<Entity> = world.query::<Lifetime>().map(|(e, _)| e).collect();
    let mut expired = Vec::new();
    for e in ents {
        if let Some(l) = world.get_mut::<Lifetime>(e) {
            l.remaining -= dt;
            if l.remaining <= 0.0 {
                expired.push(e);
            }
        }
    }
    for e in expired {
        despawn_recursive(world, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifetime_serializes_through_the_registry_round_trip() {
        let mut registry = voltex_ecs::ComponentRegistry::new();
        registry.register::<Lifetime>();
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, voltex_ecs::Transform::new());
        world.add(e, Lifetime::new(1.5));
        let json = voltex_ecs::serialize_scene_json(&world, &registry);

        let mut world2 = World::new();
        voltex_ecs::deserialize_scene_json(&mut world2, &json, &registry).unwrap();
        let e2 = world2.query::<Lifetime>().map(|(e, _)| e).next().expect("lifetime restored");
        assert!((world2.get::<Lifetime>(e2).unwrap().remaining - 1.5).abs() < 1e-6);
    }

    #[test]
    fn lifetime_counts_down_then_despawns() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Lifetime::new(1.0));

        update_lifetimes(&mut world, 0.4);
        assert!(world.is_alive(e));
        assert!((world.get::<Lifetime>(e).unwrap().remaining - 0.6).abs() < 1e-5);

        update_lifetimes(&mut world, 0.7); // total 1.1 > 1.0
        assert!(!world.is_alive(e), "entity should despawn once its lifetime elapses");
    }

    #[test]
    fn no_lifetimes_is_noop() {
        let mut world = World::new();
        update_lifetimes(&mut world, 0.016); // must not panic
    }
}
