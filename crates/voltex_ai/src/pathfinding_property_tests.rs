//! Randomized pathfinding properties on grid navmeshes with random holes:
//! - `find_path` succeeds exactly when start and goal cells are connected
//!   (cell BFS over shared edges is the oracle);
//! - the path starts at `start`, ends at `goal`, and every waypoint is on
//!   the mesh;
//! - the funnel-smoothed path never leaves the walkable area (dense samples
//!   along every segment lie on the mesh).

use std::collections::{HashMap, VecDeque};
use crate::navmesh::{NavMesh, NavTriangle};
use crate::pathfinding::{find_path, find_path_triangles, funnel_smooth};
use voltex_math::Vec3;

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        self.next() as usize % n.max(1)
    }
}

const W: usize = 9;
const H: usize = 7;

/// Walkable cells become two triangles each; adjacency from shared vertex
/// pairs, so it is consistent by construction.
fn build(walk: &[bool]) -> NavMesh {
    let vid = |i: usize, j: usize| j * (W + 1) + i;
    let vertices: Vec<Vec3> = (0..=H)
        .flat_map(|j| (0..=W).map(move |i| Vec3::new(i as f32, 0.0, j as f32)))
        .collect();
    let mut tris: Vec<[usize; 3]> = Vec::new();
    for j in 0..H {
        for i in 0..W {
            if walk[j * W + i] {
                let (a, b, c, d) = (vid(i, j), vid(i + 1, j), vid(i + 1, j + 1), vid(i, j + 1));
                tris.push([a, b, c]);
                tris.push([a, c, d]);
            }
        }
    }
    let mut edge_owner: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (t, idx) in tris.iter().enumerate() {
        for k in 0..3 {
            let (p, q) = (idx[k], idx[(k + 1) % 3]);
            edge_owner.entry((p.min(q), p.max(q))).or_default().push(t);
        }
    }
    let triangles = tris
        .iter()
        .enumerate()
        .map(|(t, idx)| {
            let mut neighbors = [None; 3];
            for (k, n) in neighbors.iter_mut().enumerate() {
                let (p, q) = (idx[k], idx[(k + 1) % 3]);
                *n = edge_owner[&(p.min(q), p.max(q))].iter().copied().find(|&o| o != t);
            }
            NavTriangle { indices: *idx, neighbors }
        })
        .collect();
    NavMesh::new(vertices, triangles)
}

fn connected(walk: &[bool], from: usize, to: usize) -> bool {
    let mut seen = [false; W * H];
    let mut q = VecDeque::from([from]);
    seen[from] = true;
    while let Some(c) = q.pop_front() {
        if c == to {
            return true;
        }
        let (i, j) = (c % W, c / W);
        let mut nb = Vec::new();
        if i > 0 { nb.push(c - 1) }
        if i + 1 < W { nb.push(c + 1) }
        if j > 0 { nb.push(c - W) }
        if j + 1 < H { nb.push(c + W) }
        for n in nb {
            if walk[n] && !seen[n] {
                seen[n] = true;
                q.push_back(n);
            }
        }
    }
    false
}

fn on_mesh(mesh: &NavMesh, p: Vec3) -> bool {
    // Points exactly on a shared boundary may round outside both triangles;
    // accept a point if it or a tiny nudge lands on the mesh.
    const E: f32 = 1e-3;
    [(0.0, 0.0), (E, 0.0), (-E, 0.0), (0.0, E), (0.0, -E)]
        .iter()
        .any(|(dx, dz)| mesh.find_triangle(Vec3::new(p.x + dx, p.y, p.z + dz)).is_some())
}

#[test]
fn pathfinding_agrees_with_connectivity_and_stays_on_the_mesh() {
    let mut paths_found = 0;
    let mut unreachable = 0;
    for seed in 1u32..=40 {
        let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9));
        let walk: Vec<bool> = (0..W * H).map(|_| rng.below(100) >= 28).collect();
        let mesh = build(&walk);
        let cells: Vec<usize> = (0..W * H).filter(|&c| walk[c]).collect();
        if cells.len() < 2 {
            continue;
        }
        for _ in 0..12 {
            let (a, b) = (cells[rng.below(cells.len())], cells[rng.below(cells.len())]);
            let centre = |c: usize| Vec3::new((c % W) as f32 + 0.37, 0.0, (c / W) as f32 + 0.61);
            let (start, goal) = (centre(a), centre(b));
            let ctx = format!("seed {seed} {a}->{b}");
            match find_path(&mesh, start, goal) {
                None => {
                    assert!(!connected(&walk, a, b), "{ctx}: no path but cells are connected");
                    unreachable += 1;
                }
                Some(path) => {
                    assert!(connected(&walk, a, b), "{ctx}: path across disconnected cells");
                    assert_eq!(path.first(), Some(&start), "{ctx}");
                    assert_eq!(path.last(), Some(&goal), "{ctx}");
                    assert!(path.iter().all(|p| on_mesh(&mesh, *p)), "{ctx}: waypoint off mesh");

                    let tris = find_path_triangles(&mesh, start, goal).unwrap();
                    let smooth = funnel_smooth(&tris, &mesh, start, goal);
                    assert_eq!(smooth.first(), Some(&start), "{ctx}: funnel start");
                    assert_eq!(smooth.last(), Some(&goal), "{ctx}: funnel end");
                    for seg in smooth.windows(2) {
                        for k in 1..40 {
                            let t = k as f32 / 40.0;
                            let p = seg[0] + (seg[1] - seg[0]) * t;
                            assert!(on_mesh(&mesh, p), "{ctx}: smoothed path leaves the mesh at {p:?}");
                        }
                    }
                    paths_found += 1;
                }
            }
        }
    }
    // The generator must exercise both outcomes.
    assert!(paths_found > 50 && unreachable > 5, "found {paths_found}, unreachable {unreachable}");
}
