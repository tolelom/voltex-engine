//! [`Sprite`] — a camera-facing billboard component for the [`BasicRenderer`]
//! path. An entity with a [`WorldTransform`](voltex_ecs::WorldTransform) and a
//! `Sprite` is drawn as a screen-aligned quad at its world position, after the
//! mesh passes (alpha-blended, depth-tested). Use for pickups, particles,
//! world-space markers, or simple 2D-in-3D enemies.

use crate::assets::TextureHandle;

/// A billboarded quad. `size` is the world-space width/height; `color` tints
/// the texture (or acts as a solid color when `texture` is `None`, since the
/// renderer samples a white default). `color.w` is alpha.
#[derive(Debug, Clone, Copy)]
pub struct Sprite {
    /// Optional diffuse texture; `None` → solid `color` quad.
    pub texture: Option<TextureHandle>,
    /// World-space [width, height].
    pub size: [f32; 2],
    /// RGBA tint / solid color (linear, alpha-blended).
    pub color: [f32; 4],
    /// Texture sub-rect `[u_min, v_min, u_max, v_max]` for sprite sheets.
    /// `[0, 0, 1, 1]` (default) samples the whole texture.
    pub uv_rect: [f32; 4],
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            texture: None,
            size: [1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
        }
    }
}

impl Sprite {
    /// A solid-color billboard of the given world-space size.
    pub fn solid(size: [f32; 2], color: [f32; 4]) -> Self {
        Self { texture: None, size, color, ..Default::default() }
    }

    /// A textured billboard (white tint) of the given world-space size.
    pub fn textured(texture: TextureHandle, size: [f32; 2]) -> Self {
        Self {
            texture: Some(texture),
            size,
            ..Default::default()
        }
    }

    /// Set the texture sub-rect (builder style) — e.g. for an atlas region.
    pub fn with_uv_rect(mut self, uv_rect: [f32; 4]) -> Self {
        self.uv_rect = uv_rect;
        self
    }

    /// Point this sprite at frame `index` of a `cols × rows` sprite sheet
    /// (row-major, left-to-right then top-to-bottom). Sets `uv_rect`.
    pub fn set_sheet_frame(&mut self, cols: u32, rows: u32, index: u32) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let index = index % (cols * rows);
        let (cx, cy) = (index % cols, index / cols);
        let (fw, fh) = (1.0 / cols as f32, 1.0 / rows as f32);
        let u0 = cx as f32 * fw;
        let v0 = cy as f32 * fh;
        self.uv_rect = [u0, v0, u0 + fw, v0 + fh];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_uv_rect_is_full_texture() {
        assert_eq!(Sprite::default().uv_rect, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(Sprite::solid([1.0, 1.0], [1.0; 4]).uv_rect, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn sheet_frame_picks_the_right_cell() {
        let mut s = Sprite::default();
        // 2×2 sheet: frame 0 = top-left quadrant.
        s.set_sheet_frame(2, 2, 0);
        assert_eq!(s.uv_rect, [0.0, 0.0, 0.5, 0.5]);
        // frame 3 = bottom-right quadrant (col 1, row 1).
        s.set_sheet_frame(2, 2, 3);
        assert_eq!(s.uv_rect, [0.5, 0.5, 1.0, 1.0]);
        // index wraps modulo the frame count.
        s.set_sheet_frame(2, 2, 4);
        assert_eq!(s.uv_rect, [0.0, 0.0, 0.5, 0.5]);
    }
}
