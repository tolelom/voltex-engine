use voltex::prelude::Vec3;

pub struct WaveSystem {
    pub wave_number: u32,
    pub enemies_to_spawn: u32,
    pub spawn_timer: f32,
    pub wave_delay: f32,
    pub between_spawn_delay: f32,
    pub spawn_accumulator: f32,
}

impl WaveSystem {
    pub fn new() -> Self {
        Self {
            wave_number: 1,
            enemies_to_spawn: 3,
            spawn_timer: 2.0, // 2s before first wave
            wave_delay: 5.0,
            between_spawn_delay: 0.3, // stagger spawns
            spawn_accumulator: 0.0,
        }
    }

    /// Returns spawn positions for this frame (if any)
    pub fn update(&mut self, dt: f32, current_enemy_count: usize) -> Vec<Vec3> {
        let mut spawns = Vec::new();

        self.spawn_timer -= dt;

        if self.spawn_timer <= 0.0 && self.enemies_to_spawn > 0 {
            self.spawn_accumulator += dt;
            while self.spawn_accumulator >= self.between_spawn_delay && self.enemies_to_spawn > 0 {
                self.spawn_accumulator -= self.between_spawn_delay;
                self.enemies_to_spawn -= 1;
                spawns.push(random_edge_position(
                    self.wave_number * 100 + self.enemies_to_spawn + spawns.len() as u32,
                ));
            }
        }

        // All enemies spawned and killed -> next wave
        if self.enemies_to_spawn == 0 && current_enemy_count == 0 && self.spawn_timer <= 0.0 {
            self.wave_number += 1;
            self.enemies_to_spawn = 2 + self.wave_number;
            self.spawn_timer = self.wave_delay;
            self.spawn_accumulator = 0.0;
        }

        spawns
    }
}

fn random_edge_position(seed: u32) -> Vec3 {
    // Deterministic pseudo-random position on arena edge
    let hash = seed.wrapping_mul(2654435761);
    let side = hash % 4;
    let t = ((hash >> 8) % 100) as f32 / 100.0; // 0..1
    let pos = t * 18.0 - 9.0; // -9..9

    match side {
        0 => Vec3::new(pos, 0.5, -9.5),  // north edge
        1 => Vec3::new(pos, 0.5, 9.5),   // south edge
        2 => Vec3::new(-9.5, 0.5, pos),  // west edge
        _ => Vec3::new(9.5, 0.5, pos),   // east edge
    }
}
