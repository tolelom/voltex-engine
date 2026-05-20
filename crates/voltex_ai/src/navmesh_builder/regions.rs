use super::{NavMeshBuilder, RegionMap, WalkableMap};

impl NavMeshBuilder {
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
}
