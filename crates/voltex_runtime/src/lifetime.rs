//! [`Lifetime`] — a built-in "despawn after N seconds" component. Attach it to
//! any entity that should clean itself up: projectiles, timed pickups, spawned
//! effects, temporary enemies. [`update_lifetimes`] runs each runtime tick,
//! counting down and despawning entities whose time is up.

use voltex_ecs::{despawn_recursive, Entity, World};

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
