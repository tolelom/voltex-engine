//! ECS systems for survivor_game.
//!
//! Each system reads the World + frame inputs (dt, InputState, player_pos)
//! and mutates the World. Collision uses voltex_physics::detect_collisions.

use std::collections::HashSet;
use voltex::ecs::Entity;
use voltex::platform::InputState;
use voltex::prelude::{Transform, Vec3, World};
use voltex_physics::{detect_collisions, Collider};
use winit::keyboard::KeyCode;

use crate::components::{
    EnemyTag, FireCooldown, Health, InvincibleTimer, Lifetime, MoveSpeed, PlayerTag,
    ProjectileTag, Velocity,
};

const ARENA_BOUND: f32 = 9.5;

/// Moves the player by WASD input. Decrements FireCooldown and InvincibleTimer.
pub fn player_input_system(world: &mut World, input: &InputState, dt: f32) {
    // Find the player entity (single query result expected).
    let player: Option<(Entity, MoveSpeed)> = world
        .query2::<PlayerTag, MoveSpeed>()
        .into_iter()
        .map(|(e, _, s)| (e, *s))
        .next();

    let (entity, speed) = match player {
        Some(p) => p,
        None => return,
    };

    let mut dir = Vec3::ZERO;
    if input.is_key_pressed(KeyCode::KeyW) {
        dir.z -= 1.0;
    }
    if input.is_key_pressed(KeyCode::KeyS) {
        dir.z += 1.0;
    }
    if input.is_key_pressed(KeyCode::KeyA) {
        dir.x -= 1.0;
    }
    if input.is_key_pressed(KeyCode::KeyD) {
        dir.x += 1.0;
    }
    if dir.length_squared() > 0.0 {
        dir = dir.normalize();
    }

    if let Some(t) = world.get_mut::<Transform>(entity) {
        t.position = t.position + dir * speed.0 * dt;
        t.position.x = t.position.x.clamp(-ARENA_BOUND, ARENA_BOUND);
        t.position.z = t.position.z.clamp(-ARENA_BOUND, ARENA_BOUND);
    }

    if let Some(c) = world.get_mut::<FireCooldown>(entity) {
        c.0 = (c.0 - dt).max(0.0);
    }
    if let Some(iv) = world.get_mut::<InvincibleTimer>(entity) {
        iv.0 = (iv.0 - dt).max(0.0);
    }
}

/// Each enemy chases the player on the XZ plane.
pub fn enemy_chase_system(world: &mut World, player_pos: Vec3, dt: f32) {
    let updates: Vec<(Entity, Vec3)> = world
        .query2::<EnemyTag, MoveSpeed>()
        .into_iter()
        .filter_map(|(e, _, speed)| {
            let t = world.get::<Transform>(e)?;
            let dir = Vec3::new(player_pos.x - t.position.x, 0.0, player_pos.z - t.position.z);
            if dir.length_squared() < 0.01 {
                return None;
            }
            Some((e, dir.normalize() * speed.0 * dt))
        })
        .collect();

    for (e, delta) in updates {
        if let Some(t) = world.get_mut::<Transform>(e) {
            t.position = t.position + delta;
        }
    }
}

/// Moves projectiles by their velocity.
pub fn projectile_motion_system(world: &mut World, dt: f32) {
    let updates: Vec<(Entity, Vec3)> = world
        .query2::<ProjectileTag, Velocity>()
        .into_iter()
        .map(|(e, _, v)| (e, v.0 * dt))
        .collect();

    for (e, delta) in updates {
        if let Some(t) = world.get_mut::<Transform>(e) {
            t.position = t.position + delta;
        }
    }
}

/// Decrements Lifetime; returns entities that expired this frame.
pub fn lifetime_system(world: &mut World, dt: f32) -> Vec<Entity> {
    let entities: Vec<Entity> = world.query::<Lifetime>().map(|(e, _)| e).collect();
    let mut expired = Vec::new();
    for e in entities {
        if let Some(l) = world.get_mut::<Lifetime>(e) {
            l.0 -= dt;
            if l.0 <= 0.0 {
                expired.push(e);
            }
        }
    }
    expired
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CollisionEvents {
    pub enemies_killed: u32,
    pub player_hit: bool,
}

/// Runs voltex_physics broadphase + narrowphase and applies game effects:
/// projectile×enemy → both despawn, score; enemy×player → HP--, invincible.
pub fn collision_system(world: &mut World) -> CollisionEvents {
    let contacts = detect_collisions(world);

    let mut to_despawn: HashSet<Entity> = HashSet::new();
    let mut events = CollisionEvents::default();

    for c in &contacts {
        let a_is_proj = world.get::<ProjectileTag>(c.entity_a).is_some();
        let b_is_proj = world.get::<ProjectileTag>(c.entity_b).is_some();
        let a_is_enemy = world.get::<EnemyTag>(c.entity_a).is_some();
        let b_is_enemy = world.get::<EnemyTag>(c.entity_b).is_some();
        let a_is_player = world.get::<PlayerTag>(c.entity_a).is_some();
        let b_is_player = world.get::<PlayerTag>(c.entity_b).is_some();

        // Projectile vs enemy: both die.
        if (a_is_proj && b_is_enemy) || (b_is_proj && a_is_enemy) {
            // Skip if either side is already queued (one bullet kills one enemy).
            if to_despawn.contains(&c.entity_a) || to_despawn.contains(&c.entity_b) {
                continue;
            }
            to_despawn.insert(c.entity_a);
            to_despawn.insert(c.entity_b);
            events.enemies_killed += 1;
            continue;
        }

        // Enemy vs player: damage if not invincible.
        let (player_e, _enemy_e) = if a_is_player && b_is_enemy {
            (c.entity_a, c.entity_b)
        } else if b_is_player && a_is_enemy {
            (c.entity_b, c.entity_a)
        } else {
            continue;
        };

        let invincible = world
            .get::<InvincibleTimer>(player_e)
            .map(|t| t.0 > 0.0)
            .unwrap_or(false);
        if invincible || events.player_hit {
            continue;
        }
        if let Some(h) = world.get_mut::<Health>(player_e) {
            h.hp -= 1;
        }
        if let Some(t) = world.get_mut::<InvincibleTimer>(player_e) {
            t.0 = 1.0;
        }
        events.player_hit = true;
    }

    for e in to_despawn {
        world.despawn(e);
    }

    events
}

// ── Spawn helpers ───────────────────────────────────────────────────

pub fn spawn_player(world: &mut World) -> Entity {
    let e = world.spawn();
    world.add(e, PlayerTag);
    world.add(e, Transform::from_position(Vec3::ZERO));
    world.add(e, MoveSpeed(8.0));
    world.add(e, Health { hp: 3, max_hp: 3 });
    world.add(e, FireCooldown(0.0));
    world.add(e, InvincibleTimer(0.0));
    world.add(e, Collider::Sphere { radius: 0.5 });
    e
}

pub fn spawn_enemy(world: &mut World, position: Vec3) -> Entity {
    let e = world.spawn();
    world.add(e, EnemyTag);
    world.add(e, Transform::from_position(position));
    world.add(e, MoveSpeed(3.0));
    world.add(e, Collider::Sphere { radius: 0.4 });
    e
}

pub fn spawn_projectile(world: &mut World, origin: Vec3, direction: Vec3, speed: f32) -> Entity {
    let e = world.spawn();
    world.add(e, ProjectileTag);
    world.add(e, Transform::from_position(origin));
    world.add(e, Velocity(direction * speed));
    world.add(e, Lifetime(2.0));
    world.add(e, Collider::Sphere { radius: 0.15 });
    e
}
