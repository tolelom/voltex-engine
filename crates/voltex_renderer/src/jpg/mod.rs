//! Baseline JPEG decoder. Supports SOF0 (sequential DCT, Huffman).
//! Returns RGBA pixel data like parse_png.
//! Supports grayscale (1-component) and YCbCr (3-component) with
//! chroma subsampling (4:4:4, 4:2:2, 4:2:0).

mod headers;
mod bit_reader;
mod idct;
mod decoder;
mod mcu;

use headers::{HuffTable, JpegComponent, parse_dqt, parse_sof, parse_dht};
use decoder::decode_scan;

pub fn parse_jpg(data: &[u8]) -> crate::Result<(Vec<u8>, u32, u32)> {
    if data.len() < 2 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err("Invalid JPEG: missing SOI marker".into());
    }

    let mut pos = 2;
    let mut width: u16 = 0;
    let mut height: u16 = 0;
    let mut num_components: u8 = 0;
    let mut components: Vec<JpegComponent> = Vec::new();
    let mut qt_tables: [[u16; 64]; 4] = [[0; 64]; 4];
    let mut dc_tables: [Option<HuffTable>; 4] = [None, None, None, None];
    let mut ac_tables: [Option<HuffTable>; 4] = [None, None, None, None];
    let mut found_sof = false;
    let mut restart_interval: u16 = 0;

    while pos + 1 < data.len() {
        if data[pos] != 0xFF {
            return Err(format!("Expected marker at position {}", pos).into());
        }
        // Skip padding 0xFF bytes
        while pos + 1 < data.len() && data[pos + 1] == 0xFF {
            pos += 1;
        }
        if pos + 1 >= data.len() {
            return Err("Unexpected end of data".into());
        }
        let marker = data[pos + 1];
        pos += 2;

        match marker {
            0xD8 => {} // SOI (already handled)
            0xD9 => break, // EOI
            0xDA => {
                // SOS — Start of Scan
                if !found_sof {
                    return Err("SOS before SOF".into());
                }
                let (rgb, scan_end) = decode_scan(
                    data, pos, width, height, num_components,
                    &components, &qt_tables, &dc_tables, &ac_tables,
                    restart_interval,
                )?;
                let _ = scan_end;
                // Convert RGB to RGBA
                let w = width as u32;
                let h = height as u32;
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                for pixel in rgb.chunks_exact(3) {
                    rgba.push(pixel[0]);
                    rgba.push(pixel[1]);
                    rgba.push(pixel[2]);
                    rgba.push(255);
                }
                return Ok((rgba, w, h));
            }
            0xC0 => {
                // SOF0 — Baseline DCT
                let (sof, len) = parse_sof(data, pos)?;
                width = sof.width;
                height = sof.height;
                num_components = sof.num_components;
                components = sof.components;
                found_sof = true;
                pos += len;
            }
            0xC4 => {
                // DHT — Define Huffman Table
                let len = parse_dht(data, pos, &mut dc_tables, &mut ac_tables)?;
                pos += len;
            }
            0xDB => {
                // DQT — Define Quantization Table
                let len = parse_dqt(data, pos, &mut qt_tables)?;
                pos += len;
            }
            0xDD => {
                // DRI — Define Restart Interval
                if pos + 4 > data.len() {
                    return Err("DRI too short".into());
                }
                let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                restart_interval =
                    u16::from_be_bytes([data[pos + 2], data[pos + 3]]);
                pos += seg_len;
            }
            0xD0..=0xD7 => {
                // RST markers — handled inside scan decoder
            }
            0xE0..=0xEF | 0xFE => {
                // APP0-APP15, COM — skip
                if pos + 2 > data.len() {
                    return Err("Segment too short".into());
                }
                let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                pos += seg_len;
            }
            _ => {
                // Unknown marker with length — skip
                if pos + 2 > data.len() {
                    return Err(format!("Unknown marker 0x{:02X}", marker).into());
                }
                let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                pos += seg_len;
            }
        }
    }

    Err("No image data found (missing SOS)".into())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use super::headers::{parse_dqt, parse_dht, HuffTable};
    use super::bit_reader::BitReader;
    use super::idct::idct;

    #[test]
    fn test_invalid_signature() {
        let data = [0u8; 10];
        assert!(parse_jpg(&data).is_err());
    }

    #[test]
    fn test_empty_data() {
        assert!(parse_jpg(&[]).is_err());
    }

    #[test]
    fn test_soi_only() {
        let data = [0xFF, 0xD8];
        assert!(parse_jpg(&data).is_err());
    }

    #[test]
    fn test_parse_dqt_8bit() {
        let mut seg = Vec::new();
        seg.extend_from_slice(&67u16.to_be_bytes()); // length = 67
        seg.push(0x00); // precision=0 (8-bit), table_id=0
        for i in 0..64u8 {
            seg.push(i + 1);
        }
        let mut qt_tables = [[0u16; 64]; 4];
        let len = parse_dqt(&seg, 0, &mut qt_tables).unwrap();
        assert_eq!(len, 67);
        assert_eq!(qt_tables[0][0], 1);
        assert_eq!(qt_tables[0][63], 64);
    }

    #[test]
    fn test_parse_dht() {
        let mut seg = Vec::new();
        let mut body = Vec::new();
        body.push(0x00); // class=0 (DC), id=0
        // counts: 1 symbol at length 1, 0 for lengths 2-16
        body.push(1);
        // Zero counts for lengths 2-16 (JPEG DHT format requires 16 length entries).
        body.extend_from_slice(&[0u8; 15]);
        body.push(0x05); // the symbol

        seg.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
        seg.extend_from_slice(&body);

        let mut dc_tables: [Option<HuffTable>; 4] = [None, None, None, None];
        let mut ac_tables: [Option<HuffTable>; 4] = [None, None, None, None];
        let len = parse_dht(&seg, 0, &mut dc_tables, &mut ac_tables).unwrap();
        assert_eq!(len, seg.len());
        assert!(dc_tables[0].is_some());
        let table = dc_tables[0].as_ref().unwrap();
        assert_eq!(table.symbols[0], 0x05);
    }

    #[test]
    fn test_bit_reader_basic() {
        // 0xA5 = 10100101
        let data = [0xA5];
        let mut reader = BitReader::new(&data, 0);
        assert_eq!(reader.read_bits(1).unwrap(), 1);
        assert_eq!(reader.read_bits(1).unwrap(), 0);
        assert_eq!(reader.read_bits(3).unwrap(), 0b100);
        assert_eq!(reader.read_bits(3).unwrap(), 0b101);
    }

    #[test]
    fn test_bit_reader_byte_stuffing() {
        // JPEG byte stuffing: 0xFF 0x00 -> single 0xFF byte
        let data = [0xFF, 0x00, 0x80];
        let mut reader = BitReader::new(&data, 0);
        let val = reader.read_bits(8).unwrap();
        assert_eq!(val, 0xFF);
        let val2 = reader.read_bits(1).unwrap();
        assert_eq!(val2, 1); // 0x80 = 10000000
    }

    #[test]
    fn test_idct_dc_only() {
        let mut block = [0i32; 64];
        block[0] = 800; // after dequantization
        let result = idct(&block);
        let expected = 100; // 800/8 = 100
        for &v in &result {
            assert!(
                (v - expected).abs() <= 1,
                "DC-only IDCT: expected ~{}, got {}",
                expected,
                v
            );
        }
    }

    #[test]
    fn test_idct_known_values() {
        let mut block = [0i32; 64];
        block[0] = 640;
        block[1] = 100;
        let result = idct(&block);
        let avg: i32 = result.iter().sum::<i32>() / 64;
        assert!((avg - 80).abs() <= 2);
    }

    /// Build a minimal valid 8x8 Baseline JPEG (grayscale) for testing.
    /// DC diff = 0 => Y = 0 => after +128 level shift => pixel = 128.
    fn build_minimal_jpeg_8x8() -> Vec<u8> {
        let mut out = Vec::new();

        // SOI
        out.extend_from_slice(&[0xFF, 0xD8]);

        // DQT — all-ones quantization table (id=0)
        out.extend_from_slice(&[0xFF, 0xDB]);
        let mut dqt = Vec::new();
        dqt.extend_from_slice(&0x0043u16.to_be_bytes()); // length = 67
        dqt.push(0x00); // 8-bit, table 0
        // 64-entry quantization table, all values = 1.
        dqt.extend_from_slice(&[1u8; 64]);
        out.extend_from_slice(&dqt);

        // SOF0 — 8x8, 1 component (grayscale)
        out.extend_from_slice(&[0xFF, 0xC0]);
        out.extend_from_slice(&0x000Bu16.to_be_bytes()); // length = 11
        out.push(8); // precision
        out.extend_from_slice(&8u16.to_be_bytes()); // height
        out.extend_from_slice(&8u16.to_be_bytes()); // width
        out.push(1); // 1 component
        out.push(1); // component ID
        out.push(0x11); // h_sample=1, v_sample=1
        out.push(0); // qt table 0

        // DHT — DC table (class=0, id=0): 1 symbol at length 1, symbol = 0x00
        out.extend_from_slice(&[0xFF, 0xC4]);
        let mut dht_body = Vec::new();
        dht_body.push(0x00); // DC, id=0
        dht_body.push(1); // 1 symbol at length 1
        // Zero counts for lengths 2-16.
        dht_body.extend_from_slice(&[0u8; 15]);
        dht_body.push(0x00); // symbol: category 0 (DC diff = 0)
        let dht_len = (dht_body.len() + 2) as u16;
        out.extend_from_slice(&dht_len.to_be_bytes());
        out.extend_from_slice(&dht_body);

        // DHT — AC table (class=1, id=0): 1 symbol at length 1, symbol = 0x00 (EOB)
        out.extend_from_slice(&[0xFF, 0xC4]);
        let mut dht_ac = Vec::new();
        dht_ac.push(0x10); // AC, id=0
        dht_ac.push(1); // 1 symbol at length 1
        // Zero counts for lengths 2-16.
        dht_ac.extend_from_slice(&[0u8; 15]);
        dht_ac.push(0x00); // symbol: 0x00 = EOB
        let dht_ac_len = (dht_ac.len() + 2) as u16;
        out.extend_from_slice(&dht_ac_len.to_be_bytes());
        out.extend_from_slice(&dht_ac);

        // SOS
        out.extend_from_slice(&[0xFF, 0xDA]);
        out.extend_from_slice(&0x0008u16.to_be_bytes()); // length=8
        out.push(1); // 1 component
        out.push(1); // component id=1
        out.push(0x00); // DC table 0, AC table 0
        out.push(0); // Ss
        out.push(63); // Se
        out.push(0); // Ah=0, Al=0

        // Scan data: DC=0 (code=0, 1 bit), AC=EOB (code=0, 1 bit)
        // Bits: 0 (DC diff=0) + 0 (EOB) = 0b00 -> padded to byte: 0x00
        out.push(0x00);
        out.push(0x00);

        // EOI
        out.extend_from_slice(&[0xFF, 0xD9]);

        out
    }

    #[test]
    fn test_grayscale_flat() {
        let jpg_data = build_minimal_jpeg_8x8();
        let (rgba, w, h) = parse_jpg(&jpg_data).unwrap();
        assert_eq!(w, 8);
        assert_eq!(h, 8);
        assert_eq!(rgba.len(), 8 * 8 * 4);
        // Grayscale mid-gray: all pixels should be ~128
        for i in (0..rgba.len()).step_by(4) {
            assert_eq!(rgba[i], rgba[i + 1]); // R == G
            assert_eq!(rgba[i + 1], rgba[i + 2]); // G == B
            assert_eq!(rgba[i + 3], 255); // alpha
        }
    }

    #[test]
    fn test_invalid_marker() {
        let data = [0xFF, 0xD8, 0x00]; // SOI then garbage
        assert!(parse_jpg(&data).is_err());
    }

    // ── Panic-resistance fuzz tests ────────────────────────────────
    //
    // parse_jpg must return Result on any input. JPEG parsers are
    // notoriously panic-prone (Huffman tables, RST markers, MCU
    // boundaries) — this catches index/slice/unwrap panics from
    // arbitrary input.

    fn fuzz_bytes(seed: u32, len: usize) -> Vec<u8> {
        let mut s: u32 = if seed == 0 { 0xDEAD_BEEF } else { seed };
        (0..len)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s & 0xFF) as u8
            })
            .collect()
    }

    #[test]
    fn fuzz_parse_jpg_no_panic_on_empty() {
        let _ = parse_jpg(&[]);
    }

    #[test]
    fn fuzz_parse_jpg_no_panic_on_soi_only() {
        let _ = parse_jpg(&[0xFF, 0xD8]);
        let _ = parse_jpg(&[0xFF]);
    }

    #[test]
    fn fuzz_parse_jpg_no_panic_on_random_bytes() {
        for seed in 1..=128u32 {
            for &len in &[0usize, 1, 2, 8, 64, 256, 1024] {
                let _ = parse_jpg(&fuzz_bytes(seed, len));
            }
        }
    }

    #[test]
    fn fuzz_parse_jpg_no_panic_on_soi_random_tail() {
        for seed in 1..=128u32 {
            for &tail_len in &[0usize, 1, 16, 64, 256, 1024] {
                let mut data = vec![0xFF, 0xD8];
                data.extend_from_slice(&fuzz_bytes(seed, tail_len));
                let _ = parse_jpg(&data);
            }
        }
    }
}
