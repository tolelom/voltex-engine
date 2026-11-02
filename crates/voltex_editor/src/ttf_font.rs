//! High-level TrueType font — parser + glyph rasteriser + glyph cache,
//! wrapped behind a single `text_quads(...)` API.
#![allow(missing_docs)]

use crate::ttf_parser::TtfParser;
use crate::rasterizer::rasterize;
use crate::glyph_cache::{GlyphCache, GlyphInfo, ATLAS_SIZE};

pub struct TtfFont {
    parser: TtfParser,
    cache: GlyphCache,
    pub font_size: f32,
    pub line_height: f32,
    pub ascender: f32,
    pub descender: f32,
    scale: f32,
}

impl TtfFont {
    pub fn new(data: &[u8], font_size: f32) -> crate::Result<Self> {
        let parser = TtfParser::parse(data.to_vec())?;
        let scale = font_size / parser.units_per_em as f32;
        let ascender = parser.ascender as f32 * scale;
        let descender = parser.descender as f32 * scale;
        // i16 metrics straight from the file: sum in i32 so corrupt values can't overflow.
        let line_height = (parser.ascender as i32 - parser.descender as i32 + parser.line_gap as i32) as f32 * scale;
        let cache = GlyphCache::new(ATLAS_SIZE, ATLAS_SIZE);

        Ok(TtfFont {
            parser,
            cache,
            font_size,
            line_height,
            ascender,
            descender,
            scale,
        })
    }

    /// Get glyph info for a character, rasterizing on cache miss.
    pub fn glyph(&mut self, ch: char) -> &GlyphInfo {
        if self.cache.get(ch).is_some() {
            return self.cache.get(ch).unwrap();
        }

        let glyph_id = self.parser.glyph_index(ch as u32);
        let metrics = self.parser.glyph_metrics(glyph_id);
        let advance = metrics.advance_width as f32 * self.scale;
        let bearing_x = metrics.left_side_bearing as f32 * self.scale;

        let outline = self.parser.glyph_outline(glyph_id);

        match outline {
            Some(ref o) if !o.contours.is_empty() => {
                let result = rasterize(o, self.scale);
                // bearing_y = y_max * scale (distance above baseline)
                // When rendering: glyph_y = baseline_y - bearing_y
                // For our UI: text_y is the top of the line, baseline = text_y + ascender
                // So: glyph_y = text_y + ascender - bearing_y
                let bearing_y = o.y_max as f32 * self.scale;

                self.cache.insert(
                    ch,
                    &result.bitmap,
                    result.width,
                    result.height,
                    advance,
                    bearing_x,
                    bearing_y,
                );
            }
            _ => {
                // No outline (space, etc.) — insert empty glyph
                self.cache.insert(ch, &[], 0, 0, advance, 0.0, 0.0);
            }
        }

        self.cache.get(ch).unwrap()
    }

    /// Calculate total width of a text string.
    pub fn text_width(&mut self, text: &str) -> f32 {
        let mut width = 0.0;
        for ch in text.chars() {
            let info = self.glyph(ch);
            width += info.advance;
        }
        width
    }

    pub fn atlas_data(&self) -> &[u8] {
        &self.cache.atlas_data
    }

    pub fn atlas_size(&self) -> (u32, u32) {
        (self.cache.atlas_width, self.cache.atlas_height)
    }

    pub fn is_dirty(&self) -> bool {
        self.cache.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.cache.clear_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_font() -> Option<TtfFont> {
        TtfFont::new(&crate::test_font::minimal_ttf(), 24.0).ok()
    }

    #[test]
    fn test_new() {
        let font = load_test_font().expect("no test font");
        assert!(font.font_size == 24.0);
        assert!(font.ascender > 0.0);
        assert!(font.line_height > 0.0);
    }

    #[test]
    fn test_glyph_caches() {
        let mut font = load_test_font().expect("no test font");
        let _g1 = font.glyph('A');
        assert!(font.is_dirty());
        font.clear_dirty();
        let _g2 = font.glyph('A'); // cache hit
        assert!(!font.is_dirty());
    }

    #[test]
    fn test_text_width() {
        let mut font = load_test_font().expect("no test font");
        let w1 = font.text_width("Hello");
        let w2 = font.text_width("Hello World");
        assert!(w1 > 0.0);
        assert!(w2 > w1);
    }

    #[test]
    fn test_space_glyph() {
        let mut font = load_test_font().expect("no test font");
        let info = font.glyph(' ');
        assert!(info.advance > 0.0); // space has advance but no bitmap
        assert!(info.width == 0.0);
    }
}
