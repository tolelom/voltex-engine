//! GPU glyph atlas — rasterised TTF glyphs packed into a single texture.
//! [`TtfFont`](crate::TtfFont) lazily fills the cache as new code points are
//! requested.
//!
//! `GlyphInfo` / `GlyphCache` expose every packing field as `pub` for tests;
//! user code should reach for [`TtfFont`](crate::TtfFont) directly.
#![allow(missing_docs)]

use std::collections::HashMap;

/// Side length (in pixels) of the square glyph atlas texture.
pub const ATLAS_SIZE: u32 = 1024;

#[derive(Clone, Debug)]
pub struct GlyphInfo {
    pub uv: [f32; 4],       // u0, v0, u1, v1
    pub width: f32,
    pub height: f32,
    pub advance: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
}

/// A rasterised glyph: `width * height` alpha bytes, row-major.
#[derive(Clone, Copy)]
pub struct GlyphBitmap<'a> {
    pub pixels: &'a [u8],
    pub width: u32,
    pub height: u32,
}

impl GlyphBitmap<'_> {
    /// Zero-size bitmap for glyphs with no outline (e.g. space).
    pub const EMPTY: GlyphBitmap<'static> = GlyphBitmap { pixels: &[], width: 0, height: 0 };
}

/// Where a glyph sits relative to the pen, in pixels: horizontal advance,
/// left bearing and distance from the baseline up to the bitmap top.
#[derive(Clone, Copy)]
pub struct GlyphPlacement {
    pub advance: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
}

pub struct GlyphCache {
    pub atlas_data: Vec<u8>,
    pub atlas_width: u32,
    pub atlas_height: u32,
    glyphs: HashMap<char, GlyphInfo>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    pub dirty: bool,
}

impl GlyphCache {
    pub fn new(width: u32, height: u32) -> Self {
        GlyphCache {
            atlas_data: vec![0u8; (width * height) as usize],
            atlas_width: width,
            atlas_height: height,
            glyphs: HashMap::new(),
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            dirty: false,
        }
    }

    pub fn get(&self, ch: char) -> Option<&GlyphInfo> {
        self.glyphs.get(&ch)
    }

    /// Old form of [`Self::insert_glyph`] with the bitmap and placement
    /// spelled out.
    #[deprecated(note = "use `insert_glyph` with a `GlyphBitmap` and a `GlyphPlacement`")]
    #[allow(clippy::too_many_arguments)] // reason: legacy signature kept for compatibility
    pub fn insert(
        &mut self,
        ch: char,
        bitmap: &[u8],
        bmp_w: u32,
        bmp_h: u32,
        advance: f32,
        bearing_x: f32,
        bearing_y: f32,
    ) -> &GlyphInfo {
        let bitmap = GlyphBitmap { pixels: bitmap, width: bmp_w, height: bmp_h };
        self.insert_glyph(ch, bitmap, GlyphPlacement { advance, bearing_x, bearing_y })
    }

    /// Insert a rasterized glyph bitmap into the atlas.
    /// Returns reference to the cached GlyphInfo.
    pub fn insert_glyph(&mut self, ch: char, bitmap: GlyphBitmap, placement: GlyphPlacement) -> &GlyphInfo {
        let GlyphBitmap { pixels: bitmap, width: bmp_w, height: bmp_h } = bitmap;
        let GlyphPlacement { advance, bearing_x, bearing_y } = placement;
        // Handle zero-size glyphs (e.g., space)
        if bmp_w == 0 || bmp_h == 0 {
            self.glyphs.insert(ch, GlyphInfo {
                uv: [0.0, 0.0, 0.0, 0.0],
                width: 0.0,
                height: 0.0,
                advance,
                bearing_x,
                bearing_y,
            });
            return self.glyphs.get(&ch).unwrap();
        }

        // Check if we need to wrap to next row
        if self.cursor_x + bmp_w > self.atlas_width {
            self.cursor_y += self.row_height + 1; // +1 pixel gap
            self.cursor_x = 0;
            self.row_height = 0;
        }

        // Check if atlas is full (would overflow vertically)
        if self.cursor_y + bmp_h > self.atlas_height {
            // Atlas full — insert with zero UV (glyph won't render but won't crash)
            self.glyphs.insert(ch, GlyphInfo {
                uv: [0.0, 0.0, 0.0, 0.0],
                width: bmp_w as f32,
                height: bmp_h as f32,
                advance,
                bearing_x,
                bearing_y,
            });
            return self.glyphs.get(&ch).unwrap();
        }

        // Copy bitmap into atlas
        for row in 0..bmp_h {
            let src_start = (row * bmp_w) as usize;
            let src_end = src_start + bmp_w as usize;
            let dst_y = self.cursor_y + row;
            let dst_x = self.cursor_x;
            let dst_start = (dst_y * self.atlas_width + dst_x) as usize;
            if src_end <= bitmap.len() && dst_start + bmp_w as usize <= self.atlas_data.len() {
                self.atlas_data[dst_start..dst_start + bmp_w as usize]
                    .copy_from_slice(&bitmap[src_start..src_end]);
            }
        }

        // Calculate UV coordinates
        let u0 = self.cursor_x as f32 / self.atlas_width as f32;
        let v0 = self.cursor_y as f32 / self.atlas_height as f32;
        let u1 = (self.cursor_x + bmp_w) as f32 / self.atlas_width as f32;
        let v1 = (self.cursor_y + bmp_h) as f32 / self.atlas_height as f32;

        let info = GlyphInfo {
            uv: [u0, v0, u1, v1],
            width: bmp_w as f32,
            height: bmp_h as f32,
            advance,
            bearing_x,
            bearing_y,
        };

        // Advance cursor
        self.cursor_x += bmp_w + 1; // +1 pixel gap
        if bmp_h > self.row_height {
            self.row_height = bmp_h;
        }

        self.dirty = true;
        self.glyphs.insert(ch, info);
        self.glyphs.get(&ch).unwrap()
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut cache = GlyphCache::new(256, 256);
        let bitmap = vec![255u8; 10 * 12]; // 10x12 glyph
        cache.insert_glyph('A', GlyphBitmap { pixels: &bitmap, width: 10, height: 12 }, GlyphPlacement { advance: 11.0, bearing_x: 0.5, bearing_y: 10.0 });
        let info = cache.get('A');
        assert!(info.is_some());
        let info = info.unwrap();
        assert!((info.width - 10.0).abs() < 0.1);
        assert!((info.advance - 11.0).abs() < 0.1);
        assert!(info.uv[0] >= 0.0 && info.uv[2] <= 1.0);
    }

    #[test]
    fn test_cache_hit() {
        let mut cache = GlyphCache::new(256, 256);
        let bitmap = vec![255u8; 8 * 8];
        cache.insert_glyph('B', GlyphBitmap { pixels: &bitmap, width: 8, height: 8 }, GlyphPlacement { advance: 9.0, bearing_x: 0.0, bearing_y: 8.0 });
        cache.clear_dirty();
        // Second access should be cache hit (no dirty)
        let _info = cache.get('B').unwrap();
        assert!(!cache.dirty);
    }

    #[test]
    fn test_row_wrap() {
        let mut cache = GlyphCache::new(64, 64);
        let bitmap = vec![255u8; 20 * 10];
        // Insert 4 glyphs of width 20 in a 64-wide atlas
        // 3 fit in first row (20+1 + 20+1 + 20 = 62), 4th wraps
        cache.insert_glyph('A', GlyphBitmap { pixels: &bitmap, width: 20, height: 10 }, GlyphPlacement { advance: 21.0, bearing_x: 0.0, bearing_y: 10.0 });
        cache.insert_glyph('B', GlyphBitmap { pixels: &bitmap, width: 20, height: 10 }, GlyphPlacement { advance: 21.0, bearing_x: 0.0, bearing_y: 10.0 });
        cache.insert_glyph('C', GlyphBitmap { pixels: &bitmap, width: 20, height: 10 }, GlyphPlacement { advance: 21.0, bearing_x: 0.0, bearing_y: 10.0 });
        cache.insert_glyph('D', GlyphBitmap { pixels: &bitmap, width: 20, height: 10 }, GlyphPlacement { advance: 21.0, bearing_x: 0.0, bearing_y: 10.0 });
        let d = cache.get('D').unwrap();
        // D should be on a different row (v0 > 0)
        assert!(d.uv[1] > 0.0, "D should be on second row, v0={}", d.uv[1]);
    }

    #[test]
    fn test_uv_range() {
        let mut cache = GlyphCache::new(256, 256);
        let bitmap = vec![128u8; 15 * 20];
        cache.insert_glyph('X', GlyphBitmap { pixels: &bitmap, width: 15, height: 20 }, GlyphPlacement { advance: 16.0, bearing_x: 1.0, bearing_y: 18.0 });
        let info = cache.get('X').unwrap();
        assert!(info.uv[0] >= 0.0 && info.uv[0] < 1.0);
        assert!(info.uv[1] >= 0.0 && info.uv[1] < 1.0);
        assert!(info.uv[2] > info.uv[0] && info.uv[2] <= 1.0);
        assert!(info.uv[3] > info.uv[1] && info.uv[3] <= 1.0);
    }

    #[test]
    fn test_zero_size_glyph() {
        let mut cache = GlyphCache::new(256, 256);
        cache.insert_glyph(' ', GlyphBitmap::EMPTY, GlyphPlacement { advance: 5.0, bearing_x: 0.0, bearing_y: 0.0 });
        let info = cache.get(' ').unwrap();
        assert!((info.advance - 5.0).abs() < 0.1);
        assert!((info.width - 0.0).abs() < 0.1);
    }
}
