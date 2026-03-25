use voltex_math::Vec3;
use voltex_platform::InputState;
use winit::keyboard::KeyCode;

pub struct Player {
    pub position: Vec3,
    pub speed: f32,
    pub hp: i32,
    pub max_hp: i32,
    pub fire_cooldown: f32,
    pub invincible_timer: f32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            speed: 8.0,
            hp: 3,
            max_hp: 3,
            fire_cooldown: 0.0,
            invincible_timer: 0.0,
        }
    }

    pub fn update(&mut self, input: &InputState, dt: f32) {
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

        self.position = self.position + dir * self.speed * dt;

        // Clamp to arena bounds
        self.position.x = self.position.x.clamp(-9.5, 9.5);
        self.position.z = self.position.z.clamp(-9.5, 9.5);
    }
}
