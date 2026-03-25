use voltex_math::Vec3;

pub struct Enemy {
    pub position: Vec3,
    pub speed: f32,
    pub alive: bool,
}

impl Enemy {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            speed: 3.0,
            alive: true,
        }
    }

    pub fn update(&mut self, player_pos: Vec3, dt: f32) {
        let dir = player_pos - self.position;
        let dir_xz = Vec3::new(dir.x, 0.0, dir.z);
        if dir_xz.length_squared() > 0.01 {
            let move_dir = dir_xz.normalize();
            self.position = self.position + move_dir * self.speed * dt;
        }
    }
}
