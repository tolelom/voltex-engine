//! A uniform-grid spatial index for gameplay proximity queries — "which
//! entities are within R of this point?" (AoE, aggro range, pickup magnets,
//! flocking neighbours). Brute-force distance loops are O(n) per query; this
//! buckets entities into a hash grid so a radius query only touches the cells
//! the radius overlaps.
//!
//! This is a *gameplay* index over entity positions, distinct from
//! `voltex_physics`'s collider BVH (which answers collision/raycast queries
//! against shapes). It stores positions by value, so build it once per frame
//! from the world and query it many times.
//!
//! ```ignore
//! let grid = SpatialGrid::from_transforms(&world, 4.0); // 4-unit cells
//! for enemy in grid.query_radius(player_pos, aggro_range) {
//!     // …aggro the enemy
//! }
//! ```

use std::collections::HashMap;

use voltex_math::Vec3;

use crate::transform::Transform;
use crate::world::World;
use crate::Entity;

/// Integer cell coordinate.
type Cell = (i32, i32, i32);

/// A uniform spatial hash grid over entity positions.
pub struct SpatialGrid {
    cell_size: f32,
    cells: HashMap<Cell, Vec<Entity>>,
    positions: HashMap<Entity, Vec3>,
}

impl SpatialGrid {
    /// An empty grid with the given `cell_size` (clamped to a small positive
    /// minimum — a non-positive cell size would make the hash meaningless).
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: if cell_size > f32::MIN_POSITIVE {
                cell_size
            } else {
                1.0
            },
            cells: HashMap::new(),
            positions: HashMap::new(),
        }
    }

    /// Build a grid from every entity that has a [`Transform`], using its
    /// `position`. Rebuild each frame (cheap; one pass over the transforms).
    pub fn from_transforms(world: &World, cell_size: f32) -> Self {
        let mut grid = Self::new(cell_size);
        for (entity, t) in world.query::<Transform>() {
            grid.insert(entity, t.position);
        }
        grid
    }

    /// The cell a position maps to.
    fn cell_of(&self, p: Vec3) -> Cell {
        (
            (p.x / self.cell_size).floor() as i32,
            (p.y / self.cell_size).floor() as i32,
            (p.z / self.cell_size).floor() as i32,
        )
    }

    /// Insert (or move) an entity at `pos`.
    pub fn insert(&mut self, entity: Entity, pos: Vec3) {
        // If the entity was already present, remove it from its old cell first.
        if let Some(&old) = self.positions.get(&entity) {
            let old_cell = self.cell_of(old);
            if let Some(v) = self.cells.get_mut(&old_cell) {
                v.retain(|&e| e != entity);
            }
        }
        let cell = self.cell_of(pos);
        self.cells.entry(cell).or_default().push(entity);
        self.positions.insert(entity, pos);
    }

    /// Number of indexed entities.
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// True if no entities are indexed.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Every indexed entity within `radius` of `center` (inclusive), as exact
    /// distances — only cells overlapping the radius are scanned, then
    /// candidates are filtered by true distance. Order is unspecified.
    pub fn query_radius(&self, center: Vec3, radius: f32) -> Vec<Entity> {
        let mut out = Vec::new();
        self.query_radius_into(center, radius, &mut out);
        out
    }

    /// [`query_radius`](Self::query_radius) into a caller-provided buffer (the
    /// buffer is cleared first) to avoid per-call allocation in hot loops.
    pub fn query_radius_into(&self, center: Vec3, radius: f32, out: &mut Vec<Entity>) {
        out.clear();
        if radius.is_nan() || radius < 0.0 {
            return; // NaN or negative radius → no hits
        }
        let r2 = radius * radius;
        let min = self.cell_of(Vec3::new(
            center.x - radius,
            center.y - radius,
            center.z - radius,
        ));
        let max = self.cell_of(Vec3::new(
            center.x + radius,
            center.y + radius,
            center.z + radius,
        ));
        for cx in min.0..=max.0 {
            for cy in min.1..=max.1 {
                for cz in min.2..=max.2 {
                    if let Some(bucket) = self.cells.get(&(cx, cy, cz)) {
                        for &e in bucket {
                            let p = self.positions[&e];
                            let d = p - center;
                            if d.x * d.x + d.y * d.y + d.z * d.z <= r2 {
                                out.push(e);
                            }
                        }
                    }
                }
            }
        }
    }

    /// The single closest indexed entity to `point`, or `None` if the grid is
    /// empty (after applying `exclude`). Scans all entities — for "nearest
    /// anything" this is O(n); for bounded-range nearest, prefer
    /// [`query_radius`](Self::query_radius). `exclude` skips one entity (e.g.
    /// the querier itself).
    pub fn nearest(&self, point: Vec3, exclude: Option<Entity>) -> Option<Entity> {
        let mut best: Option<(Entity, f32)> = None;
        for (&e, &p) in &self.positions {
            if Some(e) == exclude {
                continue;
            }
            let d = p - point;
            let d2 = d.x * d.x + d.y * d.y + d.z * d.z;
            if best.is_none_or(|(_, bd2)| d2 < bd2) {
                best = Some((e, d2));
            }
        }
        best.map(|(e, _)| e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(world: &mut World, pos: Vec3) -> Entity {
        let e = world.spawn();
        world.add(e, Transform::from_position(pos));
        e
    }

    #[test]
    fn query_radius_returns_only_entities_inside() {
        let mut world = World::new();
        let near = at(&mut world, Vec3::new(1.0, 0.0, 0.0));
        let far = at(&mut world, Vec3::new(50.0, 0.0, 0.0));
        let grid = SpatialGrid::from_transforms(&world, 4.0);

        let hits = grid.query_radius(Vec3::ZERO, 5.0);
        assert!(hits.contains(&near));
        assert!(!hits.contains(&far));
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn query_radius_spans_cell_boundaries() {
        // Two points close together but on opposite sides of a cell edge must
        // both be found — the query has to scan neighbouring cells.
        let mut world = World::new();
        let a = at(&mut world, Vec3::new(3.9, 0.0, 0.0));
        let b = at(&mut world, Vec3::new(4.1, 0.0, 0.0)); // next cell over (size 4)
        let grid = SpatialGrid::from_transforms(&world, 4.0);

        let hits = grid.query_radius(Vec3::new(4.0, 0.0, 0.0), 0.5);
        assert!(hits.contains(&a), "left-of-edge point missed");
        assert!(hits.contains(&b), "right-of-edge point missed");
    }

    #[test]
    fn query_radius_is_inclusive_at_the_boundary() {
        let mut world = World::new();
        let on_edge = at(&mut world, Vec3::new(0.0, 0.0, 5.0));
        let grid = SpatialGrid::from_transforms(&world, 2.0);
        assert!(grid.query_radius(Vec3::ZERO, 5.0).contains(&on_edge));
    }

    #[test]
    fn query_radius_handles_negative_coordinates() {
        let mut world = World::new();
        let e = at(&mut world, Vec3::new(-7.0, -3.0, -1.0));
        let grid = SpatialGrid::from_transforms(&world, 4.0);
        assert!(grid.query_radius(Vec3::new(-6.0, -3.0, -1.0), 2.0).contains(&e));
    }

    #[test]
    fn nearest_returns_closest_and_respects_exclude() {
        let mut world = World::new();
        let p = at(&mut world, Vec3::ZERO);
        let close = at(&mut world, Vec3::new(2.0, 0.0, 0.0));
        let mid = at(&mut world, Vec3::new(6.0, 0.0, 0.0));
        let grid = SpatialGrid::from_transforms(&world, 4.0);

        // Nearest to origin, excluding the querier itself, is `close`.
        assert_eq!(grid.nearest(Vec3::ZERO, Some(p)), Some(close));
        // From far right, `mid` is nearest.
        assert_eq!(grid.nearest(Vec3::new(10.0, 0.0, 0.0), None), Some(mid));
    }

    #[test]
    fn empty_grid_queries_are_empty() {
        let grid = SpatialGrid::new(4.0);
        assert!(grid.is_empty());
        assert!(grid.query_radius(Vec3::ZERO, 100.0).is_empty());
        assert_eq!(grid.nearest(Vec3::ZERO, None), None);
    }

    #[test]
    fn reinserting_an_entity_moves_it_between_cells() {
        let mut grid = SpatialGrid::new(4.0);
        let e = Entity::from_raw_parts(0, 0);
        grid.insert(e, Vec3::new(1.0, 0.0, 0.0));
        grid.insert(e, Vec3::new(100.0, 0.0, 0.0)); // moved far away
        assert_eq!(grid.len(), 1, "re-insert must not duplicate");
        assert!(grid.query_radius(Vec3::ZERO, 5.0).is_empty(), "stale cell entry left behind");
        assert!(grid.query_radius(Vec3::new(100.0, 0.0, 0.0), 5.0).contains(&e));
    }

    #[test]
    fn degenerate_cell_size_does_not_panic() {
        let mut world = World::new();
        at(&mut world, Vec3::new(1.0, 1.0, 1.0));
        let grid = SpatialGrid::from_transforms(&world, 0.0); // clamped to 1.0
        assert_eq!(grid.query_radius(Vec3::new(1.0, 1.0, 1.0), 1.0).len(), 1);
    }
}
