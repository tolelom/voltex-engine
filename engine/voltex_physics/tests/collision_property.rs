//! Randomized narrowphase property: for random Sphere / Box / Capsule /
//! box-shaped ConvexHull pairs,
//! `detect_collisions` must report a contact exactly when the shapes overlap
//! (by an independent distance oracle), its normal must point from A to B
//! (pushing B along it by `depth` separates the pair), and the answer must
//! not depend on which entity was spawned first.

use voltex_ecs::{Entity, Transform, World};
use voltex_math::Vec3;
use voltex_physics::{detect_collisions, Collider, ContactPoint, ConvexHull};

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

fn random_collider(rng: &mut XorShift) -> Collider {
    match rng.next() % 4 {
        0 => Collider::Sphere { radius: rng.range(0.2, 1.5) },
        1 => Collider::Box { half_extents: rng.vec(0.2, 1.5) },
        2 => Collider::Capsule { radius: rng.range(0.2, 1.0), half_height: rng.range(0.0, 1.5) },
        // A box written as a hull: goes through GJK/EPA but has an exact oracle.
        _ => {
            let h = rng.vec(0.2, 1.5);
            let corners = (0..8)
                .map(|i| Vec3::new(
                    if i & 1 == 0 { -h.x } else { h.x },
                    if i & 2 == 0 { -h.y } else { h.y },
                    if i & 4 == 0 { -h.z } else { h.z },
                ))
                .collect();
            Collider::ConvexHull(ConvexHull::new(corners))
        }
    }
}

/// The oracle's view of a collider: box-shaped hulls are boxes.
fn oracle_shape(c: &Collider) -> Collider {
    match c {
        Collider::ConvexHull(hull) => {
            let h = hull.vertices.iter().fold(Vec3::ZERO, |m, v| {
                Vec3::new(m.x.max(v.x.abs()), m.y.max(v.y.abs()), m.z.max(v.z.abs()))
            });
            Collider::Box { half_extents: h }
        }
        other => other.clone(),
    }
}

fn box_sdf(q: Vec3, h: Vec3) -> f32 {
    let d = Vec3::new(q.x.abs() - h.x, q.y.abs() - h.y, q.z.abs() - h.z);
    Vec3::new(d.x.max(0.0), d.y.max(0.0), d.z.max(0.0)).length() + d.x.max(d.y).max(d.z).min(0.0)
}

/// Signed distance from point `p` to the collider placed at `pos`.
fn sdf(c: &Collider, pos: Vec3, p: Vec3) -> f32 {
    let q = p - pos;
    match c {
        Collider::Sphere { radius } => q.length() - radius,
        Collider::Box { half_extents } => box_sdf(q, *half_extents),
        Collider::Capsule { radius, half_height } => {
            (q - Vec3::new(0.0, q.y.clamp(-half_height, *half_height), 0.0)).length() - radius
        }
        Collider::ConvexHull(_) => unreachable!("not generated"),
    }
}

/// Every shape is a "core" (point, Y segment or box) inflated by a radius.
fn core_segment(c: &Collider) -> Option<(f32, f32)> {
    match c {
        Collider::Sphere { radius } => Some((0.0, *radius)),
        Collider::Capsule { radius, half_height } => Some((*half_height, *radius)),
        _ => None,
    }
}

/// Signed separation between two placed colliders (negative = overlap).
/// Exact for box-box; otherwise samples the segment core of one shape
/// against the exact SDF of the other (error <= half_height / 400).
fn separation(a: &Collider, pa: Vec3, b: &Collider, pb: Vec3) -> f32 {
    let (a, b) = (&oracle_shape(a), &oracle_shape(b));
    let (seg, other, po, ps) = match (core_segment(a), core_segment(b)) {
        (Some(s), _) => (s, b, pb, pa),
        (None, Some(s)) => (s, a, pa, pb),
        (None, None) => {
            let (Collider::Box { half_extents: ha }, Collider::Box { half_extents: hb }) = (a, b) else {
                unreachable!()
            };
            return box_sdf(pb - pa, *ha + *hb);
        }
    };
    let (half_height, radius) = seg;
    (0..=400)
        .map(|i| {
            let y = -half_height + 2.0 * half_height * i as f32 / 400.0;
            sdf(other, po, ps + Vec3::new(0.0, y, 0.0))
        })
        .fold(f32::INFINITY, f32::min)
        - radius
}

fn contact_between(first: (&Collider, Vec3), second: (&Collider, Vec3)) -> (Vec<ContactPoint>, Entity) {
    let mut world = World::new();
    let e1 = world.spawn();
    world.add(e1, Transform::from_position(first.1));
    world.add(e1, first.0.clone());
    let e2 = world.spawn();
    world.add(e2, Transform::from_position(second.1));
    world.add(e2, second.0.clone());
    (detect_collisions(&world), e1)
}

#[test]
fn detect_collisions_matches_a_distance_oracle_for_random_pairs() {
    let (mut overlapping, mut apart) = (0, 0);
    for seed in [3u32, 0xFACE, 0x1357_9BDF, 77, 0x0C01_11DE] {
        let mut rng = XorShift(seed);
        for case in 0..400 {
            let a = random_collider(&mut rng);
            let b = random_collider(&mut rng);
            let pa = rng.vec(-1.0, 1.0);
            let pb = pa + rng.vec(-2.0, 2.0);
            let truth = separation(&a, pa, &b, pb);
            if truth.abs() < 0.02 {
                continue; // touching within oracle tolerance
            }
            let ctx = format!("seed {seed:x} case {case}: {a:?}@{pa:?} vs {b:?}@{pb:?}, separation {truth}");

            for (first, second) in [((&a, pa), (&b, pb)), ((&b, pb), (&a, pa))] {
                let (contacts, first_entity) = contact_between(first, second);
                assert!(contacts.len() <= 1, "{ctx}: {} contacts for one pair", contacts.len());
                let Some(c) = contacts.first() else {
                    assert!(truth > 0.0, "{ctx}: overlap missed");
                    continue;
                };
                assert!(truth < 0.0, "{ctx}: phantom contact {c:?}");
                assert!((c.normal.length() - 1.0).abs() < 1e-3, "{ctx}: normal not unit {:?}", c.normal);
                assert!(c.depth > 0.0 && c.depth.is_finite(), "{ctx}: depth {}", c.depth);
                let exact_oracle = !matches!(a, Collider::Capsule { .. }) && !matches!(b, Collider::Capsule { .. });
                if exact_oracle {
                    // Sphere/box pairs: the oracle's separation is the exact minimum translation.
                    assert!((c.depth + truth).abs() < 1e-3, "{ctx}: depth {} vs exact {}", c.depth, -truth);
                }
                // Pushing B along the A->B normal by depth (+ slack) must separate them.
                let (ca, pos_a, cb, pos_b) = if c.entity_a == first_entity {
                    (first.0, first.1, second.0, second.1)
                } else {
                    (second.0, second.1, first.0, first.1)
                };
                let pushed = separation(ca, pos_a, cb, pos_b + c.normal * (c.depth + 0.05));
                assert!(pushed > -0.01, "{ctx}: pushing B by normal*depth leaves overlap {pushed} ({c:?})");
            }
            if truth < 0.0 {
                overlapping += 1;
            } else {
                apart += 1;
            }
        }
    }
    assert!(overlapping > 300 && apart > 300, "generator too one-sided: {overlapping} / {apart}");
}

#[test]
fn capsule_vs_sphere_epa_terminates() {
    // Found by the property test above: EPA oriented faces by the sign of
    // their distance to the origin, which flips near-origin faces, so horizon
    // edges stopped cancelling and the face list grew ~3x per iteration
    // (120k faces by iteration 16) — detect_collisions effectively hung.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let capsule = Collider::Capsule { radius: 0.909024, half_height: 0.93042004 };
        let sphere = Collider::Sphere { radius: 1.380621 };
        let (contacts, _) = contact_between(
            (&capsule, Vec3::new(0.8642801, -1.3414799, -0.88127995)),
            (&sphere, Vec3::new(0.11266005, -0.16002, 0.06581998)),
        );
        let _ = tx.send(contacts);
    });
    let contacts = rx
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("detect_collisions did not finish within 20s");
    assert_eq!(contacts.len(), 1);
    assert!(contacts[0].depth > 0.5 && contacts[0].depth < 1.5, "{:?}", contacts[0]);
}
