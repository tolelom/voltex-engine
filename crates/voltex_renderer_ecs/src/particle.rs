//! A lightweight CPU particle system built on [`Sprite`] entities. A particle
//! is any entity that carries three components together — a [`Particle`], a
//! [`Transform`](voltex_ecs::Transform), and a [`Sprite`]. [`update_particles`]
//! advances its motion, interpolates the sprite's size and alpha over its
//! lifetime, and despawns it when spent. The BasicRenderer's billboard pass
//! then draws each particle for free.
//!
//! Games spawn particles however they like (a burst on impact, a steady
//! trail) — [`spawn_particle`] is a one-call convenience that wires up all
//! three components.

use voltex_ecs::{Entity, Transform, World};
use voltex_math::Vec3;

use crate::sprite::Sprite;

/// Per-particle motion + lifetime state. Pair with a `Transform` (position)
/// and a `Sprite` (appearance); [`update_particles`] drives all three.
#[derive(Debug, Clone, Copy)]
pub struct Particle {
    /// World-space velocity (units/sec).
    pub velocity: Vec3,
    /// Constant acceleration (e.g. gravity). Units/sec².
    pub acceleration: Vec3,
    /// Seconds elapsed since spawn.
    pub age: f32,
    /// Total lifetime in seconds; the particle despawns once `age >= lifetime`.
    pub lifetime: f32,
    /// Sprite size at spawn (linearly interpolated toward `end_size`).
    pub start_size: f32,
    /// Sprite size at death.
    pub end_size: f32,
    /// Sprite alpha at spawn (linearly interpolated toward `end_alpha`).
    pub start_alpha: f32,
    /// Sprite alpha at death — set below `start_alpha` for a fade-out.
    pub end_alpha: f32,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            acceleration: Vec3::ZERO,
            age: 0.0,
            lifetime: 1.0,
            start_size: 0.5,
            end_size: 0.5,
            start_alpha: 1.0,
            end_alpha: 0.0,
        }
    }
}

impl Particle {
    /// A particle that flies along `velocity`, living `lifetime` seconds and
    /// fading from full to transparent (constant size). Tune the rest of the
    /// fields directly for gravity, shrink/grow, etc.
    pub fn new(velocity: Vec3, lifetime: f32, size: f32) -> Self {
        Self {
            velocity,
            lifetime,
            start_size: size,
            end_size: size,
            ..Default::default()
        }
    }

    /// Normalized life progress in `0.0..=1.0`.
    pub fn t(&self) -> f32 {
        if self.lifetime <= 0.0 {
            1.0
        } else {
            (self.age / self.lifetime).clamp(0.0, 1.0)
        }
    }
}

/// Spawn a fully-wired particle entity: `Transform` at `position`, a solid
/// `Sprite` of `color`, and the given `particle` state. Returns the entity.
pub fn spawn_particle(
    world: &mut World,
    position: Vec3,
    color: [f32; 4],
    particle: Particle,
) -> Entity {
    let e = world.spawn();
    world.add(e, Transform::from_position(position));
    world.add(e, Sprite::solid([particle.start_size, particle.start_size], color));
    world.add(e, particle);
    e
}

/// Advance every particle: integrate velocity/acceleration into its
/// `Transform`, age it, interpolate the `Sprite`'s size + alpha over its
/// lifetime, and despawn spent particles. No-op when there are no particles.
pub fn update_particles(world: &mut World, dt: f32) {
    // Collect first to avoid mutating while querying.
    let ents: Vec<Entity> = world.query::<Particle>().map(|(e, _)| e).collect();
    let mut dead = Vec::new();
    for e in ents {
        let Some(mut p) = world.get::<Particle>(e).copied() else {
            continue;
        };
        p.age += dt;
        p.velocity = p.velocity + p.acceleration * dt;
        if p.age >= p.lifetime {
            dead.push(e);
            continue;
        }
        let t = p.t();
        let size = p.start_size + (p.end_size - p.start_size) * t;
        let alpha = p.start_alpha + (p.end_alpha - p.start_alpha) * t;

        if let Some(tr) = world.get_mut::<Transform>(e) {
            tr.position = tr.position + p.velocity * dt;
        }
        if let Some(s) = world.get_mut::<Sprite>(e) {
            s.size = [size, size];
            s.color[3] = alpha;
        }
        // Write back the advanced particle state.
        if let Some(slot) = world.get_mut::<Particle>(e) {
            *slot = p;
        }
    }
    for e in dead {
        world.despawn(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_moves_and_fades_then_despawns() {
        let mut world = World::new();
        let mut p = Particle::new(Vec3::new(1.0, 0.0, 0.0), 1.0, 0.5);
        p.start_alpha = 1.0;
        p.end_alpha = 0.0;
        let e = spawn_particle(&mut world, Vec3::ZERO, [1.0, 1.0, 1.0, 1.0], p);

        // Halfway through life: moved ~0.5 along x, alpha ~0.5.
        update_particles(&mut world, 0.5);
        let tr = world.get::<Transform>(e).unwrap();
        assert!((tr.position.x - 0.5).abs() < 1e-5, "x={}", tr.position.x);
        let s = world.get::<Sprite>(e).unwrap();
        assert!((s.color[3] - 0.5).abs() < 0.05, "alpha={}", s.color[3]);
        assert!(world.is_alive(e));

        // Past end of life: despawned.
        update_particles(&mut world, 0.6);
        assert!(!world.is_alive(e), "particle should despawn after its lifetime");
    }

    #[test]
    fn acceleration_changes_velocity() {
        let mut world = World::new();
        let mut p = Particle::new(Vec3::ZERO, 10.0, 0.5);
        p.acceleration = Vec3::new(0.0, -10.0, 0.0); // gravity
        let e = spawn_particle(&mut world, Vec3::ZERO, [1.0; 4], p);

        update_particles(&mut world, 1.0);
        let vel = world.get::<Particle>(e).unwrap().velocity;
        assert!((vel.y + 10.0).abs() < 1e-4, "v.y after 1s of -10 gravity = {}", vel.y);
        // Fell downward.
        assert!(world.get::<Transform>(e).unwrap().position.y < 0.0);
    }

    #[test]
    fn no_particles_is_noop() {
        let mut world = World::new();
        update_particles(&mut world, 0.016); // must not panic
    }
}
