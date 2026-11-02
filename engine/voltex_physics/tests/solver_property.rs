//! Randomized single-contact property for `resolve_collisions`: two dynamic
//! bodies (sphere / box) meeting with random masses, velocities, spins,
//! restitution and friction must come out with linear momentum conserved,
//! kinetic energy (linear + angular) not increased, and no approaching
//! velocity left along the contact normal.

use voltex_ecs::{Entity, Transform, World};
use voltex_math::Vec3;
use voltex_physics::{detect_collisions, inertia_tensor, resolve_collisions, Collider, RigidBody};

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() % 100_000) as f32 / 100_000.0 * (hi - lo)
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vec3 {
        Vec3::new(self.range(lo, hi), self.range(lo, hi), self.range(lo, hi))
    }
}

fn spawn(world: &mut World, rng: &mut XorShift, position: Vec3, collider: Collider) -> Entity {
    let e = world.spawn();
    world.add(e, Transform::from_position(position));
    world.add(e, collider);
    let mut body = RigidBody::dynamic(rng.range(0.2, 5.0));
    body.velocity = rng.vec(-5.0, 5.0);
    body.angular_velocity = rng.vec(-3.0, 3.0);
    body.restitution = rng.range(0.0, 1.0);
    body.friction = rng.range(0.0, 1.0);
    world.add(e, body);
    e
}

fn random_collider(rng: &mut XorShift) -> Collider {
    if rng.next().is_multiple_of(2) {
        Collider::Sphere { radius: rng.range(0.3, 1.0) }
    } else {
        Collider::Box { half_extents: rng.vec(0.3, 1.0) }
    }
}

fn momentum(world: &World, es: &[Entity]) -> Vec3 {
    es.iter().fold(Vec3::ZERO, |acc, e| {
        let b = world.get::<RigidBody>(*e).unwrap();
        acc + b.velocity * b.mass
    })
}

fn energy(world: &World, es: &[Entity]) -> f32 {
    es.iter()
        .map(|e| {
            let b = world.get::<RigidBody>(*e).unwrap();
            let i = inertia_tensor(world.get::<Collider>(*e).unwrap(), b.mass);
            let w = b.angular_velocity;
            0.5 * b.mass * b.velocity.length_squared()
                + 0.5 * (i.x * w.x * w.x + i.y * w.y * w.y + i.z * w.z * w.z)
        })
        .sum()
}

/// A random two-body world, or `None` when the pair doesn't produce exactly
/// one contact. Deterministic in `state`, so the same case can be rebuilt.
fn random_pair(state: u32) -> Option<(World, [Entity; 2])> {
    let mut rng = XorShift(state);
    let mut world = World::new();
    let ca = random_collider(&mut rng);
    let cb = random_collider(&mut rng);
    let a = spawn(&mut world, &mut rng, Vec3::ZERO, ca);
    let offset = rng.vec(-1.2, 1.2);
    let b = spawn(&mut world, &mut rng, offset, cb);
    (detect_collisions(&world).len() == 1).then_some((world, [a, b]))
}

fn case_states() -> impl Iterator<Item = u32> {
    [5u32, 0xB0B, 0x7777_1234, 99_991].into_iter().flat_map(|seed| {
        let mut rng = XorShift(seed);
        (0..300).map(move |_| rng.next())
    })
}

#[test]
fn contact_resolution_conserves_momentum_and_never_adds_energy() {
    // Default PhysicsConfig iteration count.
    let mut resolved = 0;
    for state in case_states() {
        let Some((mut world, es)) = random_pair(state) else { continue };
        let contacts = detect_collisions(&world);
        let ctx = format!("state {state:x}: {:?}", contacts[0]);
        let (p0, e0) = (momentum(&world, &es), energy(&world, &es));
        resolve_collisions(&mut world, &contacts, 4);
        let (p1, e1) = (momentum(&world, &es), energy(&world, &es));
        assert!((p1 - p0).length() < 1e-3 * (1.0 + p0.length()), "{ctx}: momentum {p0:?} -> {p1:?}");
        assert!(e1 <= e0 * (1.0 + 1e-4) + 1e-4, "{ctx}: energy grew {e0} -> {e1}");
        resolved += 1;
    }
    assert!(resolved > 400, "too few contacts exercised: {resolved}");
}

#[test]
fn contact_resolution_converges_to_non_approaching() {
    // Normal and friction impulses of one contact are computed from the same
    // velocities (Jacobi), so with friction the residual approach shrinks
    // geometrically rather than vanishing in one pass; 4 iterations can leave
    // a few cm/s. It must still converge.
    for state in case_states() {
        let Some((mut world, _)) = random_pair(state) else { continue };
        let contacts = detect_collisions(&world);
        let c = contacts[0];
        let center = |w: &World, e: Entity| w.get::<Transform>(e).unwrap().position;
        // Lever arms as the solver saw them (position correction moves centres).
        let (r_a, r_b) = (c.point_on_a - center(&world, c.entity_a), c.point_on_b - center(&world, c.entity_b));
        resolve_collisions(&mut world, &contacts, 64);
        let body = |e: Entity| *world.get::<RigidBody>(e).unwrap();
        let (ba, bb) = (body(c.entity_a), body(c.entity_b));
        let va = ba.velocity + ba.angular_velocity.cross(r_a);
        let vb = bb.velocity + bb.angular_velocity.cross(r_b);
        let approach = (va - vb).dot(c.normal);
        assert!(approach < 1e-3, "state {state:x}: {c:?}: still approaching at {approach}");
    }
}
