//! Sequential-impulse contact solver + `physics_step` top-level driver.

use voltex_ecs::{World, Entity};
use voltex_ecs::Transform;
use voltex_math::Vec3;

use crate::collider::Collider;
use crate::contact::ContactPoint;
use crate::rigid_body::{RigidBody, PhysicsConfig};
use crate::collision::detect_collisions;
use crate::integrator::{integrate, inertia_tensor, inv_inertia};
use crate::ccd;

/// Penetration depth the solver tolerates before pushing bodies apart.
const POSITION_SLOP: f32 = 0.01;
/// Fraction of the remaining penetration corrected per step (Baumgarte factor).
const POSITION_PERCENT: f32 = 0.4;
/// Below this tangential slip speed no friction impulse is applied.
const MIN_SLIP_SPEED: f32 = 1e-6;

/// Runs `iterations` rounds of sequential-impulse resolution over `contacts`,
/// updating velocities and applying positional correction (Baumgarte-style).
pub fn resolve_collisions(world: &mut World, contacts: &[ContactPoint], iterations: u32) {
    // Wake sleeping bodies that are in contact
    wake_colliding_bodies(world, contacts);

    for iteration in 0..iterations {
        // Every contact in a round reads the same pre-round state; the changes
        // are applied together at the end of the round.
        let mut velocity_changes: Vec<VelocityChange> = Vec::new();
        let mut position_changes: Vec<(Entity, Vec3)> = Vec::new();

        for contact in contacts {
            let Some(bodies) = ContactBodies::gather(world, contact) else {
                continue;
            };
            solve_contact_velocity(&bodies, contact, &mut velocity_changes);
            // Position correction only on first iteration
            if iteration == 0 {
                push_position_correction(&bodies, contact, &mut position_changes);
            }
        }

        apply_velocity_changes(world, velocity_changes);
        apply_position_changes(world, position_changes);
    }
}

/// `(entity, linear velocity delta, angular velocity delta)`.
type VelocityChange = (Entity, Vec3, Vec3);

/// The two bodies of one contact, as the solver needs them.
struct ContactBodies {
    body_a: RigidBody,
    body_b: RigidBody,
    inv_mass_a: f32,
    inv_mass_b: f32,
    /// Lever arm from A's centre to its contact point.
    lever_a: Vec3,
    /// Lever arm from B's centre to its contact point.
    lever_b: Vec3,
    /// Diagonal inverse inertia of A (zero without a collider).
    inv_inertia_a: Vec3,
    /// Diagonal inverse inertia of B (zero without a collider).
    inv_inertia_b: Vec3,
}

impl ContactBodies {
    /// `None` when either entity has no `RigidBody` or both are immovable.
    fn gather(world: &World, contact: &ContactPoint) -> Option<Self> {
        let body_a = *world.get::<RigidBody>(contact.entity_a)?;
        let body_b = *world.get::<RigidBody>(contact.entity_b)?;
        let inv_mass_a = body_a.inv_mass();
        let inv_mass_b = body_b.inv_mass();
        if inv_mass_a + inv_mass_b == 0.0 {
            return None;
        }

        let center_a = world.get::<Transform>(contact.entity_a).map_or(Vec3::ZERO, |t| t.position);
        let center_b = world.get::<Transform>(contact.entity_b).map_or(Vec3::ZERO, |t| t.position);
        let inv_inertia_of = |entity: Entity, body: &RigidBody| {
            world
                .get::<Collider>(entity)
                .map_or(Vec3::ZERO, |collider| inv_inertia(inertia_tensor(collider, body.mass)))
        };

        Some(Self {
            inv_inertia_a: inv_inertia_of(contact.entity_a, &body_a),
            inv_inertia_b: inv_inertia_of(contact.entity_b, &body_b),
            lever_a: contact.point_on_a - center_a,
            lever_b: contact.point_on_b - center_b,
            body_a,
            body_b,
            inv_mass_a,
            inv_mass_b,
        })
    }

    fn inv_mass_sum(&self) -> f32 {
        self.inv_mass_a + self.inv_mass_b
    }

    /// Velocity of A's contact point minus B's (linear + angular).
    fn relative_contact_velocity(&self) -> Vec3 {
        let v_a = self.body_a.velocity + self.body_a.angular_velocity.cross(self.lever_a);
        let v_b = self.body_b.velocity + self.body_b.angular_velocity.cross(self.lever_b);
        v_a - v_b
    }

    /// Inverse effective mass of the contact along `dir` (linear + rotational):
    /// the change in relative velocity along `dir` per unit impulse.
    fn inverse_effective_mass_along(&self, dir: Vec3) -> f32 {
        self.inv_mass_sum()
            + self.lever_a.cross(dir).mul_elementwise(self.inv_inertia_a).cross(self.lever_a).dot(dir)
            + self.lever_b.cross(dir).mul_elementwise(self.inv_inertia_b).cross(self.lever_b).dot(dir)
    }

    /// Queues `impulse` on B and `-impulse` on A, linear and angular. Returns
    /// the resulting change of the relative contact velocity `v_a - v_b`.
    fn push_impulse(&self, contact: &ContactPoint, impulse: Vec3, changes: &mut Vec<VelocityChange>) -> Vec3 {
        let dv_a = impulse * -self.inv_mass_a;
        let dw_a = self.lever_a.cross(-impulse).mul_elementwise(self.inv_inertia_a);
        let dv_b = impulse * self.inv_mass_b;
        let dw_b = self.lever_b.cross(impulse).mul_elementwise(self.inv_inertia_b);
        changes.push((contact.entity_a, dv_a, dw_a));
        changes.push((contact.entity_b, dv_b, dw_b));
        (dv_a + dw_a.cross(self.lever_a)) - (dv_b + dw_b.cross(self.lever_b))
    }
}

/// Queues the normal (restitution) impulse and then the Coulomb friction
/// impulse for one contact.
fn solve_contact_velocity(bodies: &ContactBodies, contact: &ContactPoint, changes: &mut Vec<VelocityChange>) {
    let v_rel = bodies.relative_contact_velocity();
    let v_rel_n = v_rel.dot(contact.normal);
    let inverse_effective_mass = bodies.inverse_effective_mass_along(contact.normal);

    // normal points A→B; v_rel_n > 0 means A approaches B → apply impulse
    let normal_impulse = if v_rel_n > 0.0 {
        let restitution = bodies.body_a.restitution.min(bodies.body_b.restitution);
        (1.0 + restitution) * v_rel_n / inverse_effective_mass
    } else {
        0.0
    };
    let dv_rel = bodies.push_impulse(contact, contact.normal * normal_impulse, changes);

    // Coulomb friction: only on a contact that is pushing (normal_impulse > 0),
    // sized with the tangent's own effective mass, from the slip left *after*
    // the normal impulse. The old version used the normal's effective mass,
    // the pre-impulse slip, and a depth-derived impulse on separating
    // contacts — each of which could overshoot and inject energy.
    let v_rel = v_rel + dv_rel;
    let v_rel_tangent = v_rel - contact.normal * v_rel.dot(contact.normal);
    let slip_speed = v_rel_tangent.length();

    if normal_impulse > 0.0 && slip_speed > MIN_SLIP_SPEED {
        let tangent = v_rel_tangent * (1.0 / slip_speed);
        let mu = (bodies.body_a.friction + bodies.body_b.friction) * 0.5;
        let inverse_tangent_mass = bodies.inverse_effective_mass_along(tangent);
        // Impulse on B along +tangent that would cancel the slip, clamped to the friction cone.
        let friction_impulse = (slip_speed / inverse_tangent_mass).min(normal_impulse * mu);
        bodies.push_impulse(contact, tangent * friction_impulse, changes);
    }
}

/// Queues a push-apart along the normal, split by inverse mass, for the part
/// of the penetration beyond [`POSITION_SLOP`].
fn push_position_correction(bodies: &ContactBodies, contact: &ContactPoint, changes: &mut Vec<(Entity, Vec3)>) {
    let correction_mag =
        (contact.depth - POSITION_SLOP).max(0.0) * POSITION_PERCENT / bodies.inv_mass_sum();
    if correction_mag > 0.0 {
        let correction = contact.normal * correction_mag;
        changes.push((contact.entity_a, correction * (-bodies.inv_mass_a)));
        changes.push((contact.entity_b, correction * bodies.inv_mass_b));
    }
}

fn apply_velocity_changes(world: &mut World, changes: Vec<VelocityChange>) {
    for (entity, dv, dw) in changes {
        if let Some(rb) = world.get_mut::<RigidBody>(entity) {
            rb.velocity = rb.velocity + dv;
            rb.angular_velocity = rb.angular_velocity + dw;
        }
    }
}

fn apply_position_changes(world: &mut World, changes: Vec<(Entity, Vec3)>) {
    for (entity, dp) in changes {
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = t.position + dp;
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

/// Runs one full physics step: CCD → integrate → detect → resolve.
pub fn physics_step(world: &mut World, config: &PhysicsConfig) {
    // CCD sweeps each fast body from where it was to where integration put
    // it, so it has to bracket the integrate call.
    let positions_before_integration = ccd::snapshot_positions(world);
    integrate(world, config);
    ccd::clamp_tunnelling(world, &positions_before_integration);
    let contacts = detect_collisions(world);
    resolve_collisions(world, &contacts, config.solver_iterations);
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
        world.add(floor, RigidBody::fixed());

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
        world.add(floor, RigidBody::fixed());

        let config = PhysicsConfig::default();

        // Run long enough to fall ~4.5 units and settle onto the floor.
        for _ in 0..300 {
            physics_step(&mut world, &config);
        }

        let t = world.get::<Transform>(ball).unwrap();
        // Floor box center y=-1.0, half_extent.y=1.0 → floor top at y=0.0.
        // Ball radius 0.5 → resting center y ≈ 0.5 (within position slop).
        // Solver leaves the body penetrated by up to POSITION_SLOP, so rest is
        // marginally below 0.5 (observed ~0.487).
        let rest_y = 0.5;
        assert!((t.position.y - rest_y).abs() < POSITION_SLOP + 0.05,
            "ball should rest at y~{rest_y}, got {}", t.position.y);
    }

    #[test]
    fn test_friction_slows_sliding() {
        let mut world = World::new();

        let ball = world.spawn();
        world.add(ball, Transform::from_position(Vec3::new(0.0, 0.4, 0.0)));
        world.add(ball, Collider::Sphere { radius: 0.5 });
        let mut rb = RigidBody::dynamic(1.0);
        // Coulomb friction needs a normal load: the ball is pressing into the
        // floor (as gravity makes it every frame). With no approach there is
        // no normal impulse and therefore no friction.
        rb.velocity = Vec3::new(5.0, -1.0, 0.0);
        rb.gravity_scale = 0.0;
        rb.friction = 0.5;
        world.add(ball, rb);

        let floor = world.spawn();
        world.add(floor, Transform::from_position(Vec3::new(0.0, -0.5, 0.0)));
        world.add(floor, Collider::Box { half_extents: Vec3::new(10.0, 0.5, 10.0) });
        let mut floor_rb = RigidBody::fixed();
        floor_rb.friction = 0.5;
        world.add(floor, floor_rb);

        let contacts = detect_collisions(&world);
        assert!(!contacts.is_empty(), "ball should be in contact with floor");
        resolve_collisions(&mut world, &contacts, 1);

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
        world.add(a, RigidBody::fixed());

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(0.5, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });
        world.add(b, RigidBody::fixed());

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

        // The collision lies entirely in the XY plane, so the induced spin must be
        // about the Z axis only (X/Y angular components stay ~0).
        let wa = rb_a_after.angular_velocity;
        let wb = rb_b_after.angular_velocity;
        assert!(wa.x.abs() < 1e-4 && wa.y.abs() < 1e-4, "A spin should be Z-only, got {wa:?}");
        assert!(wb.x.abs() < 1e-4 && wb.y.abs() < 1e-4, "B spin should be Z-only, got {wb:?}");
        // A approaches B from the upper-left along +X; the off-center contact
        // (normal A->B = (1,-1,0)/sqrt2) torques both bodies clockwise about -Z.
        assert!(wa.z < -1e-4, "A should spin about -Z, got {}", wa.z);
        assert!(wb.z < -1e-4, "B should spin about -Z, got {}", wb.z);
    }

    // --- Sequential impulse tests ---

    #[test]
    fn test_sequential_impulse_stability() {
        // Stack of 3 boxes on floor - with iterations they should be more stable
        let mut world = World::new();

        let floor = world.spawn();
        world.add(floor, Transform::from_position(Vec3::new(0.0, -0.5, 0.0)));
        world.add(floor, Collider::Box { half_extents: Vec3::new(10.0, 0.5, 10.0) });
        world.add(floor, RigidBody::fixed());

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
            assert!((t.position.y - expected_y).abs() < 0.1,
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
