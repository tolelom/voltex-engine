//! Navmesh files (`.vnav` and the legacy headerless format) are loaded from
//! disk; a corrupted file must be rejected or produce a mesh that
//! pathfinding can query without panicking.

use std::panic::{catch_unwind, AssertUnwindSafe};
use voltex_ai::{deserialize_navmesh, find_path, serialize_navmesh, NavMesh, NavTriangle};
use voltex_math::Vec3;

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn f(&mut self) -> f32 {
        (self.next() % 1000) as f32 / 100.0 - 1.0
    }
}

/// 3x3 grid of quads, two triangles each, with adjacency by shared edges.
fn grid() -> NavMesh {
    let mut vertices = Vec::new();
    for z in 0..4 {
        for x in 0..4 {
            vertices.push(Vec3::new(x as f32 * 3.0, 0.0, z as f32 * 3.0));
        }
    }
    let v = |x: usize, z: usize| z * 4 + x;
    let mut corners = Vec::new();
    for z in 0..3 {
        for x in 0..3 {
            corners.push([v(x, z), v(x + 1, z), v(x + 1, z + 1)]);
            corners.push([v(x, z), v(x + 1, z + 1), v(x, z + 1)]);
        }
    }
    let shares = |t: &[usize; 3], p: usize, q: usize| {
        (0..3).any(|k| {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            (a, b) == (p, q) || (a, b) == (q, p)
        })
    };
    let triangles = corners
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut neighbors = [None; 3];
            for (e, n) in neighbors.iter_mut().enumerate() {
                let (p, q) = (t[e], t[(e + 1) % 3]);
                *n = corners.iter().enumerate().position(|(j, o)| j != i && shares(o, p, q));
            }
            NavTriangle { indices: *t, neighbors }
        })
        .collect();
    NavMesh::new(vertices, triangles)
}

#[test]
fn navmesh_loaders_never_panic_and_loaded_meshes_are_queryable() {
    let mesh = grid();
    let vnav = mesh.serialize();
    let legacy = serialize_navmesh(&mesh);
    let clean = NavMesh::deserialize(&vnav).unwrap();
    assert!(find_path(&clean, Vec3::new(1.0, 0.0, 1.0), Vec3::new(8.0, 0.0, 8.0)).is_some());
    let mut rng = XorShift(0x7A7C_0003);
    for i in 0..20_000u32 {
        let vnav_format = i.is_multiple_of(2);
        let mut b = if vnav_format { vnav.clone() } else { legacy.clone() };
        if rng.next().is_multiple_of(4) {
            b.truncate(rng.next() as usize % b.len());
        } else {
            for _ in 0..1 + rng.next() % 4 {
                let at = rng.next() as usize % b.len();
                b[at] = rng.next() as u8;
            }
        }
        let (start, goal) = (Vec3::new(rng.f(), 0.0, rng.f()), Vec3::new(rng.f(), 0.0, rng.f()));
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let loaded = if vnav_format { NavMesh::deserialize(&b) } else { deserialize_navmesh(&b) };
            if let Ok(m) = loaded {
                let _ = find_path(&m, start, goal);
            }
        }));
        assert!(outcome.is_ok(), "mutation {i} panicked the navmesh loader or find_path");
    }
}
