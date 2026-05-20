use super::{Heightfield, NavMeshBuilder, WalkableMap};

impl NavMeshBuilder {
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
}
