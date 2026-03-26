use voltex_ecs::{World, Entity};
use voltex_ecs::Transform;
use voltex_math::Vec3;

use crate::collider::Collider;
use crate::contact::ContactPoint;
use crate::rigid_body::{RigidBody, PhysicsConfig};
use crate::collision::detect_collisions;
use crate::integrator::{integrate, inertia_tensor, inv_inertia};
use crate::ccd;

const POSITION_SLOP: f32 = 0.01;
const POSITION_PERCENT: f32 = 0.4;

pub fn resolve_collisions(world: &mut World, contacts: &[ContactPoint], iterations: u32) {
    // Wake sleeping bodies that are in contact
    wake_colliding_bodies(world, contacts);

    for _iter in 0..iterations {
        let mut velocity_changes: Vec<(Entity, Vec3, Vec3)> = Vec::new(); // (entity, dv_linear, dv_angular)
        let mut position_changes: Vec<(Entity, Vec3)> = Vec::new();

        for contact in contacts {
            let rb_a = world.get::<RigidBody>(contact.entity_a).copied();
            let rb_b = world.get::<RigidBody>(contact.entity_b).copied();
            let col_a = world.get::<Collider>(contact.entity_a).cloned();
            let col_b = world.get::<Collider>(contact.entity_b).cloned();
            let pos_a = world.get::<Transform>(contact.entity_a).map(|t| t.position);
            let pos_b = world.get::<Transform>(contact.entity_b).map(|t| t.position);

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

            // Compute lever arms for angular impulse
            let center_a = pos_a.unwrap_or(Vec3::ZERO);
            let center_b = pos_b.unwrap_or(Vec3::ZERO);
            let r_a = contact.point_on_a - center_a;
            let r_b = contact.point_on_b - center_b;

            // Compute inverse inertia
            let inv_i_a = col_a.map(|c| inv_inertia(inertia_tensor(&c, rb_a.mass)))
                .unwrap_or(Vec3::ZERO);
            let inv_i_b = col_b.map(|c| inv_inertia(inertia_tensor(&c, rb_b.mass)))
                .unwrap_or(Vec3::ZERO);

            // Relative velocity at contact point (including angular contribution)
            let v_a = rb_a.velocity + rb_a.angular_velocity.cross(r_a);
            let v_b = rb_b.velocity + rb_b.angular_velocity.cross(r_b);
            let v_rel = v_a - v_b;
            let v_rel_n = v_rel.dot(contact.normal);

            // Effective mass including rotational terms
            let r_a_cross_n = r_a.cross(contact.normal);
            let r_b_cross_n = r_b.cross(contact.normal);
            let angular_term_a = Vec3::new(
                r_a_cross_n.x * inv_i_a.x,
                r_a_cross_n.y * inv_i_a.y,
                r_a_cross_n.z * inv_i_a.z,
            ).cross(r_a).dot(contact.normal);
            let angular_term_b = Vec3::new(
                r_b_cross_n.x * inv_i_b.x,
                r_b_cross_n.y * inv_i_b.y,
                r_b_cross_n.z * inv_i_b.z,
            ).cross(r_b).dot(contact.normal);

            let effective_mass = inv_mass_sum + angular_term_a + angular_term_b;

            // normal points A→B; v_rel_n > 0 means A approaches B → apply impulse
            let j = if v_rel_n > 0.0 {
                let e = rb_a.restitution.min(rb_b.restitution);
                let j = (1.0 + e) * v_rel_n / effective_mass;

                // Linear impulse
                velocity_changes.push((contact.entity_a, contact.normal * (-j * inv_mass_a), Vec3::ZERO));
                velocity_changes.push((contact.entity_b, contact.normal * (j * inv_mass_b), Vec3::ZERO));

                // Angular impulse: torque = r × impulse
                let angular_impulse_a = r_a.cross(contact.normal * (-j));
                let angular_impulse_b = r_b.cross(contact.normal * j);
                let dw_a = Vec3::new(
                    angular_impulse_a.x * inv_i_a.x,
                    angular_impulse_a.y * inv_i_a.y,
                    angular_impulse_a.z * inv_i_a.z,
                );
                let dw_b = Vec3::new(
                    angular_impulse_b.x * inv_i_b.x,
                    angular_impulse_b.y * inv_i_b.y,
                    angular_impulse_b.z * inv_i_b.z,
                );
                velocity_changes.push((contact.entity_a, Vec3::ZERO, dw_a));
                velocity_changes.push((contact.entity_b, Vec3::ZERO, dw_b));

                j
            } else {
                contact.depth / inv_mass_sum
            };

            // Coulomb friction: tangential impulse clamped to mu * normal impulse
            let v_rel_tangent = v_rel - contact.normal * v_rel_n;
            let tangent_len = v_rel_tangent.length();

            if tangent_len > 1e-6 {
                let tangent = v_rel_tangent * (1.0 / tangent_len);
                let mu = (rb_a.friction + rb_b.friction) * 0.5;

                let jt = -v_rel_tangent.dot(tangent) / effective_mass;
                let friction_j = if jt.abs() <= j * mu {
                    jt
                } else {
                    j * mu * jt.signum()
                };

                velocity_changes.push((contact.entity_a, tangent * (friction_j * inv_mass_a), Vec3::ZERO));
                velocity_changes.push((contact.entity_b, tangent * (-friction_j * inv_mass_b), Vec3::ZERO));

                // Angular friction impulse
                let angular_fric_a = r_a.cross(tangent * friction_j);
                let angular_fric_b = r_b.cross(tangent * (-friction_j));
                let dw_fric_a = Vec3::new(
                    angular_fric_a.x * inv_i_a.x,
                    angular_fric_a.y * inv_i_a.y,
                    angular_fric_a.z * inv_i_a.z,
                );
                let dw_fric_b = Vec3::new(
                    angular_fric_b.x * inv_i_b.x,
                    angular_fric_b.y * inv_i_b.y,
                    angular_fric_b.z * inv_i_b.z,
                );
                velocity_changes.push((contact.entity_a, Vec3::ZERO, dw_fric_a));
                velocity_changes.push((contact.entity_b, Vec3::ZERO, dw_fric_b));
            }

            // Position correction only on first iteration
            if _iter == 0 {
                let correction_mag = (contact.depth - POSITION_SLOP).max(0.0) * POSITION_PERCENT / inv_mass_sum;
                if correction_mag > 0.0 {
                    let correction = contact.normal * correction_mag;
                    position_changes.push((contact.entity_a, correction * (-inv_mass_a)));
                    position_changes.push((contact.entity_b, correction * inv_mass_b));
                }
            }
        }

        // Apply velocity changes
        for (entity, dv, dw) in velocity_changes {
            if let Some(rb) = world.get_mut::<RigidBody>(entity) {
                rb.velocity = rb.velocity + dv;
                rb.angular_velocity = rb.angular_velocity + dw;
            }
        }

        // Apply position corrections
        for (entity, dp) in position_changes {
            if let Some(t) = world.get_mut::<Transform>(entity) {
                t.position = t.position + dp;
            }
        }
    }
}

fn wake_colliding_bodies(world: &mut World, contacts: &[ContactPoint]) {
    let wake_list: Vec<Entity> = contacts
        .iter()
        .flat_map(|c| {
            let mut entities = Vec::new();
            if let Some(rb) = world.get::<RigidBody>(c.entity_a) {
                if rb.is_sleeping { entities.push(c.entity_a); }
            }
            if let Some(rb) = world.get::<RigidBody>(c.entity_b) {
                if rb.is_sleeping { entities.push(c.entity_b); }
            }
            entities
        })
        .collect();

    for entity in wake_list {
        if let Some(rb) = world.get_mut::<RigidBody>(entity) {
            rb.wake();
        }
    }
}

pub fn physics_step(world: &mut World, config: &PhysicsConfig) {
    // CCD: for fast-moving bodies, check for tunneling
    apply_ccd(world, config);

    integrate(world, config);
    let contacts = detect_collisions(world);
    resolve_collisions(world, &contacts, config.solver_iterations);
}

fn apply_ccd(world: &mut World, config: &PhysicsConfig) {
    // Gather fast-moving bodies and all collider AABBs
    let bodies: Vec<(Entity, Vec3, Vec3, Collider)> = world
        .query3::<Transform, RigidBody, Collider>()
        .into_iter()
        .filter(|(_, _, rb, _)| !rb.is_static() && !rb.is_sleeping)
        .map(|(e, t, rb, c)| (e, t.position, rb.velocity, c.clone()))
        .collect();

    let all_colliders: Vec<(Entity, voltex_math::AABB)> = world
        .query2::<Transform, Collider>()
        .into_iter()
        .map(|(e, t, c)| (e, c.aabb(t.position)))
        .collect();

    let mut ccd_corrections: Vec<(Entity, Vec3)> = Vec::new();

    for (entity, pos, vel, collider) in &bodies {
        let speed = vel.length();
        let collider_radius = match collider {
            Collider::Sphere { radius } => *radius,
            Collider::Box { half_extents } => half_extents.x.min(half_extents.y).min(half_extents.z),
            Collider::Capsule { radius, .. } => *radius,
            Collider::ConvexHull(hull) => {
                // Use minimum distance from origin to any vertex as approximate radius
                hull.vertices.iter().map(|v| v.length()).fold(f32::MAX, f32::min)
            }
        };

        // Only apply CCD if displacement > collider radius
        if speed * config.fixed_dt <= collider_radius {
            continue;
        }

        let sweep_radius = match collider {
            Collider::Sphere { radius } => *radius,
            _ => collider_radius,
        };

        let end = *pos + *vel * config.fixed_dt;

        let mut earliest_t = 1.0f32;

        for (other_entity, other_aabb) in &all_colliders {
            if *other_entity == *entity {
                continue;
            }

            if let Some(t) = ccd::swept_sphere_vs_aabb(*pos, end, sweep_radius, other_aabb) {
                if t < earliest_t {
                    earliest_t = t;
                }
            }
        }

        if earliest_t < 1.0 {
            // Place body just before collision point
            let safe_t = (earliest_t - 0.01).max(0.0);
            let safe_pos = *pos + *vel * config.fixed_dt * safe_t;
            ccd_corrections.push((*entity, safe_pos));
        }
    }

    for (entity, safe_pos) in ccd_corrections {
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = safe_pos;
        }
        if let Some(rb) = world.get_mut::<RigidBody>(entity) {
            // Reduce velocity to prevent re-tunneling
            rb.velocity = rb.velocity * 0.5;
        }
    }
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

        resolve_collisions(&mut world, &contacts, 1);

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

        resolve_collisions(&mut world, &contacts, 1);

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

        resolve_collisions(&mut world, &contacts, 1);

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
        let mut world = World::new();

        let ball = world.spawn();
        world.add(ball, Transform::from_position(Vec3::new(0.0, 0.4, 0.0)));
        world.add(ball, Collider::Sphere { radius: 0.5 });
        let mut rb = RigidBody::dynamic(1.0);
        rb.velocity = Vec3::new(5.0, 0.0, 0.0);
        rb.gravity_scale = 0.0;
        rb.friction = 0.5;
        world.add(ball, rb);

        let floor = world.spawn();
        world.add(floor, Transform::from_position(Vec3::new(0.0, -0.5, 0.0)));
        world.add(floor, Collider::Box { half_extents: Vec3::new(10.0, 0.5, 10.0) });
        let mut floor_rb = RigidBody::statik();
        floor_rb.friction = 0.5;
        world.add(floor, floor_rb);

        let contacts = detect_collisions(&world);
        if !contacts.is_empty() {
            resolve_collisions(&mut world, &contacts, 1);
        }

        let ball_v = world.get::<RigidBody>(ball).unwrap().velocity;
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
        resolve_collisions(&mut world, &contacts, 1);

        let pa = world.get::<Transform>(a).unwrap().position;
        let pb = world.get::<Transform>(b).unwrap().position;
        assert!(approx(pa.x, 0.0));
        assert!(approx(pb.x, 0.5));
    }

    // --- Angular impulse tests ---

    #[test]
    fn test_off_center_hit_produces_spin() {
        let mut world = World::new();

        // Sphere A moving right, hitting sphere B off-center (offset in Y)
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::new(-0.5, 0.5, 0.0)));
        world.add(a, Collider::Sphere { radius: 1.0 });
        let mut rb_a = RigidBody::dynamic(1.0);
        rb_a.velocity = Vec3::new(2.0, 0.0, 0.0);
        rb_a.restitution = 0.5;
        rb_a.gravity_scale = 0.0;
        world.add(a, rb_a);

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(0.5, -0.5, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });
        let mut rb_b = RigidBody::dynamic(1.0);
        rb_b.gravity_scale = 0.0;
        rb_b.restitution = 0.5;
        world.add(b, rb_b);

        let contacts = detect_collisions(&world);
        assert!(!contacts.is_empty());

        resolve_collisions(&mut world, &contacts, 4);

        let rb_a_after = world.get::<RigidBody>(a).unwrap();
        let rb_b_after = world.get::<RigidBody>(b).unwrap();

        // At least one body should have non-zero angular velocity after off-center collision
        let total_angular = rb_a_after.angular_velocity.length() + rb_b_after.angular_velocity.length();
        assert!(total_angular > 1e-4, "off-center hit should produce angular velocity, got {}", total_angular);
    }

    // --- Sequential impulse tests ---

    #[test]
    fn test_sequential_impulse_stability() {
        // Stack of 3 boxes on floor - with iterations they should be more stable
        let mut world = World::new();

        let floor = world.spawn();
        world.add(floor, Transform::from_position(Vec3::new(0.0, -0.5, 0.0)));
        world.add(floor, Collider::Box { half_extents: Vec3::new(10.0, 0.5, 10.0) });
        world.add(floor, RigidBody::statik());

        let mut boxes = Vec::new();
        for i in 0..3 {
            let e = world.spawn();
            let y = 0.5 + i as f32 * 1.0;
            world.add(e, Transform::from_position(Vec3::new(0.0, y, 0.0)));
            world.add(e, Collider::Box { half_extents: Vec3::new(0.5, 0.5, 0.5) });
            let mut rb = RigidBody::dynamic(1.0);
            rb.gravity_scale = 0.0; // no gravity for stability test
            world.add(e, rb);
            boxes.push(e);
        }

        let config = PhysicsConfig {
            gravity: Vec3::ZERO,
            fixed_dt: 1.0 / 60.0,
            solver_iterations: 4,
        };

        // Run a few steps
        for _ in 0..5 {
            physics_step(&mut world, &config);
        }

        // All boxes should remain roughly in place (no gravity, just resting)
        for (i, e) in boxes.iter().enumerate() {
            let t = world.get::<Transform>(*e).unwrap();
            let expected_y = 0.5 + i as f32 * 1.0;
            assert!((t.position.y - expected_y).abs() < 1.0,
                "box {} moved too much: expected y~{}, got {}", i, expected_y, t.position.y);
        }
    }

    // --- Wake on collision test ---

    #[test]
    fn test_wake_on_collision() {
        let mut world = World::new();

        // Sleeping body
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });
        let mut rb_a = RigidBody::dynamic(1.0);
        rb_a.is_sleeping = true;
        rb_a.gravity_scale = 0.0;
        world.add(a, rb_a);

        // Moving body that collides with sleeping body
        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.5, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });
        let mut rb_b = RigidBody::dynamic(1.0);
        rb_b.velocity = Vec3::new(-2.0, 0.0, 0.0);
        rb_b.gravity_scale = 0.0;
        world.add(b, rb_b);

        let contacts = detect_collisions(&world);
        assert!(!contacts.is_empty());

        resolve_collisions(&mut world, &contacts, 1);

        let rb_a_after = world.get::<RigidBody>(a).unwrap();
        assert!(!rb_a_after.is_sleeping, "body should wake on collision");
    }
}
