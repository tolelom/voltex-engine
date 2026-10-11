//! Randomized raycast property: every hit/miss reported by `raycast` against
//! a single Sphere / Box / Capsule collider must agree with a brute-force
//! march of the shape's signed distance field along the same ray.
//! Rays start outside the shape (the "starts inside" conventions differ per
//! shape and are covered by unit tests).

use voltex_ecs::{Transform, World};
use voltex_math::{Ray, Vec3};
use voltex_physics::{raycast, raycast_all, Collider};

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    /// Uniform in [lo, hi).
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() % 100_000) as f32 / 100_000.0 * (hi - lo)
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vec3 {
        Vec3::new(self.range(lo, hi), self.range(lo, hi), self.range(lo, hi))
    }
}

fn sdf(collider: &Collider, center: Vec3, p: Vec3) -> f32 {
    let q = p - center;
    match collider {
        Collider::Sphere { radius } => q.length() - radius,
        Collider::Box { half_extents: h } => {
            let d = Vec3::new(q.x.abs() - h.x, q.y.abs() - h.y, q.z.abs() - h.z);
            let outside = Vec3::new(d.x.max(0.0), d.y.max(0.0), d.z.max(0.0)).length();
            outside + d.x.max(d.y).max(d.z).min(0.0)
        }
        Collider::Capsule { radius, half_height } => {
            let y = q.y.clamp(-half_height, *half_height);
            (q - Vec3::new(0.0, y, 0.0)).length() - radius
        }
        Collider::ConvexHull(_) => unreachable!("not generated"),
    }
}

fn random_collider(rng: &mut XorShift) -> Collider {
    match rng.next() % 3 {
        0 => Collider::Sphere { radius: rng.range(0.2, 2.0) },
        1 => Collider::Box { half_extents: rng.vec(0.2, 2.0) },
        _ => Collider::Capsule { radius: rng.range(0.2, 1.5), half_height: rng.range(0.0, 2.0) },
    }
}

/// First parameter in [0, max_dist] where the marched SDF goes negative.
fn marched_entry(collider: &Collider, center: Vec3, ray: &Ray, max_dist: f32) -> Option<f32> {
    let step = 0.002;
    let mut t = 0.0;
    while t <= max_dist {
        if sdf(collider, center, ray.at(t)) < 0.0 {
            return Some(t);
        }
        t += step;
    }
    None
}

#[test]
fn raycast_agrees_with_sdf_march_for_primitive_colliders() {
    let (mut hits, mut misses) = (0, 0);
    for seed in [7u32, 0xACE1, 0xBADC0DE, 2024, 0x5EED_5EED] {
        let mut rng = XorShift(seed);
        for case in 0..400 {
            let collider = random_collider(&mut rng);
            let center = rng.vec(-3.0, 3.0);
            // Aim near the shape so hits and grazing misses are both common.
            let origin = loop {
                let o = rng.vec(-8.0, 8.0);
                if sdf(&collider, center, o) > 0.05 {
                    break o;
                }
            };
            let target = center + rng.vec(-2.0, 2.0);
            let ray = Ray::new(origin, (target - origin).normalize());
            let max_dist = rng.range(4.0, 25.0);

            let mut world = World::new();
            let e = world.spawn();
            world.add(e, Transform::from_position(center));
            world.add(e, collider.clone());

            let ctx = format!("seed {seed:x} case {case}: {collider:?} at {center:?}, ray {ray:?}, max {max_dist}");
            let got = raycast(&world, &ray, max_dist);
            let all = raycast_all(&world, &ray, max_dist);
            assert_eq!(got.map(|h| h.t), all.first().map(|h| h.t), "{ctx}: raycast vs raycast_all");

            match (got, marched_entry(&collider, center, &ray, max_dist)) {
                (Some(hit), Some(entry)) => {
                    hits += 1;
                    assert!((hit.t - entry).abs() < 0.01, "{ctx}: t {} vs marched {entry}", hit.t);
                    assert!(sdf(&collider, center, hit.point).abs() < 1e-3, "{ctx}: hit point off surface");
                    assert!((hit.normal.length() - 1.0).abs() < 1e-3, "{ctx}: normal not unit {:?}", hit.normal);
                    let outward = hit.point + hit.normal * 0.01;
                    assert!(sdf(&collider, center, outward) > 0.0, "{ctx}: normal points inward {:?}", hit.normal);
                }
                (None, None) => misses += 1,
                (Some(hit), None) => {
                    // Tangential grazes the march can step over are fine.
                    let depth = sdf(&collider, center, hit.point);
                    assert!(depth.abs() < 1e-3 && hit.t <= max_dist, "{ctx}: phantom hit at t {}", hit.t);
                    hits += 1;
                }
                (None, Some(entry)) => {
                    // Only a sliver graze (entry right at max_dist or barely inside) may be missed.
                    let deepest = (0..=20)
                        .map(|i| sdf(&collider, center, ray.at((entry + i as f32 * 0.002).min(max_dist))))
                        .fold(f32::INFINITY, f32::min);
                    assert!(deepest > -2e-3, "{ctx}: missed a hit at t {entry} (depth {deepest})");
                    misses += 1;
                }
            }
        }
    }
    assert!(hits > 400 && misses > 400, "generator too one-sided: {hits} hits / {misses} misses");
}

#[test]
fn raycast_picks_the_nearest_of_many_colliders() {
    let mut rng = XorShift(0xD15C);
    for case in 0..200 {
        let mut world = World::new();
        let mut shapes = Vec::new();
        for _ in 0..6 {
            let collider = random_collider(&mut rng);
            let center = rng.vec(-6.0, 6.0);
            let e = world.spawn();
            world.add(e, Transform::from_position(center));
            world.add(e, collider.clone());
            shapes.push((e, collider, center));
        }
        let origin = rng.vec(-10.0, 10.0);
        if shapes.iter().any(|(_, c, p)| sdf(c, *p, origin) <= 0.05) {
            continue;
        }
        let ray = Ray::new(origin, (rng.vec(-6.0, 6.0) - origin).normalize());
        let expected = shapes
            .iter()
            .filter_map(|(e, c, p)| marched_entry(c, *p, &ray, 30.0).map(|t| (t, *e)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let got = raycast(&world, &ray, 30.0);
        match (got, expected) {
            (Some(h), Some((t, e))) => {
                assert!((h.t - t).abs() < 0.01, "case {case}: t {} vs {t}", h.t);
                if (h.t - t).abs() < 0.005 && h.entity != e {
                    // Near-tie between two shapes; either is acceptable.
                } else {
                    assert_eq!(h.entity, e, "case {case}: wrong entity");
                }
            }
            (None, None) => {}
            (g, x) => {
                let graze = g.map(|h| h.t).or(x.map(|x| x.0)).unwrap();
                let near_surface = shapes.iter().any(|(_, c, p)| sdf(c, *p, ray.at(graze)).abs() < 2e-3);
                assert!(near_surface, "case {case}: raycast {g:?} vs marched {x:?}");
            }
        }
        let all = raycast_all(&world, &ray, 30.0);
        assert!(all.windows(2).all(|w| w[0].t <= w[1].t), "case {case}: raycast_all not sorted");
    }
}
