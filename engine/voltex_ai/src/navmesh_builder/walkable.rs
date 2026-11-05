//! Navmesh build step 2: `NavMeshBuilder::mark_walkable` keeps heightfield
//! cells whose slope is under `max_slope`, then erodes them by `agent_radius`.

use super::{Heightfield, NavMeshBuilder, WalkableMap, FOUR_NEIGHBORS};

impl NavMeshBuilder {
    /// Step 2: Mark walkable cells by slope, then erode by the agent radius.
    /// Height clearance (`agent_height`) is not checked yet.
    pub(crate) fn mark_walkable(&self, heightfield: &Heightfield) -> WalkableMap {
        let grid = heightfield.grid;
        let max_slope_cos = (self.max_slope * std::f32::consts::PI / 180.0).cos();
        let mut walkable = vec![false; grid.cell_count()];
        let mut heights = vec![0.0f32; grid.cell_count()];

        for z in 0..grid.depth {
            for x in 0..grid.width {
                let idx = grid.index(x, z);
                if let Some((_lo, hi)) = heightfield.cells[idx] {
                    if self.is_slope_walkable(heightfield, x, z, max_slope_cos) {
                        walkable[idx] = true;
                        heights[idx] = hi;
                    }
                }
            }
        }

        // Erode by agent radius: remove walkable cells too close to non-walkable
        let erosion_cells = (self.agent_radius / grid.cell_size).ceil() as usize;
        if erosion_cells > 0 {
            let mut eroded = walkable.clone();
            for z in 0..grid.depth {
                for x in 0..grid.width {
                    let idx = grid.index(x, z);
                    if walkable[idx] && is_near_unwalkable(heightfield, &walkable, x, z, erosion_cells) {
                        eroded[idx] = false;
                    }
                }
            }
            walkable = eroded;
        }

        WalkableMap { grid, walkable, heights }
    }

    /// `true` if the height step to every existing 4-neighbour of cell
    /// (x, z) is within `max_slope` (given as its cosine).
    fn is_slope_walkable(&self, heightfield: &Heightfield, x: usize, z: usize, max_slope_cos: f32) -> bool {
        let grid = heightfield.grid;
        let h = match heightfield.cells[grid.index(x, z)] {
            Some((_, hi)) => hi,
            None => return false,
        };

        for (dx, dz) in FOUR_NEIGHBORS {
            let Some((nx, nz)) = grid.offset_cell(x, z, dx, dz) else {
                continue;
            };
            if let Some((_, neighbor_h)) = heightfield.cells[grid.index(nx, nz)] {
                let dy = (neighbor_h - h).abs();
                let dist = grid.cell_size;
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
}

/// `true` if any cell within `radius` cells (square neighbourhood, Chebyshev
/// distance) of (x, z) is unwalkable or outside the grid.
fn is_near_unwalkable(heightfield: &Heightfield, walkable: &[bool], x: usize, z: usize, radius: usize) -> bool {
    let grid = heightfield.grid;
    for dz in 0..=radius as isize {
        for dx in 0..=radius as isize {
            if dx == 0 && dz == 0 {
                continue;
            }
            // All 4 quadrants
            for (offset_x, offset_z) in [(dx, dz), (-dx, dz), (dx, -dz), (-dx, -dz)] {
                match grid.offset_cell(x, z, offset_x, offset_z) {
                    None => return true,
                    Some((nx, nz)) if !walkable[grid.index(nx, nz)] => return true,
                    Some(_) => {}
                }
            }
        }
    }
    false
}
