//! Randomized property for the ray-vs-triangle primitive (used by mesh
//! raycasts) and the BVH ray query: hits must agree with an f64
//! plane-plus-barycentric reference at every scale from 1e-3 to 1e3, and
//! `BvhTree::query_ray` must return exactly the brute-force AABB hits.

use voltex_ecs::Entity;
use voltex_math::{Ray, Vec3, AABB};
use voltex_physics::{ray_vs_triangle, BvhTree};

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

type D3 = [f64; 3];
fn d(v: Vec3) -> D3 {
    [v.x as f64, v.y as f64, v.z as f64]
}
fn sub(a: D3, b: D3) -> D3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: D3, b: D3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: D3, b: D3) -> D3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// f64 reference: Some((t, min barycentric)) when the ray's line meets the
/// triangle plane at t >= 0; inside iff min barycentric >= 0.
fn reference(ray: &Ray, v0: Vec3, v1: Vec3, v2: Vec3) -> Option<(f64, f64, f64)> {
    let (o, dir, a, b, c) = (d(ray.origin), d(ray.direction), d(v0), d(v1), d(v2));
    let n = cross(sub(b, a), sub(c, a));
    let denom = dot(n, dir);
    let cos = denom.abs() / (dot(n, n).sqrt() * dot(dir, dir).sqrt());
    if cos < 1e-9 {
        return None;
    }
    let t = dot(n, sub(a, o)) / denom;
    let p = [o[0] + dir[0] * t, o[1] + dir[1] * t, o[2] + dir[2] * t];
    let area = dot(n, n);
    let w0 = dot(cross(sub(b, p), sub(c, p)), n) / area;
    let w1 = dot(cross(sub(c, p), sub(a, p)), n) / area;
    let w2 = 1.0 - w0 - w1;
    Some((t, w0.min(w1).min(w2), cos))
}

#[test]
fn ray_vs_triangle_agrees_with_f64_reference_at_all_scales() {
    let (mut hits, mut misses) = (0, 0);
    let mut rng = XorShift(0x7121_A1E5);
    for case in 0..20_000 {
        let scale = 10f32.powf(rng.range(-3.0, 3.0));
        let center = rng.vec(-1.0, 1.0) * scale;
        let (v0, v1, v2) = (center + rng.vec(-1.0, 1.0) * scale, center + rng.vec(-1.0, 1.0) * scale, center + rng.vec(-1.0, 1.0) * scale);
        let origin = center + rng.vec(-3.0, 3.0) * scale;
        // Aim at a barycentric point that lands inside about half the time.
        let (a, b) = (rng.range(-0.3, 1.0), rng.range(-0.3, 1.0));
        let target = v0 + (v1 - v0) * a + (v2 - v0) * b;
        let ray = Ray::new(origin, (target - origin).normalize());
        let ctx = format!("case {case}: scale {scale} tri {v0:?} {v1:?} {v2:?} ray {ray:?}");

        let got = ray_vs_triangle(&ray, v0, v1, v2);
        let Some((t_ref, inside, cos)) = reference(&ray, v0, v1, v2) else { continue };
        // Skip numerically ambiguous cases: grazing angles, edge hits, origin on the plane.
        if cos < 1e-3 || inside.abs() < 1e-3 || t_ref.abs() < 1e-3 * scale as f64 {
            continue;
        }
        let expect_hit = inside > 0.0 && t_ref > 0.0;
        match got {
            Some((t, n)) => {
                assert!(expect_hit, "{ctx}: phantom hit at t {t} (ref t {t_ref}, bary {inside})");
                assert!(((t as f64) - t_ref).abs() <= 1e-3 * t_ref.max(scale as f64), "{ctx}: t {t} vs {t_ref}");
                assert!((n.length() - 1.0).abs() < 1e-3, "{ctx}: normal {n:?}");
                hits += 1;
            }
            None => {
                assert!(!expect_hit, "{ctx}: missed hit at t {t_ref} (bary {inside}, cos {cos})");
                misses += 1;
            }
        }
    }
    assert!(hits > 3000 && misses > 3000, "generator too one-sided: {hits} / {misses}");
}

#[test]
fn bvh_query_ray_matches_brute_force() {
    let mut rng = XorShift(0xB7B7_0001);
    for layout in 0..100 {
        let entries: Vec<(Entity, AABB)> = (0..20)
            .map(|i| {
                let min = rng.vec(-10.0, 10.0);
                (Entity { id: i, generation: 0 }, AABB::new(min, min + rng.vec(0.1, 3.0)))
            })
            .collect();
        let tree = BvhTree::build(&entries);
        for _ in 0..20 {
            let origin = rng.vec(-15.0, 15.0);
            let ray = Ray::new(origin, (rng.vec(-10.0, 10.0) - origin).normalize());
            let max_t = rng.range(1.0, 40.0);
            let mut got: Vec<u32> = tree.query_ray(&ray, max_t).iter().map(|(e, _)| e.id).collect();
            got.sort();
            let mut want: Vec<u32> = entries
                .iter()
                .filter(|(_, b)| slab(&ray, b).is_some_and(|t| t <= max_t))
                .map(|(e, _)| e.id)
                .collect();
            want.sort();
            assert_eq!(got, want, "layout {layout} ray {ray:?} max {max_t}");
        }
    }
}

/// Independent slab test (entry t clamped to 0 when starting inside).
fn slab(ray: &Ray, b: &AABB) -> Option<f32> {
    let (o, dir) = (d(ray.origin), d(ray.direction));
    let (lo, hi) = (d(b.min), d(b.max));
    let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
    for i in 0..3 {
        if dir[i].abs() < 1e-12 {
            if o[i] < lo[i] || o[i] > hi[i] {
                return None;
            }
        } else {
            let (a, c) = ((lo[i] - o[i]) / dir[i], (hi[i] - o[i]) / dir[i]);
            t0 = t0.max(a.min(c));
            t1 = t1.min(a.max(c));
        }
    }
    (t0 <= t1 && t1 >= 0.0).then_some(t0.max(0.0) as f32)
}
