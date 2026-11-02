//! `Tilemap` component: a grid of atlas-indexed cells drawn by
//! `BasicRenderer` as one batched draw.

use crate::assets::TextureHandle;
use voltex_math::{Mat4, Vec4};
use voltex_renderer::TilemapVertex;

/// A `cols × rows` grid of square cells in the entity's local XY plane. Cell
/// `(gx, gy)` covers `[gx*ts, (gx+1)*ts] × [gy*ts, (gy+1)*ts]` with **gy
/// growing +y (up)**; `tiles[gy * cols + gx]` is `0` for empty, `n` for atlas
/// frame `n - 1`. Unlike [`Sprite`](crate::Sprite) it is not a billboard — the
/// entity's `WorldTransform` (position/rotation/scale) places the plane.
#[derive(Debug, Clone)]
pub struct Tilemap {
    /// Grid width in cells.
    pub cols: u32,
    /// Grid height in cells.
    pub rows: u32,
    /// World size of one (square) cell.
    pub tile_size: f32,
    /// Row-major cells, `len == cols * rows`. Extra entries are ignored,
    /// missing ones read as empty.
    pub tiles: Vec<u16>,
    /// Atlas texture; `None` → white default (solid `color` tiles).
    pub texture: Option<TextureHandle>,
    /// Atlas grid `[cols, rows]`. Frame numbering matches
    /// [`Sprite::set_sheet_frame`](crate::Sprite::set_sheet_frame): row-major
    /// from the top-left.
    pub atlas: [u32; 2],
    /// Linear RGBA tint for the whole map.
    pub color: [f32; 4],
}

impl Tilemap {
    /// An all-empty `cols × rows` map of solid-color tiles (1×1 atlas).
    pub fn solid(cols: u32, rows: u32, tile_size: f32, color: [f32; 4]) -> Self {
        Self {
            cols,
            rows,
            tile_size,
            tiles: vec![0; (cols * rows) as usize],
            texture: None,
            atlas: [1, 1],
            color,
        }
    }

    /// Set cell `(gx, gy)` to `tile` (`0` clears it). Out-of-range is a no-op.
    pub fn set(&mut self, gx: u32, gy: u32, tile: u16) {
        if gx < self.cols && gy < self.rows {
            let i = (gy * self.cols + gx) as usize;
            if let Some(slot) = self.tiles.get_mut(i) {
                *slot = tile;
            }
        }
    }

    /// Number of non-empty cells inside the grid.
    pub fn filled_count(&self) -> usize {
        self.cells().count()
    }

    fn cells(&self) -> impl Iterator<Item = (u32, u32, u16)> + '_ {
        let cols = self.cols.max(1);
        self.tiles
            .iter()
            .take((self.cols * self.rows) as usize)
            .enumerate()
            .filter(|(_, t)| **t != 0)
            .map(move |(i, t)| (i as u32 % cols, i as u32 / cols, *t))
    }
}

/// Atlas sub-rect `[u0, v0, u1, v1]` for 0-based `frame` of a `cols × rows`
/// atlas (same convention as `Sprite::set_sheet_frame`).
fn atlas_uv(atlas: [u32; 2], frame: u32) -> [f32; 4] {
    let cols = atlas[0].max(1);
    let rows = atlas[1].max(1);
    let frame = frame % (cols * rows);
    let (fw, fh) = (1.0 / cols as f32, 1.0 / rows as f32);
    let u0 = (frame % cols) as f32 * fw;
    let v0 = (frame / cols) as f32 * fh;
    [u0, v0, u0 + fw, v0 + fh]
}

/// Expand every filled cell of `map` into 4 world-space vertices (bottom-left,
/// bottom-right, top-right, top-left — index with
/// `voltex_renderer::tilemap_quad_indices`). `world` is the entity's
/// `WorldTransform` matrix.
pub fn tilemap_vertices(map: &Tilemap, world: &Mat4) -> Vec<TilemapVertex> {
    let ts = map.tile_size;
    let mut out = Vec::with_capacity(map.filled_count() * 4);
    for (gx, gy, tile) in map.cells() {
        let [u0, v0, u1, v1] = atlas_uv(map.atlas, u32::from(tile) - 1);
        let (x0, y0) = (gx as f32 * ts, gy as f32 * ts);
        let (x1, y1) = (x0 + ts, y0 + ts);
        // Texture v grows downward; the quad's top edge samples v0.
        for (x, y, u, v) in [(x0, y0, u0, v1), (x1, y0, u1, v1), (x1, y1, u1, v0), (x0, y1, u0, v0)] {
            let p = *world * Vec4::new(x, y, 0.0, 1.0);
            out.push(TilemapVertex {
                position: [p.x, p.y, p.z],
                uv: [u, v],
                color: map.color,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

    fn positions(v: &[TilemapVertex]) -> Vec<[f32; 3]> {
        v.iter().map(|v| v.position).collect()
    }

    #[test]
    fn empty_cells_produce_no_vertices() {
        let map = Tilemap::solid(3, 2, 1.0, WHITE);
        assert!(tilemap_vertices(&map, &Mat4::IDENTITY).is_empty());
    }

    #[test]
    fn filled_cell_covers_its_grid_square_with_y_up() {
        let mut map = Tilemap::solid(3, 2, 2.0, WHITE);
        map.set(2, 1, 1); // third column, top row (gx != gy so axes can't swap)
        let v = tilemap_vertices(&map, &Mat4::IDENTITY);
        assert_eq!(
            positions(&v),
            vec![[4.0, 2.0, 0.0], [6.0, 2.0, 0.0], [6.0, 4.0, 0.0], [4.0, 4.0, 0.0]]
        );
    }

    #[test]
    fn world_transform_translates_and_scales_the_map() {
        let mut map = Tilemap::solid(1, 1, 1.0, WHITE);
        map.set(0, 0, 1);
        let world = Mat4::translation(10.0, -3.0, 0.5) * Mat4::scale(2.0, 2.0, 1.0);
        let v = tilemap_vertices(&map, &world);
        assert_eq!(v[0].position, [10.0, -3.0, 0.5]);
        assert_eq!(v[2].position, [12.0, -1.0, 0.5]);
    }

    #[test]
    fn atlas_frame_selects_sub_rect_top_left_first() {
        let mut map = Tilemap::solid(1, 1, 1.0, WHITE);
        map.atlas = [2, 2];
        map.set(0, 0, 4); // frame 3 = bottom-right quarter of the atlas
        let v = tilemap_vertices(&map, &Mat4::IDENTITY);
        // bottom-left corner samples (u0, v1), top-right samples (u1, v0).
        assert_eq!(v[0].uv, [0.5, 1.0]);
        assert_eq!(v[2].uv, [1.0, 0.5]);
    }

    #[test]
    fn tint_is_copied_to_every_vertex() {
        let tint = [0.2, 0.4, 0.6, 0.8];
        let mut map = Tilemap::solid(2, 1, 1.0, tint);
        map.set(0, 0, 1);
        map.set(1, 0, 1);
        let v = tilemap_vertices(&map, &Mat4::IDENTITY);
        assert_eq!(v.len(), 8);
        assert!(v.iter().all(|v| v.color == tint));
    }

    #[test]
    fn mismatched_tiles_len_is_tolerated() {
        let mut map = Tilemap::solid(2, 1, 1.0, WHITE);
        map.tiles = vec![1]; // shorter: only cell 0
        assert_eq!(tilemap_vertices(&map, &Mat4::IDENTITY).len(), 4);
        map.tiles = vec![1, 1, 1, 1]; // longer: extras ignored
        assert_eq!(tilemap_vertices(&map, &Mat4::IDENTITY).len(), 8);
    }

    #[test]
    fn set_out_of_range_is_a_no_op() {
        let mut map = Tilemap::solid(2, 2, 1.0, WHITE);
        map.set(2, 0, 1);
        map.set(0, 2, 1);
        assert_eq!(map.filled_count(), 0);
    }
}
