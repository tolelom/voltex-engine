use voltex_ecs::World;
use voltex_ecs::Transform;
use voltex_math::Vec3;
use crate::collider::Collider;
use crate::rigid_body::{RigidBody, PhysicsConfig, SLEEP_VELOCITY_THRESHOLD, SLEEP_TIME_THRESHOLD};

/// Compute diagonal inertia tensor for a collider shape.
/// Returns Vec3 where each component is the moment of inertia about that axis.
pub fn inertia_tensor(collider: &Collider, mass: f32) -> Vec3 {
    match collider {
        Collider::Sphere { radius } => {
            let i = (2.0 / 5.0) * mass * radius * radius;
            Vec3::new(i, i, i)
        }
        Collider::Box { half_extents } => {
            let w = half_extents.x * 2.0;
            let h = half_extents.y * 2.0;
            let d = half_extents.z * 2.0;
            let factor = mass / 12.0;
            Vec3::new(
                factor * (h * h + d * d),
                factor * (w * w + d * d),
                factor * (w * w + h * h),
            )
        }
        Collider::Capsule { radius, half_height } => {
            // Approximate as cylinder with total height = 2*half_height + 2*radius
            let r = *radius;
            let h = half_height * 2.0;
            let ix = mass * (3.0 * r * r + h * h) / 12.0;
            let iy = mass * r * r / 2.0;
            let iz = ix;
            Vec3::new(ix, iy, iz)
        }
        Collider::ConvexHull(hull) => {
            // Approximate using AABB of the hull vertices
            let mut min = hull.vertices[0];
            let mut max = min;
            for &v in &hull.vertices[1..] {
                min = Vec3::new(min.x.min(v.x), min.y.min(v.y), min.z.min(v.z));
                max = Vec3::new(max.x.max(v.x), max.y.max(v.y), max.z.max(v.z));
            }
            let size = max - min;
            let w = size.x;
            let h = size.y;
            let d = size.z;
            let factor = mass / 12.0;
            Vec3::new(
                factor * (h * h + d * d),
                factor * (w * w + d * d),
                factor * (w * w + h * h),
            )
        }
    }
}

/// Compute inverse inertia (component-wise 1/I). Returns zero for zero-mass or zero-inertia.
pub fn inv_inertia(inertia: Vec3) -> Vec3 {
    Vec3::new(
        if inertia.x > 1e-10 { 1.0 / inertia.x } else { 0.0 },
        if inertia.y > 1e-10 { 1.0 / inertia.y } else { 0.0 },
        if inertia.z > 1e-10 { 1.0 / inertia.z } else { 0.0 },
    )
}

pub fn integrate(world: &mut World, config: &PhysicsConfig) {
    // 1. Collect linear + angular updates
    let updates: Vec<(voltex_ecs::Entity, Vec3, Vec3, Vec3)> = world
        .query2::<Transform, RigidBody>()
        .into_iter()
        .filter(|(_, _, rb)| !rb.is_static() && !rb.is_sleeping)
        .map(|(entity, transform, rb)| {
            let new_velocity = rb.velocity + config.gravity * rb.gravity_scale * config.fixed_dt;
            let new_position = transform.position + new_velocity * config.fixed_dt;
            let new_rotation = transform.rotation + rb.angular_velocity * config.fixed_dt;
            (entity, new_velocity, new_position, new_rotation)
        })
        .collect();

    // 2. Apply
    for (entity, new_velocity, new_position, new_rotation) in updates {
        if let Some(rb) = world.get_mut::<RigidBody>(entity) {
            rb.velocity = new_velocity;
        }
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = new_position;
            t.rotation = new_rotation;
        }
    }

    // 3. Update sleep timers
    update_sleep_timers(world, config);
}

fn update_sleep_timers(world: &mut World, config: &PhysicsConfig) {
    let sleep_updates: Vec<(voltex_ecs::Entity, bool, f32)> = world
        .query::<RigidBody>()
        .filter(|(_, rb)| !rb.is_static())
        .map(|(entity, rb)| {
            let speed = rb.velocity.length() + rb.angular_velocity.length();
            if speed < SLEEP_VELOCITY_THRESHOLD {
                let new_timer = rb.sleep_timer + config.fixed_dt;
                let should_sleep = new_timer >= SLEEP_TIME_THRESHOLD;
                (entity, should_sleep, new_timer)
            } else {
                (entity, false, 0.0)
            }
        })
        .collect();

    for (entity, should_sleep, timer) in sleep_updates {
        if let Some(rb) = world.get_mut::<RigidBody>(entity) {
            rb.sleep_timer = timer;
            if should_sleep {
                rb.is_sleeping = true;
                rb.velocity = Vec3::ZERO;
                rb.angular_velocity = Vec3::ZERO;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::World;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;
    use crate::{RigidBody, Collider};

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn test_gravity_fall() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(0.0, 10.0, 0.0)));
        world.add(e, RigidBody::dynamic(1.0));

        let config = PhysicsConfig::default();
        integrate(&mut world, &config);

        let rb = world.get::<RigidBody>(e).unwrap();
        let t = world.get::<Transform>(e).unwrap();

        let expected_vy = -9.81 * config.fixed_dt;
        assert!(approx(rb.velocity.y, expected_vy));

        let expected_py = 10.0 + expected_vy * config.fixed_dt;
        assert!(approx(t.position.y, expected_py));
    }

    #[test]
    fn test_static_unchanged() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(0.0, 5.0, 0.0)));
        world.add(e, RigidBody::statik());

        let config = PhysicsConfig::default();
        integrate(&mut world, &config);

        let t = world.get::<Transform>(e).unwrap();
        assert!(approx(t.position.y, 5.0));

        let rb = world.get::<RigidBody>(e).unwrap();
        assert!(approx(rb.velocity.y, 0.0));
    }

    #[test]
    fn test_initial_velocity() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut rb = RigidBody::dynamic(1.0);
        rb.velocity = Vec3::new(5.0, 0.0, 0.0);
        rb.gravity_scale = 0.0;
        world.add(e, rb);

        let config = PhysicsConfig::default();
        integrate(&mut world, &config);

        let t = world.get::<Transform>(e).unwrap();
        let expected_x = 5.0 * config.fixed_dt;
        assert!(approx(t.position.x, expected_x));
    }

    // --- Inertia tensor tests ---

    #[test]
    fn test_inertia_tensor_sphere() {
        let c = Collider::Sphere { radius: 1.0 };
        let i = inertia_tensor(&c, 1.0);
        let expected = 2.0 / 5.0;
        assert!(approx(i.x, expected));
        assert!(approx(i.y, expected));
        assert!(approx(i.z, expected));
    }

    #[test]
    fn test_inertia_tensor_box() {
        let c = Collider::Box { half_extents: Vec3::ONE };
        let i = inertia_tensor(&c, 12.0);
        // w=2, h=2, d=2, factor=1 => ix = 4+4=8, etc
        assert!(approx(i.x, 8.0));
        assert!(approx(i.y, 8.0));
        assert!(approx(i.z, 8.0));
    }

    #[test]
    fn test_inertia_tensor_capsule() {
        let c = Collider::Capsule { radius: 0.5, half_height: 1.0 };
        let i = inertia_tensor(&c, 1.0);
        // Approximate as cylinder: r=0.5, h=2.0
        // ix = m*(3*r^2 + h^2)/12 = (3*0.25 + 4)/12 = 4.75/12
        assert!(approx(i.x, 4.75 / 12.0));
        // iy = m*r^2/2 = 0.25/2 = 0.125
        assert!(approx(i.y, 0.125));
    }

    // --- Angular velocity integration tests ---

    #[test]
    fn test_spinning_sphere() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut rb = RigidBody::dynamic(1.0);
        rb.angular_velocity = Vec3::new(0.0, 3.14159, 0.0); // ~PI rad/s around Y
        rb.gravity_scale = 0.0;
        world.add(e, rb);

        let config = PhysicsConfig::default();
        integrate(&mut world, &config);

        let t = world.get::<Transform>(e).unwrap();
        let expected_rot_y = 3.14159 * config.fixed_dt;
        assert!(approx(t.rotation.y, expected_rot_y));
        // Position should not change (no linear velocity, no gravity)
        assert!(approx(t.position.x, 0.0));
        assert!(approx(t.position.y, 0.0));
    }

    #[test]
    fn test_angular_velocity_persists() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut rb = RigidBody::dynamic(1.0);
        rb.angular_velocity = Vec3::new(1.0, 0.0, 0.0);
        rb.gravity_scale = 0.0;
        world.add(e, rb);

        let config = PhysicsConfig::default();
        integrate(&mut world, &config);
        integrate(&mut world, &config);

        let t = world.get::<Transform>(e).unwrap();
        let expected_rot_x = 2.0 * config.fixed_dt;
        assert!(approx(t.rotation.x, expected_rot_x));
    }

    // --- Sleep system tests ---

    #[test]
    fn test_body_sleeps_after_resting() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut rb = RigidBody::dynamic(1.0);
        rb.velocity = Vec3::ZERO;
        rb.gravity_scale = 0.0;
        world.add(e, rb);

        let config = PhysicsConfig {
            gravity: Vec3::ZERO,
            fixed_dt: 1.0 / 60.0,
            solver_iterations: 4,
        };

        // Integrate many times until sleep timer exceeds threshold
        for _ in 0..60 {
            integrate(&mut world, &config);
        }

        let rb = world.get::<RigidBody>(e).unwrap();
        assert!(rb.is_sleeping, "body should be sleeping after resting");
    }

    #[test]
    fn test_sleeping_body_not_integrated() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(0.0, 10.0, 0.0)));
        let mut rb = RigidBody::dynamic(1.0);
        rb.is_sleeping = true;
        world.add(e, rb);

        let config = PhysicsConfig::default();
        integrate(&mut world, &config);

        let t = world.get::<Transform>(e).unwrap();
        assert!(approx(t.position.y, 10.0), "sleeping body should not move");
    }

    #[test]
    fn test_moving_body_does_not_sleep() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut rb = RigidBody::dynamic(1.0);
        rb.velocity = Vec3::new(5.0, 0.0, 0.0);
        rb.gravity_scale = 0.0;
        world.add(e, rb);

        let config = PhysicsConfig {
            gravity: Vec3::ZERO,
            fixed_dt: 1.0 / 60.0,
            solver_iterations: 4,
        };

        for _ in 0..60 {
            integrate(&mut world, &config);
        }

        let rb = world.get::<RigidBody>(e).unwrap();
        assert!(!rb.is_sleeping, "fast-moving body should not sleep");
    }
}
