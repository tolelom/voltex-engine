//! Navmesh build step 1: `NavMeshBuilder::voxelize` rasterizes the input
//! triangles into an XZ `Heightfield`, recording the min/max surface height per cell.

use super::{GridLayout, Heightfield, NavMeshBuilder};
use voltex_math::Vec3;

impl NavMeshBuilder {
    /// Step 1: Rasterize triangles into a heightfield grid.
    pub(crate) fn voxelize(&self, vertices: &[Vec3], indices: &[u32]) -> Heightfield {
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

            let gx_min = ((tri_min_x - min_x) / self.cell_size).floor() as isize;
            let gx_max = ((tri_max_x - min_x) / self.cell_size).ceil() as isize;
            let gz_min = ((tri_min_z - min_z) / self.cell_size).floor() as isize;
            let gz_max = ((tri_max_z - min_z) / self.cell_size).ceil() as isize;

            let gx_min = gx_min.max(0) as usize;
            let gx_max = (gx_max as usize).min(width - 1);
            let gz_min = gz_min.max(0) as usize;
            let gz_max = (gz_max as usize).min(depth - 1);

            for gz in gz_min..=gz_max {
                for gx in gx_min..=gx_max {
                    let center_x = min_x + (gx as f32 + 0.5) * self.cell_size;
                    let center_z = min_z + (gz as f32 + 0.5) * self.cell_size;

                    // Check if cell center is inside triangle (XZ)
                    let center = Vec3::new(center_x, 0.0, center_z);
                    if super::triangulate::point_in_triangle_xz_loose(center, v0, v1, v2, self.cell_size * 0.5) {
                        // Interpolate Y at this XZ point
                        let y = super::triangulate::interpolate_y(v0, v1, v2, center_x, center_z);
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
            grid: GridLayout { width, depth, min_x, min_z, cell_size: self.cell_size },
            cells,
        }
    }
}
