use voltex_ecs::{World, Entity};
use voltex_ecs::Transform;
use voltex_math::Vec3;

use crate::contact::ContactPoint;
use crate::rigid_body::{RigidBody, PhysicsConfig};
use crate::collision::detect_collisions;
use crate::integrator::integrate;

const POSITION_SLOP: f32 = 0.01;
const POSITION_PERCENT: f32 = 0.4;

pub fn resolve_collisions(world: &mut World, contacts: &[ContactPoint]) {
    let mut velocity_changes: Vec<(Entity, Vec3)> = Vec::new();
    let mut position_changes: Vec<(Entity, Vec3)> = Vec::new();

    for contact in contacts {
        let rb_a = world.get::<RigidBody>(contact.entity_a).copied();
        let rb_b = world.get::<RigidBody>(contact.entity_b).copied();

        let (rb_a, rb_b) = match (rb_a, rb_b) {
            (Some(a), Some(b)) => (a, b),
            _ => continue,
        };

        let inv_mass_a = rb_a.inv_mass();
        let inv_mass_b = rb_b.inv_mass();
        let inv_mass_sum = inv_mass_a + inv_mass_b;

        if inv_mass_sum == 0.0 {
            continue;
        }

        let v_rel = rb_a.velocity - rb_b.velocity;
        let v_rel_n = v_rel.dot(contact.normal);

        // normal points A→B; v_rel_n > 0 means A approaches B → apply impulse
        let j = if v_rel_n > 0.0 {
            let e = rb_a.restitution.min(rb_b.restitution);
            let j = (1.0 + e) * v_rel_n / inv_mass_sum;

            velocity_changes.push((contact.entity_a, contact.normal * (-j * inv_mass_a)));
            velocity_changes.push((contact.entity_b, contact.normal * (j * inv_mass_b)));

            j
        } else {
            // No separating impulse needed, but use contact depth to derive a
            // representative normal force magnitude for friction clamping.
            // A simple proxy: treat the penetration as providing a static normal force.
            contact.depth / inv_mass_sum
        };

        // Coulomb friction: tangential impulse clamped to mu * normal impulse
        let v_rel_n_scalar = v_rel.dot(contact.normal);
        let v_rel_tangent = v_rel - contact.normal * v_rel_n_scalar;
        let tangent_len = v_rel_tangent.length();

        if tangent_len > 1e-6 {
            let tangent = v_rel_tangent * (1.0 / tangent_len);

            // Friction coefficient: average of both bodies
            let mu = (rb_a.friction + rb_b.friction) * 0.5;

            // Coulomb's law: friction impulse <= mu * normal impulse
            let jt = -v_rel_tangent.dot(tangent) / inv_mass_sum;
            let friction_j = if jt.abs() <= j * mu {
                jt // static friction
            } else {
                j * mu * jt.signum() // dynamic friction (sliding), clamped magnitude
            };

            velocity_changes.push((contact.entity_a, tangent * (friction_j * inv_mass_a)));
            velocity_changes.push((contact.entity_b, tangent * (-friction_j * inv_mass_b)));
        }

        let correction_mag = (contact.depth - POSITION_SLOP).max(0.0) * POSITION_PERCENT / inv_mass_sum;
        if correction_mag > 0.0 {
            let correction = contact.normal * correction_mag;
            position_changes.push((contact.entity_a, correction * (-inv_mass_a)));
            position_changes.push((contact.entity_b, correction * inv_mass_b));
        }
    }

    for (entity, dv) in velocity_changes {
        if let Some(rb) = world.get_mut::<RigidBody>(entity) {
            rb.velocity = rb.velocity + dv;
        }
    }

    for (entity, dp) in position_changes {
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = t.position + dp;
        }
    }
}

pub fn physics_step(world: &mut World, config: &PhysicsConfig) {
    integrate(world, config);
    let contacts = detect_collisions(world);
    resolve_collisions(world, &contacts);
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::World;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;
    use crate::{Collider, RigidBody};
    use crate::collision::detect_collisions;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn test_two_dynamic_spheres_head_on() {
        let mut world = World::new();

        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::new(-0.5, 0.0, 0.0)));
        world.add(a, Collider::Sphere { radius: 1.0 });
        let mut rb_a = RigidBody::dynamic(1.0);
        rb_a.velocity = Vec3::new(1.0, 0.0, 0.0);
        rb_a.restitution = 1.0;
        rb_a.gravity_scale = 0.0;
        world.add(a, rb_a);

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(0.5, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });
        let mut rb_b = RigidBody::dynamic(1.0);
        rb_b.velocity = Vec3::new(-1.0, 0.0, 0.0);
        rb_b.restitution = 1.0;
        rb_b.gravity_scale = 0.0;
        world.add(b, rb_b);

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);

        resolve_collisions(&mut world, &contacts);

        let va = world.get::<RigidBody>(a).unwrap().velocity;
        let vb = world.get::<RigidBody>(b).unwrap().velocity;

        assert!(approx(va.x, -1.0));
        assert!(approx(vb.x, 1.0));
    }

    #[test]
    fn test_dynamic_vs_static_floor() {
        let mut world = World::new();

        let ball = world.spawn();
        world.add(ball, Transform::from_position(Vec3::new(0.0, 0.5, 0.0)));
        world.add(ball, Collider::Sphere { radius: 1.0 });
        let mut rb = RigidBody::dynamic(1.0);
        rb.velocity = Vec3::new(0.0, -2.0, 0.0);
        rb.restitution = 1.0;
        rb.gravity_scale = 0.0;
        world.add(ball, rb);

        let floor = world.spawn();
        world.add(floor, Transform::from_position(Vec3::new(0.0, -1.0, 0.0)));
        world.add(floor, Collider::Box { half_extents: Vec3::new(10.0, 1.0, 10.0) });
        world.add(floor, RigidBody::statik());

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);

        resolve_collisions(&mut world, &contacts);

        let ball_rb = world.get::<RigidBody>(ball).unwrap();
        let floor_rb = world.get::<RigidBody>(floor).unwrap();

        assert!(ball_rb.velocity.y > 0.0);
        assert!(approx(floor_rb.velocity.y, 0.0));
    }

    #[test]
    fn test_position_correction() {
        let mut world = World::new();

        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });
        let mut rb_a = RigidBody::dynamic(1.0);
        rb_a.gravity_scale = 0.0;
        world.add(a, rb_a);

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });
        let mut rb_b = RigidBody::dynamic(1.0);
        rb_b.gravity_scale = 0.0;
        world.add(b, rb_b);

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);

        resolve_collisions(&mut world, &contacts);

        let pa = world.get::<Transform>(a).unwrap().position;
        let pb = world.get::<Transform>(b).unwrap().position;

        let dist = (pb - pa).length();
        assert!(dist > 1.0);
    }

    #[test]
    fn test_physics_step_ball_drop() {
        let mut world = World::new();

        let ball = world.spawn();
        world.add(ball, Transform::from_position(Vec3::new(0.0, 5.0, 0.0)));
        world.add(ball, Collider::Sphere { radius: 0.5 });
        world.add(ball, RigidBody::dynamic(1.0));

        let floor = world.spawn();
        world.add(floor, Transform::from_position(Vec3::new(0.0, -1.0, 0.0)));
        world.add(floor, Collider::Box { half_extents: Vec3::new(10.0, 1.0, 10.0) });
        world.add(floor, RigidBody::statik());

        let config = PhysicsConfig::default();

        for _ in 0..10 {
            physics_step(&mut world, &config);
        }

        let t = world.get::<Transform>(ball).unwrap();
        assert!(t.position.y < 5.0);
        assert!(t.position.y > -1.0);
    }

    #[test]
    fn test_friction_slows_sliding() {
        // Ball sliding on static floor with friction
        let mut world = World::new();

        let ball = world.spawn();
        world.add(ball, Transform::from_position(Vec3::new(0.0, 0.4, 0.0)));
        world.add(ball, Collider::Sphere { radius: 0.5 });
        let mut rb = RigidBody::dynamic(1.0);
        rb.velocity = Vec3::new(5.0, 0.0, 0.0); // sliding horizontally
        rb.gravity_scale = 0.0;
        rb.friction = 0.5;
        world.add(ball, rb);

        let floor = world.spawn();
        world.add(floor, Transform::from_position(Vec3::new(0.0, -0.5, 0.0)));
        world.add(floor, Collider::Box { half_extents: Vec3::new(10.0, 0.5, 10.0) });
        let mut floor_rb = RigidBody::statik();
        floor_rb.friction = 0.5;
        world.add(floor, floor_rb);

        // Ball center at 0.4, radius 0.5, floor top at 0.0 → overlap 0.1
        let contacts = detect_collisions(&world);
        if !contacts.is_empty() {
            resolve_collisions(&mut world, &contacts);
        }

        let ball_v = world.get::<RigidBody>(ball).unwrap().velocity;
        // X velocity should be reduced by friction
        assert!(ball_v.x < 5.0, "friction should slow horizontal velocity: {}", ball_v.x);
        assert!(ball_v.x > 0.0, "should still be moving: {}", ball_v.x);
    }

    #[test]
    fn test_both_static_no_response() {
        let mut world = World::new();

        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });
        world.add(a, RigidBody::statik());

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(0.5, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });
        world.add(b, RigidBody::statik());

        let contacts = detect_collisions(&world);
        resolve_collisions(&mut world, &contacts);

        let pa = world.get::<Transform>(a).unwrap().position;
        let pb = world.get::<Transform>(b).unwrap().position;
        assert!(approx(pa.x, 0.0));
        assert!(approx(pb.x, 0.5));
    }
}
