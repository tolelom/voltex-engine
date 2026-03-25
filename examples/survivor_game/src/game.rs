use voltex_math::Vec3;
use voltex_platform::InputState;
use crate::player::Player;
use crate::projectile::Projectile;
use crate::enemy::Enemy;
use crate::wave::WaveSystem;

pub struct FrameEvents {
    pub shot_fired: bool,
    pub enemies_killed: u32,
    pub player_hit: bool,
}

pub struct GameState {
    pub player: Player,
    pub projectiles: Vec<Projectile>,
    pub enemies: Vec<Enemy>,
    pub wave: WaveSystem,
    pub score: u32,
    pub game_over: bool,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            player: Player::new(),
            projectiles: Vec::new(),
            enemies: Vec::new(),
            wave: WaveSystem::new(),
            score: 0,
            game_over: false,
        }
    }

    pub fn try_shoot(&mut self, aim_dir: Vec3) -> bool {
        if self.player.fire_cooldown <= 0.0 {
            let origin = self.player.position + Vec3::new(0.0, 0.5, 0.0);
            self.projectiles.push(Projectile::new(origin, aim_dir, 20.0));
            self.player.fire_cooldown = 0.2;
            true
        } else {
            false
        }
    }

    pub fn update(&mut self, input: &InputState, dt: f32) -> FrameEvents {
        let mut events = FrameEvents {
            shot_fired: false,
            enemies_killed: 0,
            player_hit: false,
        };

        if self.game_over {
            return events;
        }

        self.player.update(input, dt);
        self.player.fire_cooldown = (self.player.fire_cooldown - dt).max(0.0);

        // Update projectiles and remove dead ones
        for proj in &mut self.projectiles {
            proj.update(dt);
        }
        self.projectiles.retain(|p| p.is_alive());

        // Wave system spawns
        let alive_count = self.enemies.iter().filter(|e| e.alive).count();
        let spawns = self.wave.update(dt, alive_count);
        for pos in spawns {
            self.enemies.push(Enemy::new(pos));
        }

        // Update enemies (chase player)
        let player_pos = self.player.position;
        for enemy in &mut self.enemies {
            if enemy.alive {
                enemy.update(player_pos, dt);
            }
        }

        // Remove dead enemies
        self.enemies.retain(|e| e.alive);

        // Collision: projectile vs enemy
        let mut killed_enemies = Vec::new();
        let mut killed_projectiles = Vec::new();

        for (pi, proj) in self.projectiles.iter().enumerate() {
            for (ei, enemy) in self.enemies.iter().enumerate() {
                let dist = (proj.position - enemy.position).length();
                if dist < 0.55 {
                    // projectile radius 0.15 + enemy radius 0.4
                    killed_enemies.push(ei);
                    killed_projectiles.push(pi);
                    self.score += 100;
                    events.enemies_killed += 1;
                }
            }
        }

        // Remove killed (reverse order to preserve indices)
        killed_enemies.sort();
        killed_enemies.dedup();
        for &i in killed_enemies.iter().rev() {
            self.enemies.remove(i);
        }
        killed_projectiles.sort();
        killed_projectiles.dedup();
        for &i in killed_projectiles.iter().rev() {
            self.projectiles.remove(i);
        }

        // Collision: enemy vs player
        if self.player.invincible_timer <= 0.0 {
            for enemy in self.enemies.iter() {
                let dist = (enemy.position - self.player.position).length();
                if dist < 0.9 {
                    // player radius 0.5 + enemy radius 0.4
                    self.player.hp -= 1;
                    self.player.invincible_timer = 1.0;
                    events.player_hit = true;
                    if self.player.hp <= 0 {
                        self.game_over = true;
                    }
                    break;
                }
            }
        }

        // Update invincible timer
        self.player.invincible_timer = (self.player.invincible_timer - dt).max(0.0);

        events
    }
}
