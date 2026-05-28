//! Rigid body component + per-world physics configuration.

use voltex_math::Vec3;

/// Bodies whose linear+angular velocity stay under this threshold for
/// `SLEEP_TIME_THRESHOLD` seconds are put to sleep.
pub(crate) const SLEEP_VELOCITY_THRESHOLD: f32 = 0.01;
/// Seconds a body must remain "quiet" before it is put to sleep.
pub(crate) const SLEEP_TIME_THRESHOLD: f32 = 0.5;

/// Rigid body state attached to an entity by the physics solver.
///
/// Set `mass = 0` for kinematic / static bodies (see [`RigidBody::statik`]).
#[derive(Debug, Clone, Copy)]
pub struct RigidBody {
    /// Linear velocity in world space.
    pub velocity: Vec3,
    /// Angular velocity in world space.
    pub angular_velocity: Vec3,
    /// Body mass in kg. Zero means immovable (infinite mass).
    pub mass: f32,
    /// Coefficient of restitution in `[0.0, 1.0]` (0 = no bounce).
    pub restitution: f32,
    /// Per-body multiplier applied to `PhysicsConfig::gravity`.
    pub gravity_scale: f32,
    /// Coulomb friction coefficient (default 0.5).
    pub friction: f32,
    /// `true` while the body is asleep (skipped by the integrator).
    pub is_sleeping: bool,
    /// Seconds the body has been "quiet" (drives the sleep transition).
    pub sleep_timer: f32,
}

impl RigidBody {
    /// Creates a dynamic body with the given mass and engine defaults.
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

    /// Creates an immovable body (mass 0, gravity scale 0). Useful for
    /// terrain, level geometry, kinematic platforms.
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

    /// `1.0 / mass`, or `0.0` for static bodies (mass == 0).
    pub fn inv_mass(&self) -> f32 {
        if self.mass == 0.0 { 0.0 } else { 1.0 / self.mass }
    }

    /// `true` if this body is treated as immovable (mass == 0).
    pub fn is_static(&self) -> bool {
        self.mass == 0.0
    }

    /// Wake this body from sleep.
    pub fn wake(&mut self) {
        self.is_sleeping = false;
        self.sleep_timer = 0.0;
    }
}

/// Per-world physics tuning knobs.
pub struct PhysicsConfig {
    /// Global acceleration applied each step (default `(0, -9.81, 0)`).
    pub gravity: Vec3,
    /// Fixed simulation timestep in seconds (default `1/60`).
    pub fixed_dt: f32,
    /// Sequential-impulse solver iteration count.
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
