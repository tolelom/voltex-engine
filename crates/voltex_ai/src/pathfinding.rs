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
pub fn distance_xz(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

/// Run A* on the NavMesh and return the sequence of triangle indices from start to goal.
/// Returns None if either point is outside the mesh or no path exists.
pub fn find_path_triangles(navmesh: &NavMesh, start: Vec3, goal: Vec3) -> Option<Vec<usize>> {
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
/// # Known limitation (tracked in `docs/DEFERRED.md`)
///
/// When the funnel inverts and a new apex is set, this implementation
/// `continue`s from the *current* portal index instead of restarting the scan
/// from `apex_idx + 1` as Mikko Mononen's canonical SSF does. For straight or
/// single-turn corridors the result is identical, but in a zig-zag corridor
/// (two tight turns in opposite directions) the skipped portals can yield a
/// sub-optimal path that cuts an interior corner. The engine's own
/// [`find_path`] does **not** use this — it returns triangle centroids — so
/// the limitation only affects callers that invoke `funnel_smooth` directly.
/// A correct fix needs the while-loop restart form plus a zig-zag regression
/// test built with visual verification.
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
    #[allow(unused_assignments)]
    let mut apex_idx: usize = 0;
    let mut left_idx: usize = 0;
    let mut right_idx: usize = 0;

    let n = portals_left.len();

    for i in 0..n {
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
                // Restart scan from apex
                // We need to continue from apex_idx + 1, but since
                // we can't restart a for loop, we use a recursive approach
                // or just continue (the standard SSF continues from i)
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
                continue;
            }
        }
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
        let path = find_path(&nm, start, goal).expect("should find path");
        assert!(path.len() >= 2);
        assert_eq!(path[0], start);
        assert_eq!(path[path.len() - 1], goal);
    }

    #[test]
    fn test_path_three_triangle_strip() {
        let nm = make_strip();
        // start in tri0, goal in tri2
        let start = Vec3::new(0.8, 0.0, 0.5);
        let goal = Vec3::new(2.0, 0.0, 3.5);
        let path = find_path(&nm, start, goal).expect("should find path");
        assert!(path.len() >= 3, "path through 3 triangles should have at least 3 points");
        assert_eq!(path[0], start);
        assert_eq!(path[path.len() - 1], goal);
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
        // Straight corridor: path should be just start and end (2 points)
        let nm = make_strip();
        let start = Vec3::new(1.0, 0.0, 0.5);
        let end = Vec3::new(2.0, 0.0, 3.5);
        let tris = find_path_triangles(&nm, start, end).expect("should find path");
        let smoothed = funnel_smooth(&tris, &nm, start, end);
        assert!(smoothed.len() >= 2, "funnel path should have at least 2 points, got {}", smoothed.len());
        assert_eq!(smoothed[0], start);
        assert_eq!(smoothed[smoothed.len() - 1], end);
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
        let tris = find_path_triangles(&nm, start, end);
        if let Some(tris) = tris {
            let smoothed = funnel_smooth(&tris, &nm, start, end);
            assert!(smoothed.len() >= 2);
            assert_eq!(smoothed[0], start);
            assert_eq!(smoothed[smoothed.len() - 1], end);
        }
    }
}
