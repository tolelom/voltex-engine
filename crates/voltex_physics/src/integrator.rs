use voltex_ecs::World;
use voltex_ecs::Transform;
use voltex_math::Vec3;
use crate::rigid_body::{RigidBody, PhysicsConfig};

pub fn integrate(world: &mut World, config: &PhysicsConfig) {
    // 1. Collect
    let updates: Vec<(voltex_ecs::Entity, Vec3, Vec3)> = world
        .query2::<Transform, RigidBody>()
        .into_iter()
        .filter(|(_, _, rb)| !rb.is_static())
        .map(|(entity, transform, rb)| {
            let new_velocity = rb.velocity + config.gravity * rb.gravity_scale * config.fixed_dt;
            let new_position = transform.position + new_velocity * config.fixed_dt;
            (entity, new_velocity, new_position)
        })
        .collect();

    // 2. Apply
    for (entity, new_velocity, new_position) in updates {
        if let Some(rb) = world.get_mut::<RigidBody>(entity) {
            rb.velocity = new_velocity;
        }
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = new_position;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::World;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;
    use crate::RigidBody;

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
}
