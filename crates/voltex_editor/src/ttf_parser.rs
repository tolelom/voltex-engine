//! Minimal in-house TrueType (`.ttf`) parser.
//!
//! Extracts the tables we need to rasterise glyphs (`head`, `hhea`, `hmtx`,
//! `cmap`, `glyf`/`loca`). Vendored implementation lets us keep the engine
//! dependency-free; missing or unsupported tables fall back gracefully so the
//! editor still loads with a bitmap font.
//!
//! Every table-record field is `pub` so tests can probe the parse, and the
//! set rotates as the parser grows. Allowing `missing_docs` keeps the field
//! churn cheap; user code should reach for [`TtfFont`](crate::TtfFont)
//! instead of this module.
#![allow(missing_docs)]

use std::collections::HashMap;

// --- Byte helpers (big-endian) ---
//
// Fonts are user-supplied files and every offset below comes from them, so
// reads are total: bytes past the end read as 0 (a corrupt font renders
// garbage or blank glyphs instead of panicking the editor).

fn bytes<const N: usize>(data: &[u8], off: usize) -> [u8; N] {
    off.checked_add(N)
        .and_then(|end| data.get(off..end))
        .map_or([0; N], |s| s.try_into().unwrap_or([0; N]))
}

fn read_u8(data: &[u8], off: usize) -> u8 {
    data.get(off).copied().unwrap_or(0)
}

fn read_u16(data: &[u8], off: usize) -> u16 {
    u16::from_be_bytes(bytes(data, off))
}

fn read_i16(data: &[u8], off: usize) -> i16 {
    i16::from_be_bytes(bytes(data, off))
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_be_bytes(bytes(data, off))
}

// --- Data types ---

#[derive(Debug, Clone)]
pub(crate) struct OutlinePoint {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) on_curve: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct GlyphOutline {
    pub(crate) contours: Vec<Vec<OutlinePoint>>,
    pub(crate) x_min: i16,
    pub(crate) y_min: i16,
    pub(crate) x_max: i16,
    pub(crate) y_max: i16,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GlyphMetrics {
    pub(crate) advance_width: u16,
    pub(crate) left_side_bearing: i16,
}

// --- Flag bits for simple glyph parsing ---
const ON_CURVE: u8 = 0x01;
const X_SHORT: u8 = 0x02;
const Y_SHORT: u8 = 0x04;
const REPEAT_FLAG: u8 = 0x08;
const X_SAME_OR_POS: u8 = 0x10;
const Y_SAME_OR_POS: u8 = 0x20;

// --- TtfParser ---

pub(crate) struct TtfParser {
    data: Vec<u8>,
    pub(crate) tables: HashMap<[u8; 4], (u32, u32)>, // tag -> (offset, length)
    pub(crate) units_per_em: u16,
    pub(crate) num_glyphs: u16,
    pub(crate) ascender: i16,
    pub(crate) descender: i16,
    pub(crate) line_gap: i16,
    pub(crate) num_h_metrics: u16,
    pub(crate) loca_format: i16,
}

impl TtfParser {
    /// Parse a TTF file from raw bytes.
    pub fn parse(data: Vec<u8>) -> crate::Result<Self> {
        if data.len() < 12 {
            return Err("File too short for offset table".into());
        }

        let _sf_version = read_u32(&data, 0);
        let num_tables = read_u16(&data, 4) as usize;

        if data.len() < 12 + num_tables * 16 {
            return Err("File too short for table records".into());
        }

        let mut tables = HashMap::new();
        for i in 0..num_tables {
            let rec_off = 12 + i * 16;
            let tag: [u8; 4] = bytes(&data, rec_off);
            let offset = read_u32(&data, rec_off + 8);
            let length = read_u32(&data, rec_off + 12);
            tables.insert(tag, (offset, length));
        }

        // Parse head table
        let &(head_off, _) = tables
            .get(b"head")
            .ok_or("Missing head table")?;
        let head_off = head_off as usize;
        let units_per_em = read_u16(&data, head_off + 18);
        // OpenType 'head': unitsPerEm is 16..=16384. Zero made the glyph
        // scale infinite (NaN outlines, unbounded curve subdivision).
        if !(16..=16384).contains(&units_per_em) {
            return Err(format!("Invalid unitsPerEm {units_per_em}").into());
        }
        let loca_format = read_i16(&data, head_off + 50);

        // Parse hhea table
        let &(hhea_off, _) = tables
            .get(b"hhea")
            .ok_or("Missing hhea table")?;
        let hhea_off = hhea_off as usize;
        let ascender = read_i16(&data, hhea_off + 4);
        let descender = read_i16(&data, hhea_off + 6);
        let line_gap = read_i16(&data, hhea_off + 8);
        let num_h_metrics = read_u16(&data, hhea_off + 34);

        // Parse maxp table
        let &(maxp_off, _) = tables
            .get(b"maxp")
            .ok_or("Missing maxp table")?;
        let maxp_off = maxp_off as usize;
        let num_glyphs = read_u16(&data, maxp_off + 4);

        Ok(Self {
            data,
            tables,
            units_per_em,
            num_glyphs,
            ascender,
            descender,
            line_gap,
            num_h_metrics,
            loca_format,
        })
    }

    /// Look up the glyph index for a Unicode codepoint via cmap Format 4.
    pub fn glyph_index(&self, codepoint: u32) -> u16 {
        let &(cmap_off, _) = match self.tables.get(b"cmap") {
            Some(v) => v,
            None => return 0,
        };
        let cmap_off = cmap_off as usize;
        let num_subtables = read_u16(&self.data, cmap_off + 2) as usize;

        // Find a Format 4 subtable (prefer platform 3 encoding 1, or platform 0)
        let mut fmt4_offset: Option<usize> = None;
        for i in 0..num_subtables {
            let rec = cmap_off + 4 + i * 8;
            let platform_id = read_u16(&self.data, rec);
            let encoding_id = read_u16(&self.data, rec + 2);
            let sub_offset = read_u32(&self.data, rec + 4) as usize;
            let abs_off = cmap_off + sub_offset;

            if abs_off + 2 > self.data.len() {
                continue;
            }
            let format = read_u16(&self.data, abs_off);
            if format == 4 {
                // Prefer Windows Unicode BMP (3,1)
                if platform_id == 3 && encoding_id == 1 {
                    fmt4_offset = Some(abs_off);
                    break;
                }
                // Accept platform 0 as fallback
                if platform_id == 0 && fmt4_offset.is_none() {
                    fmt4_offset = Some(abs_off);
                }
            }
        }

        let sub_off = match fmt4_offset {
            Some(o) => o,
            None => return 0,
        };

        // Parse Format 4
        let seg_count_x2 = read_u16(&self.data, sub_off + 6) as usize;
        let seg_count = seg_count_x2 / 2;

        let end_code_base = sub_off + 14;
        let start_code_base = end_code_base + seg_count * 2 + 2; // +2 for reservedPad
        let id_delta_base = start_code_base + seg_count * 2;
        let id_range_offset_base = id_delta_base + seg_count * 2;

        for i in 0..seg_count {
            let end_code = read_u16(&self.data, end_code_base + i * 2) as u32;
            let start_code = read_u16(&self.data, start_code_base + i * 2) as u32;

            if end_code >= codepoint && start_code <= codepoint {
                let id_delta = read_i16(&self.data, id_delta_base + i * 2);
                let id_range_offset = read_u16(&self.data, id_range_offset_base + i * 2) as usize;

                if id_range_offset == 0 {
                    return (codepoint as i32 + id_delta as i32) as u16;
                } else {
                    let offset_in_bytes =
                        id_range_offset + 2 * (codepoint - start_code) as usize;
                    let glyph_addr = id_range_offset_base + i * 2 + offset_in_bytes;
                    if glyph_addr + 1 < self.data.len() {
                        let glyph = read_u16(&self.data, glyph_addr);
                        if glyph != 0 {
                            return (glyph as i32 + id_delta as i32) as u16;
                        }
                    }
                    return 0;
                }
            }
        }

        0
    }

    /// Get the offset of a glyph in the glyf table using loca.
    fn glyph_offset(&self, glyph_id: u16) -> Option<(usize, usize)> {
        let &(loca_off, _) = self.tables.get(b"loca")?;
        let &(glyf_off, _) = self.tables.get(b"glyf")?;
        let loca_off = loca_off as usize;
        let glyf_off = glyf_off as usize;

        if glyph_id >= self.num_glyphs {
            return None;
        }

        let (offset, next_offset) = if self.loca_format == 0 {
            // Short format: u16 * 2
            let o = read_u16(&self.data, loca_off + glyph_id as usize * 2) as usize * 2;
            let n = read_u16(&self.data, loca_off + (glyph_id as usize + 1) * 2) as usize * 2;
            (o, n)
        } else {
            // Long format: u32
            let o = read_u32(&self.data, loca_off + glyph_id as usize * 4) as usize;
            let n = read_u32(&self.data, loca_off + (glyph_id as usize + 1) * 4) as usize;
            (o, n)
        };

        // Empty glyph (e.g., space) — or a corrupt loca with a backwards range.
        if next_offset <= offset {
            return None;
        }

        Some((glyf_off + offset, next_offset - offset))
    }

    /// Parse the outline of a simple glyph.
    pub fn glyph_outline(&self, glyph_id: u16) -> Option<GlyphOutline> {
        let (glyph_off, _glyph_len) = self.glyph_offset(glyph_id)?;

        let num_contours = read_i16(&self.data, glyph_off);
        if num_contours < 0 {
            // Compound glyph — not supported
            return None;
        }

        let num_contours = num_contours as usize;
        if num_contours == 0 {
            return Some(GlyphOutline {
                contours: Vec::new(),
                x_min: read_i16(&self.data, glyph_off + 2),
                y_min: read_i16(&self.data, glyph_off + 4),
                x_max: read_i16(&self.data, glyph_off + 6),
                y_max: read_i16(&self.data, glyph_off + 8),
            });
        }

        let x_min = read_i16(&self.data, glyph_off + 2);
        let y_min = read_i16(&self.data, glyph_off + 4);
        let x_max = read_i16(&self.data, glyph_off + 6);
        let y_max = read_i16(&self.data, glyph_off + 8);

        // endPtsOfContours
        let mut end_pts = Vec::with_capacity(num_contours);
        let mut off = glyph_off + 10;
        for _ in 0..num_contours {
            end_pts.push(read_u16(&self.data, off) as usize);
            off += 2;
        }

        // Contour end points must strictly increase (the last one sizes every
        // per-point array below); a corrupt glyph is treated as missing.
        if end_pts.windows(2).any(|w| w[1] <= w[0]) {
            return None;
        }
        let num_points = end_pts[num_contours - 1] + 1;

        // Skip instructions
        let instruction_length = read_u16(&self.data, off) as usize;
        off += 2 + instruction_length;

        // Parse flags
        let mut flags = Vec::with_capacity(num_points);
        while flags.len() < num_points {
            let flag = read_u8(&self.data, off);
            off += 1;
            flags.push(flag);
            if flag & REPEAT_FLAG != 0 {
                let repeat_count = read_u8(&self.data, off) as usize;
                off += 1;
                for _ in 0..repeat_count {
                    flags.push(flag);
                }
            }
        }

        // Parse x-coordinates (delta-encoded)
        let mut x_coords = Vec::with_capacity(num_points);
        let mut x: i32 = 0;
        for &flag in flags.iter().take(num_points) {
            if flag & X_SHORT != 0 {
                let dx = read_u8(&self.data, off) as i32;
                off += 1;
                x += if flag & X_SAME_OR_POS != 0 { dx } else { -dx };
            } else if flag & X_SAME_OR_POS != 0 {
                // delta = 0
            } else {
                let dx = read_i16(&self.data, off) as i32;
                off += 2;
                x += dx;
            }
            x_coords.push(x);
        }

        // Parse y-coordinates (delta-encoded)
        let mut y_coords = Vec::with_capacity(num_points);
        let mut y: i32 = 0;
        for &flag in flags.iter().take(num_points) {
            if flag & Y_SHORT != 0 {
                let dy = read_u8(&self.data, off) as i32;
                off += 1;
                y += if flag & Y_SAME_OR_POS != 0 { dy } else { -dy };
            } else if flag & Y_SAME_OR_POS != 0 {
                // delta = 0
            } else {
                let dy = read_i16(&self.data, off) as i32;
                off += 2;
                y += dy;
            }
            y_coords.push(y);
        }

        // Build contours with implicit on-curve point insertion
        let mut contours = Vec::with_capacity(num_contours);
        let mut start = 0;
        for &end in &end_pts {
            let raw_points: Vec<(f32, f32, bool)> = (start..=end)
                .map(|i| {
                    (
                        x_coords[i] as f32,
                        y_coords[i] as f32,
                        flags[i] & ON_CURVE != 0,
                    )
                })
                .collect();

            let mut contour = Vec::new();
            let n = raw_points.len();
            if n == 0 {
                start = end + 1;
                contours.push(contour);
                continue;
            }

            for j in 0..n {
                let (cx, cy, c_on) = raw_points[j];
                let (nx, ny, n_on) = raw_points[(j + 1) % n];

                contour.push(OutlinePoint {
                    x: cx,
                    y: cy,
                    on_curve: c_on,
                });

                // If both current and next are off-curve, insert implicit midpoint
                if !c_on && !n_on {
                    contour.push(OutlinePoint {
                        x: (cx + nx) * 0.5,
                        y: (cy + ny) * 0.5,
                        on_curve: true,
                    });
                }
            }

            start = end + 1;
            contours.push(contour);
        }

        Some(GlyphOutline {
            contours,
            x_min,
            y_min,
            x_max,
            y_max,
        })
    }

    /// Get the horizontal metrics for a glyph.
    pub fn glyph_metrics(&self, glyph_id: u16) -> GlyphMetrics {
        let &(hmtx_off, _) = match self.tables.get(b"hmtx") {
            Some(v) => v,
            None => {
                return GlyphMetrics {
                    advance_width: 0,
                    left_side_bearing: 0,
                }
            }
        };
        let hmtx_off = hmtx_off as usize;

        if glyph_id < self.num_h_metrics {
            let rec = hmtx_off + glyph_id as usize * 4;
            GlyphMetrics {
                advance_width: read_u16(&self.data, rec),
                left_side_bearing: read_i16(&self.data, rec + 2),
            }
        } else {
            // Use last advance_width, lsb from separate array
            let last_aw_off = hmtx_off + (self.num_h_metrics as usize).saturating_sub(1) * 4;
            let advance_width = read_u16(&self.data, last_aw_off);
            let lsb_array_off = hmtx_off + self.num_h_metrics as usize * 4;
            let idx = glyph_id as usize - self.num_h_metrics as usize;
            let left_side_bearing = read_i16(&self.data, lsb_array_off + idx * 2);
            GlyphMetrics {
                advance_width,
                left_side_bearing,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_font() -> Option<TtfParser> {
        let paths = [
            "C:/Windows/Fonts/arial.ttf",
            "C:/Windows/Fonts/consola.ttf",
        ];
        for path in &paths {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(parser) = TtfParser::parse(data) {
                    return Some(parser);
                }
            }
        }
        None
    }

    #[test]
    fn test_parse_loads_tables() {
        let parser = load_test_font().expect("no test font found");
        assert!(parser.tables.contains_key(b"head"));
        assert!(parser.tables.contains_key(b"cmap"));
        assert!(parser.tables.contains_key(b"glyf"));
    }

    #[test]
    fn test_head_values() {
        let parser = load_test_font().expect("no test font found");
        assert!(parser.units_per_em > 0);
        assert!(parser.loca_format == 0 || parser.loca_format == 1);
    }

    #[test]
    fn test_hhea_values() {
        let parser = load_test_font().expect("no test font found");
        assert!(parser.ascender > 0);
        assert!(parser.num_h_metrics > 0);
    }

    #[test]
    fn test_maxp_values() {
        let parser = load_test_font().expect("no test font found");
        assert!(parser.num_glyphs > 0);
    }

    #[test]
    fn test_cmap_ascii() {
        let parser = load_test_font().expect("no test font found");
        let glyph_a = parser.glyph_index(0x41); // 'A'
        assert!(glyph_a > 0, "glyph index for 'A' should be > 0");
    }

    #[test]
    fn test_cmap_space() {
        let parser = load_test_font().expect("no test font found");
        let glyph = parser.glyph_index(0x20); // space
        assert!(glyph > 0);
    }

    #[test]
    fn test_cmap_unmapped() {
        let parser = load_test_font().expect("no test font found");
        let glyph = parser.glyph_index(0xFFFD0); // unlikely codepoint
        assert_eq!(glyph, 0);
    }

    #[test]
    fn test_glyph_outline_has_contours() {
        let parser = load_test_font().expect("no test font found");
        let gid = parser.glyph_index(0x41); // 'A'
        let outline = parser.glyph_outline(gid);
        assert!(outline.is_some());
        let outline = outline.unwrap();
        assert!(!outline.contours.is_empty(), "A should have contours");
    }

    #[test]
    fn test_glyph_metrics() {
        let parser = load_test_font().expect("no test font found");
        let gid = parser.glyph_index(0x41);
        let metrics = parser.glyph_metrics(gid);
        assert!(metrics.advance_width > 0);
    }

    #[test]
    fn test_space_no_contours() {
        let parser = load_test_font().expect("no test font found");
        let gid = parser.glyph_index(0x20);
        let outline = parser.glyph_outline(gid);
        // Space may have no outline or empty contours
        if let Some(o) = outline {
            assert!(o.contours.is_empty());
        }
    }
}
