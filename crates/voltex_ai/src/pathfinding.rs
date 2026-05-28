//! A* pathfinding over a [`NavMesh`]. Returns waypoint vectors threaded
//! through triangle centroids; consumers typically run a funnel / string-pulling
//! step afterwards to straighten the corridor.

use std::collections::BinaryHeap;
use std::cmp::Ordering;
use voltex_math::Vec3;
use crate::navmesh::NavMesh;

/// Node for A* priority queue (min-heap by f_cost, then g_cost).
#[derive(Debug, Clone)]
struct AStarNode {
    tri_idx: usize,
    g_cost: f32,
    f_cost: f32,
    parent: Option<usize>,
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool {
        self.f_cost == other.f_cost && self.g_cost == other.g_cost
    }
}

impl Eq for AStarNode {}

// BinaryHeap is a max-heap; we negate costs to get min-heap behavior.
impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering: lower f_cost = higher priority
        other.f_cost.partial_cmp(&self.f_cost)
            .unwrap_or(Ordering::Equal)
            .then_with(|| other.g_cost.partial_cmp(&self.g_cost).unwrap_or(Ordering::Equal))
    }
}

/// XZ distance between two Vec3 points (ignoring Y).
pub(crate) fn distance_xz(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

/// Run A* on the NavMesh and return the sequence of triangle indices from start to goal.
/// Returns None if either point is outside the mesh or no path exists.
pub(crate) fn find_path_triangles(navmesh: &NavMesh, start: Vec3, goal: Vec3) -> Option<Vec<usize>> {
    let start_tri = navmesh.find_triangle(start)?;
    let goal_tri = navmesh.find_triangle(goal)?;

    if start_tri == goal_tri {
        return Some(vec![start_tri]);
    }

    let n = navmesh.triangles.len();
    let mut g_costs = vec![f32::INFINITY; n];
    let mut parents: Vec<Option<usize>> = vec![None; n];
    let mut visited = vec![false; n];

    let goal_center = navmesh.triangle_center(goal_tri);
    g_costs[start_tri] = 0.0;
    let start_center = navmesh.triangle_center(start_tri);
    let h = distance_xz(start_center, goal_center);

    let mut open = BinaryHeap::new();
    open.push(AStarNode {
        tri_idx: start_tri,
        g_cost: 0.0,
        f_cost: h,
        parent: None,
    });

    while let Some(node) = open.pop() {
        let idx = node.tri_idx;

        if visited[idx] {
            continue;
        }
        visited[idx] = true;
        parents[idx] = node.parent;

        if idx == goal_tri {
            let mut tri_path = Vec::new();
            let mut cur = idx;
            loop {
                tri_path.push(cur);
                match parents[cur] {
                    Some(p) => cur = p,
                    None => break,
                }
            }
            tri_path.reverse();
            return Some(tri_path);
        }

        let tri = &navmesh.triangles[idx];
        let current_center = navmesh.triangle_center(idx);

        for neighbor_opt in &tri.neighbors {
            if let Some(nb_idx) = *neighbor_opt {
                if visited[nb_idx] {
                    continue;
                }
                let nb_center = navmesh.triangle_center(nb_idx);
                let tentative_g = g_costs[idx] + distance_xz(current_center, nb_center);
                if tentative_g < g_costs[nb_idx] {
                    g_costs[nb_idx] = tentative_g;
                    let h_nb = distance_xz(nb_center, goal_center);
                    open.push(AStarNode {
                        tri_idx: nb_idx,
                        g_cost: tentative_g,
                        f_cost: tentative_g + h_nb,
                        parent: Some(idx),
                    });
                }
            }
        }
    }

    None
}

/// Find a path from `start` to `goal` on the given NavMesh using A*.
///
/// Returns `Some(path)` where `path[0] == start`, the last element equals
/// `goal`, and intermediate points are triangle centers. Returns `None` if
/// either point is outside the mesh or no path exists.
pub fn find_path(navmesh: &NavMesh, start: Vec3, goal: Vec3) -> Option<Vec<Vec3>> {
    let tri_path = find_path_triangles(navmesh, start, goal)?;

    if tri_path.len() == 1 {
        return Some(vec![start, goal]);
    }

    let mut path = Vec::new();
    path.push(start);
    for &ti in &tri_path[1..tri_path.len() - 1] {
        path.push(navmesh.triangle_center(ti));
    }
    path.push(goal);
    Some(path)
}

/// 2D cross product on XZ plane: (b - a) x (c - a) projected onto Y.
fn cross_xz(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    (b.x - a.x) * (c.z - a.z) - (b.z - a.z) * (c.x - a.x)
}

/// Simple Stupid Funnel (SSF) algorithm for string-pulling a triangle corridor path.
///
/// Given the sequence of triangle indices from A*, produces a path with
/// waypoints at portal edge corners where the path turns.
///
/// # Algorithm note (apex restart)
///
/// When the funnel inverts and a new apex is set, the scan is restarted from
/// the portal *after* the new apex (`i = apex_idx`, then the loop's own `i += 1`
/// resumes at `apex_idx + 1`), exactly as Mikko Mononen's canonical SSF does.
/// An earlier version `continue`d from the current portal index; that skipped
/// portals between the old and new apex and, in zig-zag corridors, cut interior
/// corners, producing paths that left the navigable corridor (verified: up to
/// ~0.41 units of penetration on a 3-band zig-zag — see
/// `test_funnel_zigzag_stays_inside_corridor`). The restart form fixes this.
pub fn funnel_smooth(path_triangles: &[usize], navmesh: &NavMesh, start: Vec3, end: Vec3) -> Vec<Vec3> {
    // Trivial cases
    if path_triangles.is_empty() {
        return vec![start, end];
    }
    if path_triangles.len() == 1 {
        return vec![start, end];
    }

    // Build portal list: shared edges between consecutive triangles
    let mut portals_left = Vec::new();
    let mut portals_right = Vec::new();

    for i in 0..path_triangles.len() - 1 {
        let from = path_triangles[i];
        let to = path_triangles[i + 1];
        if let Some((left, right)) = navmesh.shared_edge(from, to) {
            portals_left.push(left);
            portals_right.push(right);
        }
    }

    // Add end point as the final portal (degenerate portal: both sides = end)
    portals_left.push(end);
    portals_right.push(end);

    let mut path = vec![start];

    let mut apex = start;
    let mut left = start;
    let mut right = start;
    // Initialized to 0 but always reassigned (from left_idx/right_idx) before
    // being read via `i = apex_idx`; the initial value is intentionally unused.
    #[allow(unused_assignments)]
    let mut apex_idx: usize = 0;
    let mut left_idx: usize = 0;
    let mut right_idx: usize = 0;

    let n = portals_left.len();

    // `while` loop (not `for`) so the scan can be restarted at the apex index
    // when the funnel inverts, matching Mononen's canonical SSF.
    let mut i = 0;
    while i < n {
        let pl = portals_left[i];
        let pr = portals_right[i];

        // Update right vertex
        if cross_xz(apex, right, pr) <= 0.0 {
            if apex == right || cross_xz(apex, left, pr) > 0.0 {
                // Tighten the funnel
                right = pr;
                right_idx = i;
            } else {
                // Right over left: left becomes new apex
                if left != apex {
                    path.push(left);
                }
                apex = left;
                apex_idx = left_idx;
                left = apex;
                right = apex;
                left_idx = apex_idx;
                right_idx = apex_idx;
                // Restart the scan from the new apex. The `i += 1` below then
                // resumes at apex_idx + 1 (canonical Mononen SSF).
                i = apex_idx;
                i += 1;
                continue;
            }
        }

        // Update left vertex
        if cross_xz(apex, left, pl) >= 0.0 {
            if apex == left || cross_xz(apex, right, pl) < 0.0 {
                // Tighten the funnel
                left = pl;
                left_idx = i;
            } else {
                // Left over right: right becomes new apex
                if right != apex {
                    path.push(right);
                }
                apex = right;
                apex_idx = right_idx;
                left = apex;
                right = apex;
                left_idx = apex_idx;
                right_idx = apex_idx;
                i = apex_idx;
                i += 1;
                continue;
            }
        }

        i += 1;
    }

    // Add end point if not already there
    if let Some(&last) = path.last() {
        if (last.x - end.x).abs() > 1e-6 || (last.z - end.z).abs() > 1e-6 {
            path.push(end);
        }
    } else {
        path.push(end);
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navmesh::{NavMesh, NavTriangle};

    /// Build a 3-triangle strip along Z axis:
    /// v0=(0,0,0), v1=(2,0,0), v2=(0,0,2), v3=(2,0,2), v4=(0,0,4), v5=(2,0,4)
    /// Tri0: v0,v1,v2 — Tri1: v1,v3,v2 — Tri2: v2,v3,v4... wait, need a strip
    ///
    /// Simpler strip:
    /// Tri0: (0,0,0),(2,0,0),(1,0,2)  center ~(1,0,0.67)
    /// Tri1: (2,0,0),(3,0,2),(1,0,2)  center ~(2,0,1.33)
    /// Tri2: (1,0,2),(3,0,2),(2,0,4)  center ~(2,0,2.67)
    fn make_strip() -> NavMesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0), // 0
            Vec3::new(2.0, 0.0, 0.0), // 1
            Vec3::new(1.0, 0.0, 2.0), // 2
            Vec3::new(3.0, 0.0, 2.0), // 3
            Vec3::new(2.0, 0.0, 4.0), // 4
        ];
        let triangles = vec![
            NavTriangle { indices: [0, 1, 2], neighbors: [None, Some(1), None] },
            NavTriangle { indices: [1, 3, 2], neighbors: [None, Some(2), Some(0)] },
            NavTriangle { indices: [2, 3, 4], neighbors: [Some(1), None, None] },
        ];
        NavMesh::new(vertices, triangles)
    }

    fn make_disconnected() -> NavMesh {
        // Two separate triangles not connected
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 2.0),
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(12.0, 0.0, 10.0),
            Vec3::new(11.0, 0.0, 12.0),
        ];
        let triangles = vec![
            NavTriangle { indices: [0, 1, 2], neighbors: [None, None, None] },
            NavTriangle { indices: [3, 4, 5], neighbors: [None, None, None] },
        ];
        NavMesh::new(vertices, triangles)
    }

    #[test]
    fn test_path_same_triangle() {
        let nm = make_strip();
        let start = Vec3::new(0.5, 0.0, 0.5);
        let goal = Vec3::new(1.5, 0.0, 0.5);
        let path = find_path(&nm, start, goal).expect("should find path");
        assert_eq!(path.len(), 2);
        assert_eq!(path[0], start);
        assert_eq!(path[path.len() - 1], goal);
    }

    #[test]
    fn test_path_adjacent_triangles() {
        let nm = make_strip();
        // start in tri0, goal in tri1
        let start = Vec3::new(0.8, 0.0, 0.5);
        let goal = Vec3::new(2.5, 0.0, 1.5);

        // Exact triangle sequence: tri0 -> tri1 (adjacent).
        let tris = find_path_triangles(&nm, start, goal).expect("should find tri path");
        assert_eq!(tris, vec![0, 1], "expected exactly tri0 -> tri1");

        let path = find_path(&nm, start, goal).expect("should find path");
        // 2 adjacent triangles => start + goal only (centroid of intermediate tris
        // is empty because tri_path[1..len-1] is empty for len 2).
        assert_eq!(path.len(), 2);
        assert_eq!(path[0], start);
        assert_eq!(path[path.len() - 1], goal);
    }

    #[test]
    fn test_path_three_triangle_strip() {
        let nm = make_strip();
        // start in tri0, goal in tri2
        let start = Vec3::new(0.8, 0.0, 0.5);
        let goal = Vec3::new(2.0, 0.0, 3.5);

        // Exact triangle sequence: tri0 -> tri1 -> tri2.
        let tris = find_path_triangles(&nm, start, goal).expect("should find tri path");
        assert_eq!(tris, vec![0, 1, 2], "expected exactly tri0 -> tri1 -> tri2");

        let path = find_path(&nm, start, goal).expect("should find path");
        // 3 triangles => start + centroid(tri1) + goal == 3 points.
        assert_eq!(path.len(), 3, "path through 3 triangles should have 3 points");
        assert_eq!(path[0], start);
        // Middle waypoint is the centroid of the intermediate triangle (tri1).
        let mid = nm.triangle_center(1);
        assert!((path[1].x - mid.x).abs() < 1e-5 && (path[1].z - mid.z).abs() < 1e-5,
            "intermediate waypoint should be tri1 centroid, got {:?}", path[1]);
        assert_eq!(path[path.len() - 1], goal);

        // Total length must be within tolerance of the threaded centroid distance.
        let expected = distance_xz(start, mid) + distance_xz(mid, goal);
        let actual: f32 = path.windows(2).map(|w| distance_xz(w[0], w[1])).sum();
        assert!((actual - expected).abs() < 1e-4,
            "path length {actual} should match centroid-threaded distance {expected}");
    }

    #[test]
    fn test_path_outside_returns_none() {
        let nm = make_strip();
        // start is outside the mesh
        let start = Vec3::new(100.0, 0.0, 100.0);
        let goal = Vec3::new(0.8, 0.0, 0.5);
        let result = find_path(&nm, start, goal);
        assert!(result.is_none());
    }

    #[test]
    fn test_path_disconnected_returns_none() {
        let nm = make_disconnected();
        let start = Vec3::new(0.8, 0.0, 0.5);
        let goal = Vec3::new(11.0, 0.0, 10.5);
        let result = find_path(&nm, start, goal);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_path_triangles_same() {
        let nm = make_strip();
        let start = Vec3::new(0.5, 0.0, 0.5);
        let goal = Vec3::new(1.5, 0.0, 0.5);
        let tris = find_path_triangles(&nm, start, goal).expect("should find path");
        assert_eq!(tris.len(), 1);
    }

    #[test]
    fn test_find_path_triangles_strip() {
        let nm = make_strip();
        let start = Vec3::new(0.8, 0.0, 0.5);
        let goal = Vec3::new(2.0, 0.0, 3.5);
        let tris = find_path_triangles(&nm, start, goal).expect("should find path");
        assert_eq!(tris.len(), 3);
        assert_eq!(tris[0], 0);
        assert_eq!(tris[2], 2);
    }

    #[test]
    fn test_funnel_straight_path() {
        // Straight corridor: funnel must NOT introduce spurious corners.
        let nm = make_strip();
        let start = Vec3::new(1.0, 0.0, 0.5);
        let end = Vec3::new(2.0, 0.0, 3.5);
        let tris = find_path_triangles(&nm, start, end).expect("should find path");
        let smoothed = funnel_smooth(&tris, &nm, start, end);
        assert_eq!(smoothed.len(), 2,
            "straight corridor must produce exactly start+end, got {:?}", smoothed);
        assert_eq!(smoothed[0], start);
        assert_eq!(smoothed[1], end);
    }

    #[test]
    fn test_funnel_same_triangle() {
        let nm = make_strip();
        let start = Vec3::new(0.5, 0.0, 0.5);
        let end = Vec3::new(1.5, 0.0, 0.5);
        let smoothed = funnel_smooth(&[0], &nm, start, end);
        assert_eq!(smoothed.len(), 2);
        assert_eq!(smoothed[0], start);
        assert_eq!(smoothed[1], end);
    }

    #[test]
    fn test_funnel_l_shaped_path() {
        // Build an L-shaped navmesh to force a turn
        // Tri0: (0,0,0),(2,0,0),(0,0,2) - bottom left
        // Tri1: (2,0,0),(2,0,2),(0,0,2) - top right of first square
        // Tri2: (2,0,0),(4,0,0),(2,0,2) - extends right
        // Tri3: (2,0,2),(4,0,0),(4,0,2) - top right
        // Tri4: (2,0,2),(4,0,2),(2,0,4) - goes up
        // This makes an L shape going right then up
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0), // 0
            Vec3::new(2.0, 0.0, 0.0), // 1
            Vec3::new(0.0, 0.0, 2.0), // 2
            Vec3::new(2.0, 0.0, 2.0), // 3
            Vec3::new(4.0, 0.0, 0.0), // 4
            Vec3::new(4.0, 0.0, 2.0), // 5
            Vec3::new(2.0, 0.0, 4.0), // 6
        ];
        let triangles = vec![
            NavTriangle { indices: [0, 1, 2], neighbors: [Some(1), None, None] }, // 0
            NavTriangle { indices: [1, 3, 2], neighbors: [Some(2), None, Some(0)] }, // 1
            NavTriangle { indices: [1, 4, 3], neighbors: [None, Some(3), Some(1)] }, // 2
            NavTriangle { indices: [4, 5, 3], neighbors: [None, None, Some(2)] },  // 3 -- not used in L path
            NavTriangle { indices: [3, 5, 6], neighbors: [None, None, None] }, // 4
        ];
        let nm = NavMesh::new(vertices, triangles);

        let start = Vec3::new(0.3, 0.0, 0.3);
        let end = Vec3::new(3.5, 0.0, 0.5);

        // A path MUST exist (tri0 -> tri1 -> tri2). The old test wrapped the
        // assertions in `if let Some`, so it silently passed when no path was
        // found; require the path explicitly.
        let tris = find_path_triangles(&nm, start, end).expect("path exists");
        assert_eq!(tris, vec![0, 1, 2], "expected tri0 -> tri1 -> tri2");

        let smoothed = funnel_smooth(&tris, &nm, start, end);
        assert_eq!(smoothed[0], start);
        assert_eq!(smoothed[smoothed.len() - 1], end);

        // start (0.3,0.3) and end (3.5,0.5) both sit near z=0; the taut string
        // travelling right wraps the single inside corner at vertex 1 = (2,0,0).
        // The path must therefore turn exactly once, at that corner.
        assert_eq!(smoothed.len(), 3, "L-path should turn once, got {smoothed:?}");
        let corner = Vec3::new(2.0, 0.0, 0.0);
        assert!(
            (smoothed[1].x - corner.x).abs() < 1e-4 && (smoothed[1].z - corner.z).abs() < 1e-4,
            "expected turn at inside corner {corner:?}, got {:?}",
            smoothed[1]
        );

        // The turned path must stay inside the corridor.
        let pen = max_penetration(&nm, &smoothed);
        assert!(pen < 1e-3, "L-path left the corridor by {pen} units: {smoothed:?}");
    }

    /// Build a zig-zag corridor as a triangle strip and return
    /// `(navmesh, start, end)`. The corridor weaves so the taut path must touch
    /// an interior corner; a naive funnel that fails to restart the scan at the
    /// apex cuts that corner and leaves the corridor.
    fn make_zigzag() -> (NavMesh, Vec3, Vec3) {
        // Left-wall (l) and right-wall (r) chains. Coordinates chosen (via a
        // randomized search arbitrated by a max-penetration oracle) so the
        // pre-fix `continue`-from-i funnel produced a path ~0.41 units outside
        // the corridor, while the apex-restart funnel stays inside.
        let l = [
            Vec3::new(-2.8217, 0.0, 0.0000),
            Vec3::new(-3.7173, 0.0, 1.3245),
            Vec3::new(-4.9518, 0.0, 4.1572),
            Vec3::new(-4.4156, 0.0, 5.8779),
        ];
        let r = [
            Vec3::new(-0.2940, 0.0, 0.0000),
            Vec3::new(-2.3601, 0.0, 1.3245),
            Vec3::new(-2.8620, 0.0, 4.1572),
            Vec3::new(-0.4660, 0.0, 5.8779),
        ];
        let bands = l.len() - 1;
        let mut vertices = Vec::new();
        for k in 0..l.len() {
            vertices.push(l[k]);
            vertices.push(r[k]);
        }
        let li = |k: usize| k * 2;
        let ri = |k: usize| k * 2 + 1;
        let mut triangles: Vec<NavTriangle> = Vec::new();
        for k in 0..bands {
            let ta = triangles.len();
            let tb = ta + 1;
            let prev_tb = if k == 0 { None } else { Some(ta - 1) };
            let next_ta = if k == bands - 1 { None } else { Some(tb + 1) };
            // Ta = (L_k, R_k, L_{k+1}); edge1 (R_k, L_{k+1}) -> Tb
            triangles.push(NavTriangle {
                indices: [li(k), ri(k), li(k + 1)],
                neighbors: [prev_tb, Some(tb), None],
            });
            // Tb = (R_k, R_{k+1}, L_{k+1}); edge1 (R_{k+1}, L_{k+1}) -> next Ta; edge2 -> Ta
            triangles.push(NavTriangle {
                indices: [ri(k), ri(k + 1), li(k + 1)],
                neighbors: [None, next_ta, Some(ta)],
            });
        }
        let nm = NavMesh::new(vertices, triangles);
        let start = Vec3::new(-1.5579, 0.0, 0.01);
        let end = Vec3::new(-2.4408, 0.0, 5.8679);
        (nm, start, end)
    }

    /// Maximum distance any interior path sample lies outside ALL triangles.
    /// Returns 0.0 when the whole path stays inside (or merely touches) the
    /// corridor. Robust to the float-edge noise of a pure inside/outside test.
    fn max_penetration(nm: &NavMesh, path: &[Vec3]) -> f32 {
        fn point_in_tri(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
            let d1 = (p.x - b.x) * (a.z - b.z) - (a.x - b.x) * (p.z - b.z);
            let d2 = (p.x - c.x) * (b.z - c.z) - (b.x - c.x) * (p.z - c.z);
            let d3 = (p.x - a.x) * (c.z - a.z) - (c.x - a.x) * (p.z - a.z);
            let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
            !(has_neg && has_pos)
        }
        fn dist_seg(p: Vec3, a: Vec3, b: Vec3) -> f32 {
            let abx = b.x - a.x;
            let abz = b.z - a.z;
            let denom = abx * abx + abz * abz;
            let t = if denom > 0.0 {
                (((p.x - a.x) * abx + (p.z - a.z) * abz) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let cx = a.x + abx * t;
            let cz = a.z + abz * t;
            ((p.x - cx).powi(2) + (p.z - cz).powi(2)).sqrt()
        }
        let mut worst = 0.0f32;
        for w in path.windows(2) {
            for s in 1..100 {
                let t = s as f32 / 100.0;
                let pt = Vec3::new(
                    w[0].x + (w[1].x - w[0].x) * t,
                    0.0,
                    w[0].z + (w[1].z - w[0].z) * t,
                );
                let mut inside = false;
                let mut best = f32::INFINITY;
                for tri in &nm.triangles {
                    let a = nm.vertices[tri.indices[0]];
                    let b = nm.vertices[tri.indices[1]];
                    let c = nm.vertices[tri.indices[2]];
                    if point_in_tri(pt, a, b, c) {
                        inside = true;
                        break;
                    }
                    let d = dist_seg(pt, a, b).min(dist_seg(pt, b, c)).min(dist_seg(pt, c, a));
                    if d < best {
                        best = d;
                    }
                }
                if !inside && best > worst {
                    worst = best;
                }
            }
        }
        worst
    }

    #[test]
    fn test_funnel_zigzag_stays_inside_corridor() {
        // Regression for the SSF apex-restart bug (docs/DEFERRED.md). The funnel
        // must touch the interior corner at (-2.862, 4.1572) so the path stays
        // inside the zig-zag corridor. The pre-fix `continue`-from-current-index
        // funnel skipped that corner, producing a path ~0.41 units outside.
        let (nm, start, end) = make_zigzag();
        let tris = find_path_triangles(&nm, start, end).expect("path exists");
        assert_eq!(tris, vec![0, 1, 2, 3, 4, 5], "expected the full 6-triangle strip");

        let smoothed = funnel_smooth(&tris, &nm, start, end);
        assert_eq!(smoothed[0], start);
        assert_eq!(smoothed[smoothed.len() - 1], end);

        // Must turn at the interior corner (vertex L2 = (-2.862, 4.1572)).
        let corner = Vec3::new(-2.8620, 0.0, 4.1572);
        let has_corner = smoothed
            .iter()
            .any(|p| (p.x - corner.x).abs() < 1e-3 && (p.z - corner.z).abs() < 1e-3);
        assert!(has_corner, "funnel must touch interior corner {corner:?}, got {smoothed:?}");

        // Decisive oracle: the smoothed path must NOT leave the corridor.
        let pen = max_penetration(&nm, &smoothed);
        assert!(pen < 1e-3, "funnel path left the corridor by {pen} units: {smoothed:?}");
    }
}
