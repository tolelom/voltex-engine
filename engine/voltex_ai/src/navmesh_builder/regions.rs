//! Navmesh build step 3: `NavMeshBuilder::build_regions` flood-fills
//! 4-connected walkable cells into numbered regions (a `RegionMap`).

use super::{NavMeshBuilder, RegionMap, WalkableMap, FOUR_NEIGHBORS};

impl NavMeshBuilder {
    /// Step 3: Flood-fill connected walkable areas into regions.
    pub(crate) fn build_regions(&self, walkable_map: &WalkableMap) -> RegionMap {
        let grid = walkable_map.grid;
        let mut regions = vec![0u32; grid.cell_count()];
        let mut current_region = 0u32;

        for z in 0..grid.depth {
            for x in 0..grid.width {
                let idx = grid.index(x, z);
                if walkable_map.walkable[idx] && regions[idx] == 0 {
                    current_region += 1;
                    // Flood fill
                    let mut stack = vec![(x, z)];
                    regions[idx] = current_region;
                    while let Some((cx, cz)) = stack.pop() {
                        for (dx, dz) in FOUR_NEIGHBORS {
                            let Some((nx, nz)) = grid.offset_cell(cx, cz, dx, dz) else {
                                continue;
                            };
                            let ni = grid.index(nx, nz);
                            if walkable_map.walkable[ni] && regions[ni] == 0 {
                                regions[ni] = current_region;
                                stack.push((nx, nz));
                            }
                        }
                    }
                }
            }
        }

        RegionMap {
            grid,
            regions,
            heights: walkable_map.heights.clone(),
            num_regions: current_region,
        }
    }
}
