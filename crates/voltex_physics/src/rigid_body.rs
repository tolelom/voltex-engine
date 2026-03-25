use voltex_math::Vec3;

pub const SLEEP_VELOCITY_THRESHOLD: f32 = 0.01;
pub const SLEEP_TIME_THRESHOLD: f32 = 0.5;

#[derive(Debug, Clone, Copy)]
pub struct RigidBody {
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
    pub restitution: f32,
    pub gravity_scale: f32,
    pub friction: f32, // Coulomb friction coefficient, default 0.5
    pub is_sleeping: bool,
    pub sleep_timer: f32,
}

impl RigidBody {
    pub fn dynamic(mass: f32) -> Self {
        Self {
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass,
            restitution: 0.3,
            gravity_scale: 1.0,
            friction: 0.5,
            is_sleeping: false,
            sleep_timer: 0.0,
        }
    }

    pub fn statik() -> Self {
        Self {
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass: 0.0,
            restitution: 0.3,
            gravity_scale: 0.0,
            friction: 0.5,
            is_sleeping: false,
            sleep_timer: 0.0,
        }
    }

    pub fn inv_mass(&self) -> f32 {
        if self.mass == 0.0 { 0.0 } else { 1.0 / self.mass }
    }

    pub fn is_static(&self) -> bool {
        self.mass == 0.0
    }

    /// Wake this body from sleep.
    pub fn wake(&mut self) {
        self.is_sleeping = false;
        self.sleep_timer = 0.0;
    }
}

pub struct PhysicsConfig {
    pub gravity: Vec3,
    pub fixed_dt: f32,
    pub solver_iterations: u32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            fixed_dt: 1.0 / 60.0,
            solver_iterations: 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_body() {
        let rb = RigidBody::dynamic(2.0);
        assert_eq!(rb.mass, 2.0);
        assert!(!rb.is_static());
        assert!((rb.inv_mass() - 0.5).abs() < 1e-6);
        assert_eq!(rb.velocity, Vec3::ZERO);
        assert_eq!(rb.restitution, 0.3);
        assert_eq!(rb.gravity_scale, 1.0);
        assert!(!rb.is_sleeping);
        assert_eq!(rb.sleep_timer, 0.0);
    }

    #[test]
    fn test_static_body() {
        let rb = RigidBody::statik();
        assert_eq!(rb.mass, 0.0);
        assert!(rb.is_static());
        assert_eq!(rb.inv_mass(), 0.0);
        assert_eq!(rb.gravity_scale, 0.0);
    }

    #[test]
    fn test_physics_config_default() {
        let cfg = PhysicsConfig::default();
        assert!((cfg.gravity.y - (-9.81)).abs() < 1e-6);
        assert!((cfg.fixed_dt - 1.0 / 60.0).abs() < 1e-6);
        assert_eq!(cfg.solver_iterations, 4);
    }

    #[test]
    fn test_wake() {
        let mut rb = RigidBody::dynamic(1.0);
        rb.is_sleeping = true;
        rb.sleep_timer = 1.0;
        rb.wake();
        assert!(!rb.is_sleeping);
        assert_eq!(rb.sleep_timer, 0.0);
    }
}
