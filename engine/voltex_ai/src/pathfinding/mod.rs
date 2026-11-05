//! A* pathfinding over a [`NavMesh`]. Returns waypoint vectors threaded
//! through triangle centroids. No funnel / string-pulling step runs on the
//! result; an unwired one lives in [`crate::experimental::funnel`].

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

/// Moved to [`experimental::funnel::funnel_smooth`](crate::experimental::funnel::funnel_smooth).
#[deprecated(note = "moved to `voltex_ai::experimental::funnel::funnel_smooth`; not used by the engine")]
#[inline]
pub fn funnel_smooth(path_triangles: &[usize], navmesh: &NavMesh, start: Vec3, end: Vec3) -> Vec<Vec3> {
    crate::experimental::funnel::funnel_smooth(path_triangles, navmesh, start, end)
}

#[cfg(test)]
mod tests;
