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

/// Find a path from `start` to `goal` on the given NavMesh using A*.
///
/// Returns Some(path) where path[0] == start, path[last] == goal, and
/// intermediate points are triangle centers. Returns None if either
/// point is outside the mesh or no path exists.
pub fn find_path(navmesh: &NavMesh, start: Vec3, goal: Vec3) -> Option<Vec<Vec3>> {
    let start_tri = navmesh.find_triangle(start)?;
    let goal_tri = navmesh.find_triangle(goal)?;

    // If start and goal are in the same triangle, path is direct.
    if start_tri == goal_tri {
        return Some(vec![start, goal]);
    }

    let n = navmesh.triangles.len();
    // g_cost[i] = best known cost to reach triangle i
    let mut g_costs = vec![f32::INFINITY; n];
    // parent[i] = index of parent triangle in the A* tree
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
            // Reconstruct path
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

            // Convert triangle path to Vec3 waypoints:
            // start point -> intermediate triangle centers -> goal point
            let mut path = Vec::new();
            path.push(start);
            // skip first (start_tri) and last (goal_tri) in intermediate centers
            for &ti in &tri_path[1..tri_path.len() - 1] {
                path.push(navmesh.triangle_center(ti));
            }
            path.push(goal);
            return Some(path);
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

    None // No path found
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
}
