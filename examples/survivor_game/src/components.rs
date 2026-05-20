//! Game components for survivor_game.
//!
//! Stored in `voltex_ecs::World`. Tag-style markers are zero-sized.
//! 직렬화는 사용하지 않으므로 `voltex_ecs::Component` 미구현.

use voltex::prelude::Vec3;

/// Marks the player entity. Singleton.
#[derive(Debug, Clone, Copy)]
pub struct PlayerTag;

/// Marks an enemy entity.
#[derive(Debug, Clone, Copy)]
pub struct EnemyTag;

/// Marks a projectile entity.
#[derive(Debug, Clone, Copy)]
pub struct ProjectileTag;

/// Linear velocity (world space). Projectiles use this directly;
/// player/enemy compute movement from input/AI each frame.
#[derive(Debug, Clone, Copy)]
pub struct Velocity(pub Vec3);

/// Move speed in units/sec (used by player input + enemy chase).
#[derive(Debug, Clone, Copy)]
pub struct MoveSpeed(pub f32);

/// Current and max hit points.
#[derive(Debug, Clone, Copy)]
pub struct Health {
    pub hp: i32,
    pub max_hp: i32,
}

/// Seconds until the entity can fire again. Decremented each frame.
#[derive(Debug, Clone, Copy)]
pub struct FireCooldown(pub f32);

/// Seconds remaining of damage immunity. Player only.
#[derive(Debug, Clone, Copy)]
pub struct InvincibleTimer(pub f32);

/// Seconds until the entity should despawn. Projectiles only.
#[derive(Debug, Clone, Copy)]
pub struct Lifetime(pub f32);
