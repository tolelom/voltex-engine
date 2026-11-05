//! Automatic [`NavMesh`] generation from triangle geometry
//! (voxelize → mark walkable → flood-fill regions → triangulate).

mod voxelize;
mod walkable;
mod regions;
mod triangulate;

use voltex_math::Vec3;
use crate::navmesh::NavMesh;

/// Configuration for navmesh generation.
pub struct NavMeshBuilder {
    /// XZ voxel size (default 0.3)
    pub cell_size: f32,
    /// Y voxel size (default 0.2). Not used yet: heights are kept as floats.
    pub cell_height: f32,
    /// Minimum clearance height for walkable areas (default 2.0).
    /// Not used yet: every cell with geometry is assumed to have clearance.
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

/// XZ cell grid shared by every pipeline stage. Cell `(x, z)` covers
/// `[min_x + x·cell_size, +cell_size) × [min_z + z·cell_size, +cell_size)`
/// and is stored at `z * width + x`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GridLayout {
    /// Number of cells along X.
    pub(crate) width: usize,
    /// Number of cells along Z.
    pub(crate) depth: usize,
    pub(crate) min_x: f32,
    pub(crate) min_z: f32,
    pub(crate) cell_size: f32,
}

/// Offsets of the four edge-adjacent cells.
pub(crate) const FOUR_NEIGHBORS: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

impl GridLayout {
    pub(crate) fn cell_count(&self) -> usize {
        self.width * self.depth
    }

    pub(crate) fn index(&self, x: usize, z: usize) -> usize {
        z * self.width + x
    }

    /// Cell at `(x + dx, z + dz)`, or `None` outside the grid.
    pub(crate) fn offset_cell(&self, x: usize, z: usize, dx: isize, dz: isize) -> Option<(usize, usize)> {
        let nx = x as isize + dx;
        let nz = z as isize + dz;
        if nx < 0 || nz < 0 || nx >= self.width as isize || nz >= self.depth as isize {
            return None;
        }
        Some((nx as usize, nz as usize))
    }
}

/// Heightfield: a 2D grid of min/max height spans for voxelized geometry.
pub(crate) struct Heightfield {
    pub(crate) grid: GridLayout,
    /// For each cell (x, z), store (min_y, max_y). None if empty.
    pub(crate) cells: Vec<Option<(f32, f32)>>,
}

/// Map of walkable cells. True if the cell is walkable.
pub(crate) struct WalkableMap {
    pub(crate) grid: GridLayout,
    pub(crate) walkable: Vec<bool>,
    /// Height at each walkable cell (top of walkable surface).
    pub(crate) heights: Vec<f32>,
}

/// A region of connected walkable cells (flood-fill result).
pub(crate) struct RegionMap {
    pub(crate) grid: GridLayout,
    /// Region ID per cell. 0 = not walkable, 1+ = region ID.
    pub(crate) regions: Vec<u32>,
    pub(crate) heights: Vec<f32>,
    /// Number of distinct regions (read by tests; lib code uses `regions` directly).
    #[allow(dead_code)]
    pub(crate) num_regions: u32,
}

impl NavMeshBuilder {
    /// Builder with default parameters (see field docs).
    pub fn new() -> Self {
        Self::default()
    }

    /// Full pipeline: voxelize, mark walkable, build regions, triangulate.
    pub fn build(&self, vertices: &[Vec3], indices: &[u32]) -> NavMesh {
        let heightfield = self.voxelize(vertices, indices);
        let walkable_map = self.mark_walkable(&heightfield);
        let region_map = self.build_regions(&walkable_map);
        self.triangulate(&region_map)
    }
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
                if (4..6).contains(&x) && (4..6).contains(&z) {
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
        let heightfield = builder.voxelize(&verts, &idxs);

        // Should have cells covering the 10x10 area
        assert!(heightfield.grid.width > 0);
        assert!(heightfield.grid.depth > 0);

        // Check that some cells are populated
        let populated = heightfield.cells.iter().filter(|c| c.is_some()).count();
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
        let heightfield = builder.voxelize(&verts, &idxs);
        let walkable_map = builder.mark_walkable(&heightfield);
        let region_map = builder.build_regions(&walkable_map);

        assert_eq!(region_map.num_regions, 1, "flat plane should be a single region");
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
        let heightfield = builder.voxelize(&vertices, &indices);
        let walkable_map = builder.mark_walkable(&heightfield);

        // Most interior cells should be unwalkable due to steep slope
        let walkable_count = walkable_map.walkable.iter().filter(|&&w| w).count();
        // The bottom edge cells might be walkable (flat), but interior should not
        // Just check that not all cells are walkable
        let total_cells = heightfield.cells.iter().filter(|c| c.is_some()).count();
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
