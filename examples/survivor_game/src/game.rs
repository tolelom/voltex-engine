//! ECS-based game state. Owns the World and the wave spawner.
//!
//! The renderer reads from this through `*_positions()` / `player_*()`
//! query helpers so main.rs never touches the World directly.

use voltex::ecs::Entity;
use voltex::platform::InputState;
use voltex::prelude::{Transform, Vec3, World};

use crate::components::{EnemyTag, FireCooldown, Health, InvincibleTimer, ProjectileTag};
use crate::systems::{
    collision_system, enemy_chase_system, lifetime_system, player_input_system,
    projectile_motion_system, spawn_enemy, spawn_player, spawn_projectile, CollisionEvents,
};
use crate::wave::WaveSystem;

const FIRE_COOLDOWN_SEC: f32 = 0.2;
const PROJECTILE_SPEED: f32 = 20.0;
const PROJECTILE_ORIGIN_OFFSET: Vec3 = Vec3 {
    x: 0.0,
    y: 0.5,
    z: 0.0,
};

#[derive(Default)]
pub struct FrameEvents {
    pub enemies_killed: u32,
    pub player_hit: bool,
}

pub struct Game {
    pub world: World,
    pub player_entity: Entity,
    pub wave: WaveSystem,
    pub score: u32,
    pub game_over: bool,
}

impl Game {
    pub fn new() -> Self {
        let mut world = World::new();
        let player_entity = spawn_player(&mut world);
        Self {
            world,
            player_entity,
            wave: WaveSystem::new(),
            score: 0,
            game_over: false,
        }
    }

    pub fn restart(&mut self) {
        *self = Self::new();
    }

    pub fn update(&mut self, input: &InputState, dt: f32) -> FrameEvents {
        let mut events = FrameEvents::default();
        if self.game_over {
            return events;
        }

        // Player + projectile motion.
        player_input_system(&mut self.world, input, dt);
        projectile_motion_system(&mut self.world, dt);

        // Enemy chase needs the latest player position.
        let player_pos = self.player_pos();
        enemy_chase_system(&mut self.world, player_pos, dt);

        // Wave spawning.
        let alive = self.alive_enemy_count();
        for pos in self.wave.update(dt, alive) {
            spawn_enemy(&mut self.world, pos);
        }

        // Despawn expired projectiles before collision so dead bullets don't hit.
        for e in lifetime_system(&mut self.world, dt) {
            self.world.despawn(e);
        }

        // Collisions: projectile×enemy + enemy×player.
        let CollisionEvents {
            enemies_killed,
            player_hit,
        } = collision_system(&mut self.world);
        self.score += enemies_killed * 100;
        events.enemies_killed = enemies_killed;
        events.player_hit = player_hit;

        if self.player_health().0 <= 0 {
            self.game_over = true;
        }

        events
    }

    /// Spawns a projectile if FireCooldown allows. Returns true when fired.
    pub fn try_shoot(&mut self, aim_dir: Vec3) -> bool {
        let ready = self
            .world
            .get::<FireCooldown>(self.player_entity)
            .map(|c| c.0 <= 0.0)
            .unwrap_or(false);
        if !ready {
            return false;
        }
        let origin = self.player_pos() + PROJECTILE_ORIGIN_OFFSET;
        spawn_projectile(&mut self.world, origin, aim_dir, PROJECTILE_SPEED);
        if let Some(c) = self.world.get_mut::<FireCooldown>(self.player_entity) {
            c.0 = FIRE_COOLDOWN_SEC;
        }
        true
    }

    // ── Query helpers (renderer / HUD consumers) ────────────────────

    pub fn player_pos(&self) -> Vec3 {
        self.world
            .get::<Transform>(self.player_entity)
            .map(|t| t.position)
            .unwrap_or(Vec3::ZERO)
    }

    /// (current_hp, max_hp).
    pub fn player_health(&self) -> (i32, i32) {
        self.world
            .get::<Health>(self.player_entity)
            .map(|h| (h.hp, h.max_hp))
            .unwrap_or((0, 0))
    }

    /// Seconds remaining on invincibility, or 0.0 when not active.
    pub fn player_invincible(&self) -> f32 {
        self.world
            .get::<InvincibleTimer>(self.player_entity)
            .map(|t| t.0)
            .unwrap_or(0.0)
    }

    pub fn alive_enemy_count(&self) -> usize {
        self.world.query::<EnemyTag>().count()
    }

    pub fn projectile_positions(&self) -> Vec<Vec3> {
        self.world
            .query::<ProjectileTag>()
            .filter_map(|(e, _)| self.world.get::<Transform>(e).map(|t| t.position))
            .collect()
    }

    pub fn enemy_positions(&self) -> Vec<Vec3> {
        self.world
            .query::<EnemyTag>()
            .filter_map(|(e, _)| self.world.get::<Transform>(e).map(|t| t.position))
            .collect()
    }
}
