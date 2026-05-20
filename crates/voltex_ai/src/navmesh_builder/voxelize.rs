use super::{Heightfield, NavMeshBuilder};
use voltex_math::Vec3;

impl NavMeshBuilder {
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
                    if super::triangulate::point_in_triangle_xz_loose(p, v0, v1, v2, self.cell_size * 0.5) {
                        // Interpolate Y at this XZ point
                        let y = super::triangulate::interpolate_y(v0, v1, v2, cx, cz);
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
}
