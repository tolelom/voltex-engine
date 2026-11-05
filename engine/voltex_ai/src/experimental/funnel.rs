//! Simple Stupid Funnel (SSF) string pulling: turns the triangle corridor
//! from A* into a path with waypoints only at the corners where it turns.
//!
//! [`find_path`](crate::find_path) does not call it (its waypoints stay at
//! triangle centres), and the triangle corridor it needs comes from the
//! crate-internal `find_path_triangles`.

use voltex_math::Vec3;
use crate::navmesh::NavMesh;

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
