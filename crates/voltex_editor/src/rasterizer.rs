//! TrueType glyph rasteriser — converts a parsed [`GlyphOutline`] into a
//! grayscale coverage bitmap used by the glyph atlas.
#![allow(missing_docs)]

use crate::ttf_parser::GlyphOutline;

/// Result of rasterizing a single glyph.
pub struct RasterResult {
    pub width: u32,
    pub height: u32,
    /// R8 alpha bitmap, row-major.
    pub bitmap: Vec<u8>,
    /// Left bearing in pixels.
    pub offset_x: f32,
    /// Distance from baseline to top of glyph bbox in pixels.
    pub offset_y: f32,
}

/// Recursively flatten a quadratic bezier into line segments.
pub(crate) fn flatten_quad(
    x0: f32, y0: f32,
    cx: f32, cy: f32,
    x1: f32, y1: f32,
    edges: &mut Vec<(f32, f32, f32, f32)>,
) {
    // Check if curve is flat enough (midpoint distance < 0.5px)
    let mx = (x0 + 2.0 * cx + x1) / 4.0;
    let my = (y0 + 2.0 * cy + y1) / 4.0;
    let lx = (x0 + x1) / 2.0;
    let ly = (y0 + y1) / 2.0;
    let dx = mx - lx;
    let dy = my - ly;
    if dx * dx + dy * dy < 0.25 {
        edges.push((x0, y0, x1, y1));
    } else {
        // Subdivide at t=0.5
        let m01x = (x0 + cx) / 2.0;
        let m01y = (y0 + cy) / 2.0;
        let m12x = (cx + x1) / 2.0;
        let m12y = (cy + y1) / 2.0;
        let midx = (m01x + m12x) / 2.0;
        let midy = (m01y + m12y) / 2.0;
        flatten_quad(x0, y0, m01x, m01y, midx, midy, edges);
        flatten_quad(midx, midy, m12x, m12y, x1, y1, edges);
    }
}

/// Rasterize a glyph outline at the given scale into an alpha bitmap.
pub(crate) fn rasterize(outline: &GlyphOutline, scale: f32) -> RasterResult {
    // Handle empty outline (e.g., space)
    if outline.contours.is_empty() || outline.x_max <= outline.x_min {
        return RasterResult {
            width: 0,
            height: 0,
            bitmap: vec![],
            offset_x: 0.0,
            offset_y: 0.0,
        };
    }

    let x_min = outline.x_min as f32;
    let y_min = outline.y_min as f32;
    let x_max = outline.x_max as f32;
    let y_max = outline.y_max as f32;

    let w = ((x_max - x_min) * scale).ceil() as u32 + 2;
    let h = ((y_max - y_min) * scale).ceil() as u32 + 2;

    // Transform a font-space coordinate to bitmap pixel space.
    let transform = |px: f32, py: f32| -> (f32, f32) {
        let bx = (px - x_min) * scale + 1.0;
        let by = (y_max - py) * scale + 1.0;
        (bx, by)
    };

    // Build edges from contours
    let mut edges: Vec<(f32, f32, f32, f32)> = Vec::new();

    for contour in &outline.contours {
        let n = contour.len();
        if n < 2 {
            continue;
        }
        let mut i = 0;
        while i < n {
            let p0 = &contour[i];
            let p1 = &contour[(i + 1) % n];
            if p0.on_curve && p1.on_curve {
                // Line segment
                let (px0, py0) = transform(p0.x, p0.y);
                let (px1, py1) = transform(p1.x, p1.y);
                edges.push((px0, py0, px1, py1));
                i += 1;
            } else if p0.on_curve && !p1.on_curve {
                // Quadratic bezier: p0 -> p1(control) -> p2(on_curve)
                let p2 = &contour[(i + 2) % n];
                let (px0, py0) = transform(p0.x, p0.y);
                let (cx, cy) = transform(p1.x, p1.y);
                let (px1, py1) = transform(p2.x, p2.y);
                flatten_quad(px0, py0, cx, cy, px1, py1, &mut edges);
                i += 2;
            } else {
                // Skip unexpected off-curve start
                i += 1;
            }
        }
    }

    // Scanline fill (non-zero winding)
    let mut bitmap = vec![0u8; (w * h) as usize];
    for row in 0..h {
        let scan_y = row as f32 + 0.5;
        let mut intersections: Vec<(f32, i32)> = Vec::new();

        for &(x0, y0, x1, y1) in &edges {
            if (y0 <= scan_y && y1 > scan_y) || (y1 <= scan_y && y0 > scan_y) {
                let t = (scan_y - y0) / (y1 - y0);
                let ix = x0 + t * (x1 - x0);
                let dir = if y1 > y0 { 1 } else { -1 };
                intersections.push((ix, dir));
            }
        }

        intersections.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        // Fill using non-zero winding rule
        let mut winding = 0i32;
        let mut fill_start = 0.0f32;
        for &(x, dir) in &intersections {
            let old_winding = winding;
            winding += dir;
            if old_winding == 0 && winding != 0 {
                fill_start = x;
            }
            if old_winding != 0 && winding == 0 {
                let px_start = (fill_start.floor() as i32).max(0) as u32;
                let px_end = (x.ceil() as u32).min(w);
                for px in px_start..px_end {
                    bitmap[(row * w + px) as usize] = 255;
                }
            }
        }
    }

    let offset_x = x_min * scale;
    let offset_y = y_max * scale;

    RasterResult {
        width: w,
        height: h,
        bitmap,
        offset_x,
        offset_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ttf_parser::TtfParser;

    fn load_test_font() -> Option<TtfParser> {
        let paths = [
            "C:/Windows/Fonts/arial.ttf",
            "C:/Windows/Fonts/consola.ttf",
        ];
        for p in &paths {
            if let Ok(data) = std::fs::read(p) {
                if let Ok(parser) = TtfParser::parse(data) {
                    return Some(parser);
                }
            }
        }
        None
    }

    #[test]
    fn test_rasterize_produces_bitmap() {
        let parser = load_test_font().expect("no font");
        let gid = parser.glyph_index(0x41); // 'A'
        let outline = parser.glyph_outline(gid).expect("no outline");
        let scale = 32.0 / parser.units_per_em as f32;
        let result = rasterize(&outline, scale);
        assert!(result.width > 0);
        assert!(result.height > 0);
        assert!(!result.bitmap.is_empty());
    }

    #[test]
    fn test_rasterize_has_filled_pixels() {
        let parser = load_test_font().expect("no font");
        let gid = parser.glyph_index(0x41);
        let outline = parser.glyph_outline(gid).expect("no outline");
        let scale = 32.0 / parser.units_per_em as f32;
        let result = rasterize(&outline, scale);
        let filled = result.bitmap.iter().filter(|&&b| b > 0).count();
        assert!(filled > 0, "bitmap should have filled pixels");
    }

    #[test]
    fn test_rasterize_empty() {
        let parser = load_test_font().expect("no font");
        let gid = parser.glyph_index(0x20); // space
        let outline = parser.glyph_outline(gid);
        if let Some(o) = outline {
            let scale = 32.0 / parser.units_per_em as f32;
            let result = rasterize(&o, scale);
            // Space should be empty or zero-sized
            assert!(
                result.width == 0 || result.bitmap.iter().all(|&b| b == 0)
            );
        }
    }

    #[test]
    fn test_flatten_quad_produces_edges() {
        let mut edges = Vec::new();
        flatten_quad(0.0, 0.0, 5.0, 10.0, 10.0, 0.0, &mut edges);
        assert!(
            edges.len() >= 2,
            "bezier should flatten to multiple segments"
        );
    }
}
