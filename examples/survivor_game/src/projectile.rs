use voltex_math::Vec3;

pub struct Projectile {
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: f32,
}

impl Projectile {
    pub fn new(origin: Vec3, direction: Vec3, speed: f32) -> Self {
        Self {
            position: origin,
            velocity: direction * speed,
            lifetime: 2.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.position = self.position + self.velocity * dt;
        self.lifetime -= dt;
    }

    pub fn is_alive(&self) -> bool {
        self.lifetime > 0.0
    }
}
