use voltex_math::Vec3;
use voltex_platform::InputState;
use crate::player::Player;
use crate::projectile::Projectile;
use crate::enemy::Enemy;
use crate::wave::WaveSystem;

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

    pub fn try_shoot(&mut self, aim_dir: Vec3) {
        if self.player.fire_cooldown <= 0.0 {
            let origin = self.player.position + Vec3::new(0.0, 0.5, 0.0);
            self.projectiles.push(Projectile::new(origin, aim_dir, 20.0));
            self.player.fire_cooldown = 0.2;
        }
    }

    pub fn update(&mut self, input: &InputState, dt: f32) {
        if self.game_over {
            return;
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
    }
}
