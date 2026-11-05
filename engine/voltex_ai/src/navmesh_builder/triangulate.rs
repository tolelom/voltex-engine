use super::{NavMeshBuilder, RegionMap};
use crate::navmesh::{barycentric_xz, NavMesh, NavTriangle};
use voltex_math::Vec3;

impl NavMeshBuilder {
    /// Steps 4-5 combined: Convert walkable grid cells directly into a NavMesh.
    /// Each walkable cell becomes a quad (2 triangles), with adjacency computed.
    pub(crate) fn triangulate(&self, region_map: &RegionMap) -> NavMesh {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();

        // For each walkable cell, create 4 vertices and 2 triangles.
        // Map from cell (x,z) -> (tri_a_idx, tri_b_idx) for adjacency lookup.
        let n = region_map.grid.cell_count();
        // cell_tri_map[cell_idx] = Some((tri_a_idx, tri_b_idx)) or None
        let mut cell_tri_map: Vec<Option<(usize, usize)>> = vec![None; n];

        for z in 0..region_map.grid.depth {
            for x in 0..region_map.grid.width {
                let idx = region_map.grid.index(x, z);
                if region_map.regions[idx] == 0 {
                    continue;
                }

                let h = region_map.heights[idx];
                let x0 = region_map.grid.min_x + x as f32 * region_map.grid.cell_size;
                let x1 = x0 + region_map.grid.cell_size;
                let z0 = region_map.grid.min_z + z as f32 * region_map.grid.cell_size;
                let z1 = z0 + region_map.grid.cell_size;

                let vi = vertices.len();
                vertices.push(Vec3::new(x0, h, z0)); // vi+0: bottom-left
                vertices.push(Vec3::new(x1, h, z0)); // vi+1: bottom-right
                vertices.push(Vec3::new(x1, h, z1)); // vi+2: top-right
                vertices.push(Vec3::new(x0, h, z1)); // vi+3: top-left

                let ta = triangles.len();
                // Triangle A: bottom-left, bottom-right, top-right (vi+0, vi+1, vi+2)
                triangles.push(NavTriangle {
                    indices: [vi, vi + 1, vi + 2],
                    neighbors: [None, None, None], // filled in later
                });
                // Triangle B: bottom-left, top-right, top-left (vi+0, vi+2, vi+3)
                triangles.push(NavTriangle {
                    indices: [vi, vi + 2, vi + 3],
                    neighbors: [None, None, None],
                });

                // Internal adjacency: A and B share edge (vi+0, vi+2)
                // For A: edge 2 is (vi+2 -> vi+0) — indices[2] to indices[0]
                // For B: edge 0 is (vi+0 -> vi+2) — indices[0] to indices[1]
                triangles[ta].neighbors[2] = Some(ta + 1);     // A's edge2 -> B
                triangles[ta + 1].neighbors[0] = Some(ta);     // B's edge0 -> A

                cell_tri_map[idx] = Some((ta, ta + 1));
            }
        }

        // Now compute inter-cell adjacency
        // Cell layout:
        //   Tri A: (v0, v1, v2) = (BL, BR, TR)
        //     edge 0: BL->BR (bottom edge, connects to cell below z-1)
        //     edge 1: BR->TR (right edge, connects to cell at x+1)
        //     edge 2: TR->BL (diagonal, internal, already connected to B)
        //   Tri B: (v0, v2, v3) = (BL, TR, TL)
        //     edge 0: BL->TR (diagonal, internal, already connected to A)
        //     edge 1: TR->TL (top edge, connects to cell above z+1)
        //     edge 2: TL->BL (left edge, connects to cell at x-1)

        for z in 0..region_map.grid.depth {
            for x in 0..region_map.grid.width {
                let idx = region_map.grid.index(x, z);
                if cell_tri_map[idx].is_none() {
                    continue;
                }
                let (ta, _tb) = cell_tri_map[idx].unwrap();

                // Bottom neighbor (z-1): A's edge 0 connects to neighbor's B edge 1 (TR->TL = top)
                if z > 0 {
                    let ni = (z - 1) * region_map.grid.width + x;
                    if let Some((_, nb_tb)) = cell_tri_map[ni] {
                        triangles[ta].neighbors[0] = Some(nb_tb);
                        triangles[nb_tb].neighbors[1] = Some(ta);
                    }
                }

                // Right neighbor (x+1): A's edge 1 connects to neighbor's B edge 2 (TL->BL = left)
                if x + 1 < region_map.grid.width {
                    let ni = z * region_map.grid.width + (x + 1);
                    if let Some((_, nb_tb)) = cell_tri_map[ni] {
                        triangles[ta].neighbors[1] = Some(nb_tb);
                        triangles[nb_tb].neighbors[2] = Some(ta);
                    }
                }
            }
        }

        NavMesh::new(vertices, triangles)
    }
}

/// Loose point-in-triangle test on XZ plane, with a tolerance margin.
/// Degenerate triangles never contain a point.
pub(super) fn point_in_triangle_xz_loose(point: Vec3, a: Vec3, b: Vec3, c: Vec3, margin: f32) -> bool {
    let Some((u, v, w)) = barycentric_xz(point.x, point.z, a, b, c) else {
        return false;
    };
    let longest_edge = (a - b).length().max((b - c).length()).max((c - a).length()).max(0.001);
    let tolerance = margin / longest_edge;
    u >= -tolerance && v >= -tolerance && w >= -tolerance
}

/// Interpolate Y height at (px, pz) on the plane defined by triangle (a, b, c).
pub(super) fn interpolate_y(a: Vec3, b: Vec3, c: Vec3, px: f32, pz: f32) -> f32 {
    match barycentric_xz(px, pz, a, b, c) {
        Some((u, v, w)) => u * a.y + v * b.y + w * c.y,
        None => (a.y + b.y + c.y) / 3.0,
    }
}
