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
mod tests;
