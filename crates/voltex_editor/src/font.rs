/// Bitmap font atlas for ASCII 32-126.
/// 8x12 pixel glyphs arranged in 16 columns x 6 rows = 128x72 texture.
pub struct FontAtlas {
    pub width: u32,
    pub height: u32,
    pub glyph_width: u32,
    pub glyph_height: u32,
    pub pixels: Vec<u8>, // R8 grayscale
}

impl FontAtlas {
    /// Generate a minimal font atlas.
    /// Each glyph is 8x12 pixels. Atlas is 16 cols x 6 rows = 128x72.
    /// Space (32) = all zeros. Other chars get a simple recognizable pattern.
    pub fn generate() -> Self {
        let glyph_width: u32 = 8;
        let glyph_height: u32 = 12;
        let cols: u32 = 16;
        let rows: u32 = 6;
        let width = cols * glyph_width;   // 128
        let height = rows * glyph_height; // 72

        let mut pixels = vec![0u8; (width * height) as usize];

        // ASCII 32 (space) through 126 (~) = 95 characters
        for code in 32u8..=126u8 {
            let index = (code - 32) as u32;
            let col = index % cols;
            let row = index / cols;
            let base_x = col * glyph_width;
            let base_y = row * glyph_height;

            if code == 32 {
                // Space: all zeros (already zero)
                continue;
            }

            // Generate a recognizable pattern based on char code.
            // Draw a border frame + inner pattern derived from code value.
            for py in 0..glyph_height {
                for px in 0..glyph_width {
                    let pixel_idx = ((base_y + py) * width + (base_x + px)) as usize;

                    let on_border = px == 0 || px == glyph_width - 1
                        || py == 0 || py == glyph_height - 1;

                    // Inner pattern: use bits of char code to create variation
                    let inner_bit = ((code as u32).wrapping_mul(px + 1).wrapping_mul(py + 1)) & 0x3;
                    let on_inner = inner_bit == 0 && px > 1 && px < glyph_width - 2
                        && py > 2 && py < glyph_height - 2;

                    // Horizontal bar in middle for letter-like appearance
                    let mid_y = glyph_height / 2;
                    let on_midbar = py == mid_y && px > 1 && px < glyph_width - 2;

                    // Vertical stem on left side
                    let on_stem = px == 2 && py > 1 && py < glyph_height - 2;

                    if on_border || on_inner || on_midbar || on_stem {
                        pixels[pixel_idx] = 255;
                    }
                }
            }
        }

        // Ensure pixel (0,0) is white (255) so solid-color rects can sample
        // UV (0,0) and get full brightness for tinting.
        pixels[0] = 255;

        FontAtlas {
            width,
            height,
            glyph_width,
            glyph_height,
            pixels,
        }
    }

    /// Returns (u0, v0, u1, v1) UV coordinates for a given character.
    /// Returns coordinates for space if character is out of ASCII range.
    pub fn glyph_uv(&self, ch: char) -> (f32, f32, f32, f32) {
        let code = ch as u32;
        let index = if code >= 32 && code <= 126 {
            code - 32
        } else {
            0 // space
        };

        let cols = self.width / self.glyph_width;
        let col = index % cols;
        let row = index / cols;

        let u0 = (col * self.glyph_width) as f32 / self.width as f32;
        let v0 = (row * self.glyph_height) as f32 / self.height as f32;
        let u1 = u0 + self.glyph_width as f32 / self.width as f32;
        let v1 = v0 + self.glyph_height as f32 / self.height as f32;

        (u0, v0, u1, v1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_size() {
        let atlas = FontAtlas::generate();
        assert_eq!(atlas.width, 128);
        assert_eq!(atlas.height, 72);
        assert_eq!(atlas.pixels.len(), (128 * 72) as usize);
    }

    #[test]
    fn test_glyph_uv_space() {
        let atlas = FontAtlas::generate();
        let (u0, v0, u1, v1) = atlas.glyph_uv(' ');
        assert!((u0 - 0.0).abs() < 1e-6);
        assert!((v0 - 0.0).abs() < 1e-6);
        assert!((u1 - 8.0 / 128.0).abs() < 1e-6);
        assert!((v1 - 12.0 / 72.0).abs() < 1e-6);
    }

    #[test]
    fn test_glyph_uv_a() {
        let atlas = FontAtlas::generate();
        // 'A' = ASCII 65, index = 65 - 32 = 33
        // col = 33 % 16 = 1, row = 33 / 16 = 2
        let (u0, v0, u1, v1) = atlas.glyph_uv('A');
        let expected_u0 = 1.0 * 8.0 / 128.0;
        let expected_v0 = 2.0 * 12.0 / 72.0;
        let expected_u1 = expected_u0 + 8.0 / 128.0;
        let expected_v1 = expected_v0 + 12.0 / 72.0;
        assert!((u0 - expected_u0).abs() < 1e-6, "u0 mismatch: {} vs {}", u0, expected_u0);
        assert!((v0 - expected_v0).abs() < 1e-6, "v0 mismatch: {} vs {}", v0, expected_v0);
        assert!((u1 - expected_u1).abs() < 1e-6);
        assert!((v1 - expected_v1).abs() < 1e-6);
    }
}
