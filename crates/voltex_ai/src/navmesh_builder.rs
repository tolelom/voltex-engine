use voltex_math::Vec3;
use crate::navmesh::{NavMesh, NavTriangle};

/// Configuration for navmesh generation.
pub struct NavMeshBuilder {
    /// XZ voxel size (default 0.3)
    pub cell_size: f32,
    /// Y voxel size (default 0.2)
    pub cell_height: f32,
    /// Minimum clearance height for walkable areas (default 2.0)
    pub agent_height: f32,
    /// Agent capsule radius used to erode walkable areas (default 0.5)
    pub agent_radius: f32,
    /// Maximum walkable slope angle in degrees (default 45.0)
    pub max_slope: f32,
}

impl Default for NavMeshBuilder {
    fn default() -> Self {
        Self {
            cell_size: 0.3,
            cell_height: 0.2,
            agent_height: 2.0,
            agent_radius: 0.5,
            max_slope: 45.0,
        }
    }
}

/// Heightfield: a 2D grid of min/max height spans for voxelized geometry.
pub struct Heightfield {
    pub width: usize,  // number of cells along X
    pub depth: usize,  // number of cells along Z
    pub min_x: f32,
    pub min_z: f32,
    pub cell_size: f32,
    pub cell_height: f32,
    /// For each cell (x, z), store (min_y, max_y). None if empty.
    pub cells: Vec<Option<(f32, f32)>>,
}

impl Heightfield {
    pub fn cell_index(&self, x: usize, z: usize) -> usize {
        z * self.width + x
    }
}

/// Map of walkable cells. True if the cell is walkable.
pub struct WalkableMap {
    pub width: usize,
    pub depth: usize,
    pub min_x: f32,
    pub min_z: f32,
    pub cell_size: f32,
    pub walkable: Vec<bool>,
    /// Height at each walkable cell (top of walkable surface).
    pub heights: Vec<f32>,
}

impl WalkableMap {
    pub fn cell_index(&self, x: usize, z: usize) -> usize {
        z * self.width + x
    }
}

/// A region of connected walkable cells (flood-fill result).
pub struct RegionMap {
    pub width: usize,
    pub depth: usize,
    pub min_x: f32,
    pub min_z: f32,
    pub cell_size: f32,
    /// Region ID per cell. 0 = not walkable, 1+ = region ID.
    pub regions: Vec<u32>,
    pub heights: Vec<f32>,
    pub num_regions: u32,
}

impl NavMeshBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Step 1: Rasterize triangles into a heightfield grid.
    pub fn voxelize(&self, vertices: &[Vec3], indices: &[u32]) -> Heightfield {
        // Find bounding box
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;

        for v in vertices {
            min_x = min_x.min(v.x);
            max_x = max_x.max(v.x);
            min_y = min_y.min(v.y);
            max_y = max_y.max(v.y);
            min_z = min_z.min(v.z);
            max_z = max_z.max(v.z);
        }

        let width = ((max_x - min_x) / self.cell_size).ceil() as usize + 1;
        let depth = ((max_z - min_z) / self.cell_size).ceil() as usize + 1;
        let mut cells: Vec<Option<(f32, f32)>> = vec![None; width * depth];

        // Rasterize each triangle
        for tri_i in (0..indices.len()).step_by(3) {
            if tri_i + 2 >= indices.len() {
                break;
            }
            let v0 = vertices[indices[tri_i] as usize];
            let v1 = vertices[indices[tri_i + 1] as usize];
            let v2 = vertices[indices[tri_i + 2] as usize];

            // Find bounding box of triangle in grid coords
            let tri_min_x = v0.x.min(v1.x).min(v2.x);
            let tri_max_x = v0.x.max(v1.x).max(v2.x);
            let tri_min_z = v0.z.min(v1.z).min(v2.z);
            let tri_max_z = v0.z.max(v1.z).max(v2.z);

            let gx0 = ((tri_min_x - min_x) / self.cell_size).floor() as isize;
            let gx1 = ((tri_max_x - min_x) / self.cell_size).ceil() as isize;
            let gz0 = ((tri_min_z - min_z) / self.cell_size).floor() as isize;
            let gz1 = ((tri_max_z - min_z) / self.cell_size).ceil() as isize;

            let gx0 = gx0.max(0) as usize;
            let gx1 = (gx1 as usize).min(width - 1);
            let gz0 = gz0.max(0) as usize;
            let gz1 = (gz1 as usize).min(depth - 1);

            for gz in gz0..=gz1 {
                for gx in gx0..=gx1 {
                    let cx = min_x + (gx as f32 + 0.5) * self.cell_size;
                    let cz = min_z + (gz as f32 + 0.5) * self.cell_size;

                    // Check if cell center is inside triangle (XZ)
                    let p = Vec3::new(cx, 0.0, cz);
                    if point_in_triangle_xz_loose(p, v0, v1, v2, self.cell_size * 0.5) {
                        // Interpolate Y at this XZ point
                        let y = interpolate_y(v0, v1, v2, cx, cz);
                        let idx = gz * width + gx;
                        match &mut cells[idx] {
                            Some((ref mut lo, ref mut hi)) => {
                                *lo = lo.min(y);
                                *hi = hi.max(y);
                            }
                            None => {
                                cells[idx] = Some((y, y));
                            }
                        }
                    }
                }
            }
        }

        Heightfield {
            width,
            depth,
            min_x,
            min_z,
            cell_size: self.cell_size,
            cell_height: self.cell_height,
            cells,
        }
    }

    /// Step 2: Mark walkable cells based on slope and agent height clearance.
    pub fn mark_walkable(&self, hf: &Heightfield) -> WalkableMap {
        let max_slope_cos = (self.max_slope * std::f32::consts::PI / 180.0).cos();
        let n = hf.width * hf.depth;
        let mut walkable = vec![false; n];
        let mut heights = vec![0.0f32; n];

        for z in 0..hf.depth {
            for x in 0..hf.width {
                let idx = z * hf.width + x;
                if let Some((_lo, hi)) = hf.cells[idx] {
                    // Check slope by comparing height differences with neighbors
                    let slope_ok = self.check_slope(hf, x, z, max_slope_cos);

                    // Check clearance: for simplicity, if cell has geometry, assume clearance
                    // unless there's a cell above within agent_height (not implemented for simple case)
                    if slope_ok {
                        walkable[idx] = true;
                        heights[idx] = hi;
                    }
                }
            }
        }

        // Erode by agent radius: remove walkable cells too close to non-walkable
        let erosion_cells = (self.agent_radius / hf.cell_size).ceil() as usize;
        if erosion_cells > 0 {
            let mut eroded = walkable.clone();
            for z in 0..hf.depth {
                for x in 0..hf.width {
                    let idx = z * hf.width + x;
                    if !walkable[idx] {
                        continue;
                    }
                    // Check if near boundary of walkable area
                    let mut near_edge = false;
                    for dz in 0..=erosion_cells {
                        for dx in 0..=erosion_cells {
                            if dx == 0 && dz == 0 {
                                continue;
                            }
                            // Check all 4 quadrants
                            let checks: [(isize, isize); 4] = [
                                (dx as isize, dz as isize),
                                (-(dx as isize), dz as isize),
                                (dx as isize, -(dz as isize)),
                                (-(dx as isize), -(dz as isize)),
                            ];
                            for (ddx, ddz) in checks {
                                let nx = x as isize + ddx;
                                let nz = z as isize + ddz;
                                if nx < 0 || nz < 0 || nx >= hf.width as isize || nz >= hf.depth as isize {
                                    near_edge = true;
                                    break;
                                }
                                let ni = nz as usize * hf.width + nx as usize;
                                if !walkable[ni] {
                                    near_edge = true;
                                    break;
                                }
                            }
                            if near_edge {
                                break;
                            }
                        }
                        if near_edge {
                            break;
                        }
                    }
                    if near_edge {
                        eroded[idx] = false;
                    }
                }
            }
            return WalkableMap {
                width: hf.width,
                depth: hf.depth,
                min_x: hf.min_x,
                min_z: hf.min_z,
                cell_size: hf.cell_size,
                walkable: eroded,
                heights,
            };
        }

        WalkableMap {
            width: hf.width,
            depth: hf.depth,
            min_x: hf.min_x,
            min_z: hf.min_z,
            cell_size: hf.cell_size,
            walkable,
            heights,
        }
    }

    /// Check if the slope at cell (x, z) is walkable.
    fn check_slope(&self, hf: &Heightfield, x: usize, z: usize, max_slope_cos: f32) -> bool {
        let idx = z * hf.width + x;
        let h = match hf.cells[idx] {
            Some((_, hi)) => hi,
            None => return false,
        };

        // Compare with direct neighbors to estimate slope
        let neighbors: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        for (dx, dz) in neighbors {
            let nx = x as isize + dx;
            let nz = z as isize + dz;
            if nx < 0 || nz < 0 || nx >= hf.width as isize || nz >= hf.depth as isize {
                continue;
            }
            let ni = nz as usize * hf.width + nx as usize;
            if let Some((_, nh)) = hf.cells[ni] {
                let dy = (nh - h).abs();
                let dist = hf.cell_size;
                // slope angle: atan(dy/dist), check cos of that angle
                let slope_len = (dy * dy + dist * dist).sqrt();
                let cos_angle = dist / slope_len;
                if cos_angle < max_slope_cos {
                    return false;
                }
            }
        }
        true
    }

    /// Step 3: Flood-fill connected walkable areas into regions.
    pub fn build_regions(&self, wm: &WalkableMap) -> RegionMap {
        let n = wm.width * wm.depth;
        let mut regions = vec![0u32; n];
        let mut current_region = 0u32;

        for z in 0..wm.depth {
            for x in 0..wm.width {
                let idx = z * wm.width + x;
                if wm.walkable[idx] && regions[idx] == 0 {
                    current_region += 1;
                    // Flood fill
                    let mut stack = vec![(x, z)];
                    regions[idx] = current_region;
                    while let Some((cx, cz)) = stack.pop() {
                        let neighbors: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                        for (dx, dz) in neighbors {
                            let nx = cx as isize + dx;
                            let nz = cz as isize + dz;
                            if nx < 0 || nz < 0 || nx >= wm.width as isize || nz >= wm.depth as isize {
                                continue;
                            }
                            let ni = nz as usize * wm.width + nx as usize;
                            if wm.walkable[ni] && regions[ni] == 0 {
                                regions[ni] = current_region;
                                stack.push((nx as usize, nz as usize));
                            }
                        }
                    }
                }
            }
        }

        RegionMap {
            width: wm.width,
            depth: wm.depth,
            min_x: wm.min_x,
            min_z: wm.min_z,
            cell_size: wm.cell_size,
            regions,
            heights: wm.heights.clone(),
            num_regions: current_region,
        }
    }

    /// Steps 4-5 combined: Convert walkable grid cells directly into a NavMesh.
    /// Each walkable cell becomes a quad (2 triangles), with adjacency computed.
    pub fn triangulate(&self, rm: &RegionMap) -> NavMesh {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();

        // For each walkable cell, create 4 vertices and 2 triangles.
        // Map from cell (x,z) -> (tri_a_idx, tri_b_idx) for adjacency lookup.
        let n = rm.width * rm.depth;
        // cell_tri_map[cell_idx] = Some((tri_a_idx, tri_b_idx)) or None
        let mut cell_tri_map: Vec<Option<(usize, usize)>> = vec![None; n];

        for z in 0..rm.depth {
            for x in 0..rm.width {
                let idx = z * rm.width + x;
                if rm.regions[idx] == 0 {
                    continue;
                }

                let h = rm.heights[idx];
                let x0 = rm.min_x + x as f32 * rm.cell_size;
                let x1 = x0 + rm.cell_size;
                let z0 = rm.min_z + z as f32 * rm.cell_size;
                let z1 = z0 + rm.cell_size;

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

        for z in 0..rm.depth {
            for x in 0..rm.width {
                let idx = z * rm.width + x;
                if cell_tri_map[idx].is_none() {
                    continue;
                }
                let (ta, tb) = cell_tri_map[idx].unwrap();

                // Bottom neighbor (z-1): A's edge 0 connects to neighbor's B edge 1 (TR->TL = top)
                if z > 0 {
                    let ni = (z - 1) * rm.width + x;
                    if let Some((_, nb_tb)) = cell_tri_map[ni] {
                        triangles[ta].neighbors[0] = Some(nb_tb);
                        triangles[nb_tb].neighbors[1] = Some(ta);
                    }
                }

                // Right neighbor (x+1): A's edge 1 connects to neighbor's B edge 2 (TL->BL = left)
                if x + 1 < rm.width {
                    let ni = z * rm.width + (x + 1);
                    if let Some((_, nb_tb)) = cell_tri_map[ni] {
                        triangles[ta].neighbors[1] = Some(nb_tb);
                        triangles[nb_tb].neighbors[2] = Some(ta);
                    }
                }
            }
        }

        NavMesh::new(vertices, triangles)
    }

    /// Full pipeline: voxelize, mark walkable, build regions, triangulate.
    pub fn build(&self, vertices: &[Vec3], indices: &[u32]) -> NavMesh {
        let hf = self.voxelize(vertices, indices);
        let wm = self.mark_walkable(&hf);
        let rm = self.build_regions(&wm);
        self.triangulate(&rm)
    }
}

/// Loose point-in-triangle test on XZ plane, with a tolerance margin.
fn point_in_triangle_xz_loose(point: Vec3, a: Vec3, b: Vec3, c: Vec3, margin: f32) -> bool {
    let px = point.x;
    let pz = point.z;

    let denom = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
    if denom.abs() < f32::EPSILON {
        // Degenerate: check if point is near the line segment
        return false;
    }

    let u = ((b.z - c.z) * (px - c.x) + (c.x - b.x) * (pz - c.z)) / denom;
    let v = ((c.z - a.z) * (px - c.x) + (a.x - c.x) * (pz - c.z)) / denom;
    let w = 1.0 - u - v;

    let e = margin / ((a - b).length().max((b - c).length()).max((c - a).length()).max(0.001));
    u >= -e && v >= -e && w >= -e
}

/// Interpolate Y height at (px, pz) on the plane defined by triangle (a, b, c).
fn interpolate_y(a: Vec3, b: Vec3, c: Vec3, px: f32, pz: f32) -> f32 {
    let denom = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
    if denom.abs() < f32::EPSILON {
        return (a.y + b.y + c.y) / 3.0;
    }
    let u = ((b.z - c.z) * (px - c.x) + (c.x - b.x) * (pz - c.z)) / denom;
    let v = ((c.z - a.z) * (px - c.x) + (a.x - c.x) * (pz - c.z)) / denom;
    let w = 1.0 - u - v;
    u * a.y + v * b.y + w * c.y
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a simple flat plane (10x10, y=0) as 2 triangles.
    fn flat_plane_geometry() -> (Vec<Vec3>, Vec<u32>) {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, 10.0),
        ];
        let indices = vec![0, 1, 2, 0, 2, 3];
        (vertices, indices)
    }

    /// Create a plane with a box obstacle (hole) in the middle.
    /// The plane is 10x10 with a 2x2 hole at center (4-6, 4-6).
    fn plane_with_obstacle() -> (Vec<Vec3>, Vec<u32>) {
        // Build geometry as a grid of quads, skipping the obstacle region.
        // Use 1.0 unit grid cells for simplicity.
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // 10x10 grid of 1x1 quads, skip 4<=x<6 && 4<=z<6
        for z in 0..10 {
            for x in 0..10 {
                if x >= 4 && x < 6 && z >= 4 && z < 6 {
                    continue; // obstacle
                }
                let vi = vertices.len() as u32;
                let fx = x as f32;
                let fz = z as f32;
                vertices.push(Vec3::new(fx, 0.0, fz));
                vertices.push(Vec3::new(fx + 1.0, 0.0, fz));
                vertices.push(Vec3::new(fx + 1.0, 0.0, fz + 1.0));
                vertices.push(Vec3::new(fx, 0.0, fz + 1.0));
                indices.push(vi);
                indices.push(vi + 1);
                indices.push(vi + 2);
                indices.push(vi);
                indices.push(vi + 2);
                indices.push(vi + 3);
            }
        }

        (vertices, indices)
    }

    #[test]
    fn test_voxelize_flat_plane() {
        let builder = NavMeshBuilder {
            cell_size: 1.0,
            cell_height: 0.2,
            agent_height: 2.0,
            agent_radius: 0.0, // no erosion for simple test
            max_slope: 45.0,
        };
        let (verts, idxs) = flat_plane_geometry();
        let hf = builder.voxelize(&verts, &idxs);

        // Should have cells covering the 10x10 area
        assert!(hf.width > 0);
        assert!(hf.depth > 0);

        // Check that some cells are populated
        let populated = hf.cells.iter().filter(|c| c.is_some()).count();
        assert!(populated > 0, "some cells should be populated");
    }

    #[test]
    fn test_flat_plane_single_region() {
        let builder = NavMeshBuilder {
            cell_size: 1.0,
            cell_height: 0.2,
            agent_height: 2.0,
            agent_radius: 0.0,
            max_slope: 45.0,
        };
        let (verts, idxs) = flat_plane_geometry();
        let hf = builder.voxelize(&verts, &idxs);
        let wm = builder.mark_walkable(&hf);
        let rm = builder.build_regions(&wm);

        assert_eq!(rm.num_regions, 1, "flat plane should be a single region");
    }

    #[test]
    fn test_flat_plane_builds_navmesh() {
        let builder = NavMeshBuilder {
            cell_size: 1.0,
            cell_height: 0.2,
            agent_height: 2.0,
            agent_radius: 0.0,
            max_slope: 45.0,
        };
        let (verts, idxs) = flat_plane_geometry();
        let nm = builder.build(&verts, &idxs);

        assert!(!nm.vertices.is_empty(), "navmesh should have vertices");
        assert!(!nm.triangles.is_empty(), "navmesh should have triangles");

        // Should be able to find a triangle at center of the plane
        let center = Vec3::new(5.0, 0.0, 5.0);
        assert!(nm.find_triangle(center).is_some(), "should find triangle at center");
    }

    #[test]
    fn test_obstacle_path_around() {
        let builder = NavMeshBuilder {
            cell_size: 1.0,
            cell_height: 0.2,
            agent_height: 2.0,
            agent_radius: 0.0,
            max_slope: 45.0,
        };
        let (verts, idxs) = plane_with_obstacle();
        let nm = builder.build(&verts, &idxs);

        // Start at (1, 0, 5) and goal at (9, 0, 5) — must go around obstacle
        let start = Vec3::new(1.5, 0.0, 5.5);
        let goal = Vec3::new(8.5, 0.0, 5.5);

        use crate::pathfinding::find_path;
        let path = find_path(&nm, start, goal);
        assert!(path.is_some(), "should find path around obstacle");
    }

    #[test]
    fn test_slope_walkable_unwalkable() {
        // Create a steep ramp: triangle from (0,0,0) to (1,0,0) to (0.5, 5, 1)
        // Slope angle = atan(5/1) ≈ 78.7 degrees — should be unwalkable at max_slope=45
        let builder = NavMeshBuilder {
            cell_size: 0.2,
            cell_height: 0.2,
            agent_height: 2.0,
            agent_radius: 0.0,
            max_slope: 45.0,
        };
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(1.0, 10.0, 2.0),
        ];
        let indices = vec![0, 1, 2];
        let hf = builder.voxelize(&vertices, &indices);
        let wm = builder.mark_walkable(&hf);

        // Most interior cells should be unwalkable due to steep slope
        let walkable_count = wm.walkable.iter().filter(|&&w| w).count();
        // The bottom edge cells might be walkable (flat), but interior should not
        // Just check that not all cells are walkable
        let total_cells = hf.cells.iter().filter(|c| c.is_some()).count();
        assert!(
            walkable_count < total_cells || total_cells <= 1,
            "steep slope should make most cells unwalkable (walkable={}, total={})",
            walkable_count, total_cells
        );
    }

    #[test]
    fn test_navmesh_adjacency() {
        let builder = NavMeshBuilder {
            cell_size: 1.0,
            cell_height: 0.2,
            agent_height: 2.0,
            agent_radius: 0.0,
            max_slope: 45.0,
        };
        let (verts, idxs) = flat_plane_geometry();
        let nm = builder.build(&verts, &idxs);

        // Check that some triangles have neighbors
        let has_neighbors = nm.triangles.iter().any(|t| t.neighbors.iter().any(|n| n.is_some()));
        assert!(has_neighbors, "navmesh triangles should have adjacency");
    }
}
