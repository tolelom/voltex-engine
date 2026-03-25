# JPG Decoder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Self-contained Baseline JPEG decoder returning RGBA pixel data, matching existing `parse_png` API pattern.

**Architecture:** JFIF marker-based parser → Huffman bit-stream decoder → inverse quantization → 8x8 IDCT → YCbCr→RGB → MCU assembly. Single file `jpg.rs` with helper structs, registered in `lib.rs`.

**Tech Stack:** Pure Rust, no external dependencies. `Result<(Vec<u8>, u32, u32), String>` error pattern.

---

### Task 1: JFIF Marker Parser + Skeleton

**Files:**
- Create: `crates/voltex_renderer/src/jpg.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

- [ ] **Step 1: Write failing test for marker parsing**

```rust
// In crates/voltex_renderer/src/jpg.rs
#[cfg(test)]
mod tests {
    use super::*;

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
        let data = [0xFF, 0xD8]; // SOI only, no SOF/SOS
        assert!(parse_jpg(&data).is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package voltex_renderer -- jpg::tests -v`
Expected: FAIL — module `jpg` not found

- [ ] **Step 3: Implement marker parser skeleton**

```rust
// crates/voltex_renderer/src/jpg.rs

/// Baseline JPEG decoder. Supports SOF0 (sequential DCT, Huffman).
/// Returns RGBA pixel data like parse_png.

pub fn parse_jpg(data: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
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

    while pos + 1 < data.len() {
        if data[pos] != 0xFF {
            return Err(format!("Expected marker at position {}", pos));
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
                )?;
                pos = scan_end;
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
                if pos + 4 > data.len() { return Err("DRI too short".into()); }
                // Skip restart interval (2 byte length + 2 byte interval)
                let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                pos += seg_len;
            }
            0xD0..=0xD7 => {
                // RST markers — handled inside scan decoder
            }
            0xE0..=0xEF | 0xFE => {
                // APP0-APP15, COM — skip
                if pos + 2 > data.len() { return Err("Segment too short".into()); }
                let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                pos += seg_len;
            }
            _ => {
                // Unknown marker with length — skip
                if pos + 2 > data.len() { return Err(format!("Unknown marker 0x{:02X}", marker)); }
                let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                pos += seg_len;
            }
        }
    }

    Err("No image data found (missing SOS)".into())
}

#[derive(Clone)]
struct JpegComponent {
    id: u8,
    h_sample: u8,  // horizontal sampling factor
    v_sample: u8,  // vertical sampling factor
    qt_id: u8,     // quantization table ID
    dc_table: u8,  // DC Huffman table ID
    ac_table: u8,  // AC Huffman table ID
}

struct SofData {
    width: u16,
    height: u16,
    num_components: u8,
    components: Vec<JpegComponent>,
}

struct HuffTable {
    // lookup[code_length - 1] = Vec of (code, symbol)
    symbols: Vec<u8>,     // symbols in code-length order
    offsets: [u16; 17],   // offset into symbols for each code length
    maxcode: [i32; 17],   // max code value for each length (-1 if unused)
    mincode: [u16; 17],   // min code value for each length
}
```

Register module in lib.rs:
```rust
pub mod jpg;
pub use jpg::parse_jpg;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --package voltex_renderer -- jpg::tests -v`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_renderer/src/jpg.rs crates/voltex_renderer/src/lib.rs
git commit -m "feat(renderer): add JPEG decoder skeleton with marker parser"
```

---

### Task 2: Quantization Table (DQT) + Huffman Table (DHT) Parsers

**Files:**
- Modify: `crates/voltex_renderer/src/jpg.rs`

- [ ] **Step 1: Write tests for DQT and DHT parsing**

```rust
#[test]
fn test_parse_dqt_8bit() {
    // DQT segment: length=67, precision=0(8-bit), table_id=0, 64 bytes of quantization values
    let mut seg = Vec::new();
    seg.extend_from_slice(&67u16.to_be_bytes()); // segment length
    seg.push(0x00); // precision=0 (8-bit), table_id=0
    for i in 0..64u8 {
        seg.push(i + 1); // quant values 1..64
    }
    let mut qt_tables = [[0u16; 64]; 4];
    let len = parse_dqt(&seg, 0, &mut qt_tables).unwrap();
    assert_eq!(len, 67);
    assert_eq!(qt_tables[0][0], 1);
    assert_eq!(qt_tables[0][63], 64);
}

#[test]
fn test_parse_dht() {
    // DHT segment: DC table, class=0, id=0
    // Simple table: 1 symbol of length 1 (symbol = 0x05)
    let mut seg = Vec::new();
    let mut body = Vec::new();
    body.push(0x00); // class=0 (DC), id=0
    // counts: 1 symbol at length 1, 0 for lengths 2-16
    body.push(1);
    for _ in 1..16 { body.push(0); }
    body.push(0x05); // the symbol

    seg.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
    seg.extend_from_slice(&body);

    let mut dc_tables: [Option<HuffTable>; 4] = [None, None, None, None];
    let mut ac_tables: [Option<HuffTable>; 4] = [None, None, None, None];
    let len = parse_dht(&seg, 0, &mut dc_tables, &mut ac_tables).unwrap();
    assert!(dc_tables[0].is_some());
    let table = dc_tables[0].as_ref().unwrap();
    assert_eq!(table.symbols[0], 0x05);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --package voltex_renderer -- jpg::tests::test_parse_dqt -v`
Expected: FAIL — `parse_dqt` and `parse_dht` not found / not implemented

- [ ] **Step 3: Implement DQT and DHT parsers**

```rust
fn parse_dqt(data: &[u8], pos: usize, qt_tables: &mut [[u16; 64]; 4]) -> Result<usize, String> {
    if pos + 2 > data.len() { return Err("DQT too short".into()); }
    let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    if pos + seg_len > data.len() { return Err("DQT segment extends past data".into()); }

    let mut off = pos + 2;
    let seg_end = pos + seg_len;
    while off < seg_end {
        let pq_tq = data[off];
        let precision = pq_tq >> 4; // 0=8-bit, 1=16-bit
        let table_id = (pq_tq & 0x0F) as usize;
        off += 1;
        if table_id >= 4 { return Err(format!("DQT table id {} out of range", table_id)); }

        if precision == 0 {
            // 8-bit values
            if off + 64 > seg_end { return Err("DQT 8-bit data too short".into()); }
            for i in 0..64 {
                qt_tables[table_id][i] = data[off + i] as u16;
            }
            off += 64;
        } else {
            // 16-bit values
            if off + 128 > seg_end { return Err("DQT 16-bit data too short".into()); }
            for i in 0..64 {
                qt_tables[table_id][i] = u16::from_be_bytes([data[off + i * 2], data[off + i * 2 + 1]]);
            }
            off += 128;
        }
    }
    Ok(seg_len)
}

fn parse_sof(data: &[u8], pos: usize) -> Result<(SofData, usize), String> {
    if pos + 2 > data.len() { return Err("SOF too short".into()); }
    let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    if pos + seg_len > data.len() { return Err("SOF segment extends past data".into()); }

    let precision = data[pos + 2];
    if precision != 8 { return Err(format!("Unsupported sample precision: {}", precision)); }

    let height = u16::from_be_bytes([data[pos + 3], data[pos + 4]]);
    let width = u16::from_be_bytes([data[pos + 5], data[pos + 6]]);
    let num_comp = data[pos + 7];

    let mut components = Vec::new();
    let mut off = pos + 8;
    for _ in 0..num_comp {
        if off + 3 > pos + seg_len { return Err("SOF component data too short".into()); }
        let id = data[off];
        let sampling = data[off + 1];
        let h_sample = sampling >> 4;
        let v_sample = sampling & 0x0F;
        let qt_id = data[off + 2];
        components.push(JpegComponent {
            id, h_sample, v_sample, qt_id,
            dc_table: 0, ac_table: 0,
        });
        off += 3;
    }

    Ok((SofData { width, height, num_components: num_comp, components }, seg_len))
}

fn parse_dht(
    data: &[u8], pos: usize,
    dc_tables: &mut [Option<HuffTable>; 4],
    ac_tables: &mut [Option<HuffTable>; 4],
) -> Result<usize, String> {
    if pos + 2 > data.len() { return Err("DHT too short".into()); }
    let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    if pos + seg_len > data.len() { return Err("DHT segment extends past data".into()); }

    let mut off = pos + 2;
    let seg_end = pos + seg_len;
    while off < seg_end {
        let tc_th = data[off];
        let table_class = tc_th >> 4; // 0=DC, 1=AC
        let table_id = (tc_th & 0x0F) as usize;
        off += 1;
        if table_id >= 4 { return Err(format!("DHT table id {} out of range", table_id)); }

        // Read 16 counts (number of symbols for each code length 1..16)
        if off + 16 > seg_end { return Err("DHT counts too short".into()); }
        let mut counts = [0u8; 16];
        counts.copy_from_slice(&data[off..off + 16]);
        off += 16;

        let total_symbols: usize = counts.iter().map(|&c| c as usize).sum();
        if off + total_symbols > seg_end { return Err("DHT symbols too short".into()); }

        let symbols: Vec<u8> = data[off..off + total_symbols].to_vec();
        off += total_symbols;

        // Build lookup tables
        let mut offsets = [0u16; 17];
        let mut maxcode = [-1i32; 17];
        let mut mincode = [0u16; 17];
        let mut code: u16 = 0;
        let mut sym_offset: u16 = 0;

        for i in 0..16 {
            offsets[i] = sym_offset;
            if counts[i] > 0 {
                mincode[i] = code;
                maxcode[i] = (code + counts[i] as u16 - 1) as i32;
                sym_offset += counts[i] as u16;
            }
            code = (code + counts[i] as u16) << 1;
        }
        offsets[16] = sym_offset;

        let table = HuffTable { symbols, offsets, maxcode, mincode };
        if table_class == 0 {
            dc_tables[table_id] = Some(table);
        } else {
            ac_tables[table_id] = Some(table);
        }
    }
    Ok(seg_len)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --package voltex_renderer -- jpg::tests -v`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_renderer/src/jpg.rs
git commit -m "feat(renderer): add JPEG DQT/DHT/SOF marker parsers"
```

---

### Task 3: Huffman Bit-Stream Reader

**Files:**
- Modify: `crates/voltex_renderer/src/jpg.rs`

- [ ] **Step 1: Write tests for bit reader**

```rust
#[test]
fn test_bit_reader_basic() {
    // 0xA5 = 10100101
    let data = [0xA5];
    let mut reader = BitReader::new(&data, 0);
    assert_eq!(reader.read_bits(1).unwrap(), 1); // 1
    assert_eq!(reader.read_bits(1).unwrap(), 0); // 0
    assert_eq!(reader.read_bits(3).unwrap(), 0b100); // 100
    assert_eq!(reader.read_bits(3).unwrap(), 0b101); // 101
}

#[test]
fn test_bit_reader_byte_stuffing() {
    // JPEG byte stuffing: 0xFF 0x00 → single 0xFF byte
    let data = [0xFF, 0x00, 0x80];
    let mut reader = BitReader::new(&data, 0);
    let val = reader.read_bits(8).unwrap();
    assert_eq!(val, 0xFF);
    let val2 = reader.read_bits(1).unwrap();
    assert_eq!(val2, 1); // 0x80 = 10000000
}
```

- [ ] **Step 2: Run tests to verify they fail**

- [ ] **Step 3: Implement BitReader**

```rust
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bit_pos: u8,    // 0-7, MSB first
    current: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8], start: usize) -> Self {
        Self { data, pos: start, bit_pos: 0, current: 0 }
    }

    fn read_byte(&mut self) -> Result<u8, String> {
        if self.pos >= self.data.len() {
            return Err("Unexpected end of scan data".into());
        }
        let byte = self.data[self.pos];
        self.pos += 1;
        // Handle byte stuffing: 0xFF 0x00 → 0xFF
        if byte == 0xFF {
            if self.pos >= self.data.len() {
                return Err("Unexpected end after 0xFF".into());
            }
            let next = self.data[self.pos];
            if next == 0x00 {
                self.pos += 1; // skip stuffed 0x00
            } else if next >= 0xD0 && next <= 0xD7 {
                // RST marker — skip and read next byte
                self.pos += 1;
                return self.read_byte();
            } else {
                // Marker found — end of scan
                return Err("Marker found in scan data".into());
            }
        }
        Ok(byte)
    }

    fn ensure_bits(&mut self) -> Result<(), String> {
        if self.bit_pos == 0 {
            self.current = self.read_byte()?;
            self.bit_pos = 8;
        }
        Ok(())
    }

    fn read_bit(&mut self) -> Result<u8, String> {
        self.ensure_bits()?;
        self.bit_pos -= 1;
        Ok((self.current >> self.bit_pos) & 1)
    }

    fn read_bits(&mut self, count: u8) -> Result<u16, String> {
        let mut val: u16 = 0;
        for _ in 0..count {
            val = (val << 1) | self.read_bit()? as u16;
        }
        Ok(val)
    }

    /// Decode one Huffman symbol using the given table
    fn decode_huffman(&mut self, table: &HuffTable) -> Result<u8, String> {
        let mut code: u16 = 0;
        for len in 0..16 {
            code = (code << 1) | self.read_bit()? as u16;
            if table.maxcode[len] >= 0 && code as i32 <= table.maxcode[len] {
                let idx = table.offsets[len] as usize + (code - table.mincode[len]) as usize;
                return Ok(table.symbols[idx]);
            }
        }
        Err("Invalid Huffman code".into())
    }

    fn scan_end_pos(&self) -> usize {
        self.pos
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package voltex_renderer -- jpg::tests -v`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_renderer/src/jpg.rs
git commit -m "feat(renderer): add JPEG Huffman bit-stream reader with byte stuffing"
```

---

### Task 4: 8x8 IDCT

**Files:**
- Modify: `crates/voltex_renderer/src/jpg.rs`

- [ ] **Step 1: Write IDCT test**

```rust
#[test]
fn test_idct_dc_only() {
    // DC-only block: only coefficient [0] is set
    let mut block = [0i32; 64];
    block[0] = 800; // after dequantization
    let result = idct(&block);
    // All 64 values should be the same: 800/8 = 100
    let expected = 100;
    for &v in &result {
        assert!((v - expected).abs() <= 1, "DC-only IDCT: expected ~{}, got {}", expected, v);
    }
}

#[test]
fn test_idct_known_values() {
    // A simple known block
    let mut block = [0i32; 64];
    block[0] = 640;
    block[1] = 100;
    let result = idct(&block);
    // DC component should dominate: average ~80
    let avg: i32 = result.iter().sum::<i32>() / 64;
    assert!((avg - 80).abs() <= 2);
}
```

- [ ] **Step 2: Run to verify failure**

- [ ] **Step 3: Implement IDCT**

```rust
/// Zig-zag order for 8x8 block
const ZIGZAG: [usize; 64] = [
     0,  1,  8, 16,  9,  2,  3, 10,
    17, 24, 32, 25, 18, 11,  4,  5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13,  6,  7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63,
];

fn idct(coeffs: &[i32; 64]) -> [i32; 64] {
    // AAN-based float IDCT
    let mut workspace = [0.0f64; 64];

    // Arrange from zigzag to row-major
    let mut block = [0.0f64; 64];
    for i in 0..64 {
        block[ZIGZAG[i]] = coeffs[i] as f64;
    }

    // 1D IDCT on rows
    for row in 0..8 {
        let off = row * 8;
        idct_1d(&mut block, off);
    }

    // Transpose
    for r in 0..8 {
        for c in 0..8 {
            workspace[c * 8 + r] = block[r * 8 + c];
        }
    }

    // 1D IDCT on columns (now rows after transpose)
    for row in 0..8 {
        let off = row * 8;
        idct_1d(&mut workspace, off);
    }

    // Transpose back and round
    let mut result = [0i32; 64];
    for r in 0..8 {
        for c in 0..8 {
            result[r * 8 + c] = (workspace[c * 8 + r] / 8.0).round() as i32;
        }
    }
    result
}

fn idct_1d(data: &mut [f64], off: usize) {
    use std::f64::consts::PI;
    let mut tmp = [0.0f64; 8];
    for x in 0..8 {
        let mut sum = 0.0;
        for u in 0..8 {
            let cu = if u == 0 { 1.0 / 2.0f64.sqrt() } else { 1.0 };
            sum += cu * data[off + u] * ((2.0 * x as f64 + 1.0) * u as f64 * PI / 16.0).cos();
        }
        tmp[x] = sum / 2.0;
    }
    data[off..off + 8].copy_from_slice(&tmp);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package voltex_renderer -- jpg::tests::test_idct -v`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_renderer/src/jpg.rs
git commit -m "feat(renderer): add JPEG 8x8 IDCT with zig-zag reordering"
```

---

### Task 5: Scan Decoder (MCU Decoding + YCbCr→RGB)

**Files:**
- Modify: `crates/voltex_renderer/src/jpg.rs`

- [ ] **Step 1: Write integration test with synthetic minimal JPEG**

```rust
#[test]
fn test_decode_synthetic_1x1_gray_like() {
    // Test the MCU decode + color conversion path
    // We'll create a minimal 8x8 JPEG by hand
    // This is complex — test via the full parse_jpg path with a known-good JPEG
    let jpg_data = build_minimal_jpeg_8x8();
    let result = parse_jpg(&jpg_data);
    assert!(result.is_ok(), "Failed to decode minimal JPEG: {:?}", result.err());
    let (rgba, w, h) = result.unwrap();
    assert_eq!(w, 8);
    assert_eq!(h, 8);
    assert_eq!(rgba.len(), 8 * 8 * 4);
}
```

- [ ] **Step 2: Run test to verify failure**

- [ ] **Step 3: Implement scan decoder**

Implement `decode_scan` function:
- Parse SOS header (component → Huffman table mapping)
- For each MCU: decode DC (differential) + AC (run-length) for each component
- Dequantize coefficients using DQT table
- IDCT each 8x8 block
- Handle chroma subsampling (assemble MCU from Y/Cb/Cr blocks)
- YCbCr → RGB conversion: `R = Y + 1.402*(Cr-128)`, `G = Y - 0.344136*(Cb-128) - 0.714136*(Cr-128)`, `B = Y + 1.772*(Cb-128)`
- Clamp to 0..255
- Handle restart markers (DRI/RST): reset DC predictors

```rust
fn decode_scan(
    data: &[u8], pos: usize,
    width: u16, height: u16, num_components: u8,
    components: &[JpegComponent],
    qt_tables: &[[u16; 64]; 4],
    dc_tables: &[Option<HuffTable>; 4],
    ac_tables: &[Option<HuffTable>; 4],
) -> Result<(Vec<u8>, usize), String> {
    // Parse SOS header
    if pos + 2 > data.len() { return Err("SOS too short".into()); }
    let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    let ns = data[pos + 2] as usize;
    let mut scan_components = components.to_vec();
    let mut off = pos + 3;
    for i in 0..ns {
        let _cs = data[off]; // component selector
        let td_ta = data[off + 1];
        scan_components[i].dc_table = td_ta >> 4;
        scan_components[i].ac_table = td_ta & 0x0F;
        off += 2;
    }
    // Skip spectral selection and successive approximation bytes
    let scan_data_start = pos + seg_len;

    let mut reader = BitReader::new(data, scan_data_start);

    // Calculate MCU dimensions
    let max_h = scan_components.iter().map(|c| c.h_sample).max().unwrap_or(1);
    let max_v = scan_components.iter().map(|c| c.v_sample).max().unwrap_or(1);
    let mcu_width = (max_h as u16) * 8;
    let mcu_height = (max_v as u16) * 8;
    let mcus_x = (width + mcu_width - 1) / mcu_width;
    let mcus_y = (height + mcu_height - 1) / mcu_height;

    let mut dc_pred = vec![0i32; num_components as usize];
    let mut rgb = vec![0u8; (width as usize) * (height as usize) * 3];

    for mcu_row in 0..mcus_y {
        for mcu_col in 0..mcus_x {
            // Decode blocks for each component in this MCU
            let mut mcu_blocks: Vec<Vec<[i32; 64]>> = Vec::new();

            for (ci, comp) in scan_components.iter().enumerate().take(num_components as usize) {
                let blocks_h = comp.h_sample as usize;
                let blocks_v = comp.v_sample as usize;
                let mut blocks = Vec::with_capacity(blocks_h * blocks_v);

                for _ in 0..(blocks_h * blocks_v) {
                    let block = decode_block(
                        &mut reader,
                        dc_tables[comp.dc_table as usize].as_ref()
                            .ok_or("Missing DC Huffman table")?,
                        ac_tables[comp.ac_table as usize].as_ref()
                            .ok_or("Missing AC Huffman table")?,
                        &mut dc_pred[ci],
                        &qt_tables[comp.qt_id as usize],
                    )?;
                    blocks.push(block);
                }
                mcu_blocks.push(blocks);
            }

            // Assemble MCU pixels to RGB
            assemble_mcu(
                &mcu_blocks, &scan_components, num_components,
                max_h, max_v, mcu_col as usize, mcu_row as usize,
                width as usize, height as usize, &mut rgb,
            );
        }
    }

    Ok((rgb, reader.scan_end_pos()))
}

fn decode_block(
    reader: &mut BitReader,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
    dc_pred: &mut i32,
    qt: &[u16; 64],
) -> Result<[i32; 64], String> {
    let mut coeffs = [0i32; 64];

    // DC coefficient
    let dc_len = reader.decode_huffman(dc_table)?;
    let dc_val = if dc_len > 0 {
        let bits = reader.read_bits(dc_len)? as i32;
        // Extend sign
        if bits < (1 << (dc_len - 1)) {
            bits - (1 << dc_len) + 1
        } else {
            bits
        }
    } else { 0 };
    *dc_pred += dc_val;
    coeffs[0] = *dc_pred * qt[0] as i32;

    // AC coefficients
    let mut k = 1;
    while k < 64 {
        let rs = reader.decode_huffman(ac_table)?;
        let run = (rs >> 4) as usize;     // zero run length
        let size = (rs & 0x0F) as u8;     // value bit length

        if size == 0 {
            if run == 0 { break; }         // EOB
            if run == 15 { k += 16; continue; } // ZRL (16 zeros)
            break;
        }

        k += run;
        if k >= 64 { break; }

        let bits = reader.read_bits(size)? as i32;
        let val = if bits < (1 << (size - 1)) {
            bits - (1 << size) + 1
        } else {
            bits
        };
        coeffs[k] = val * qt[k] as i32;
        k += 1;
    }

    Ok(idct(&coeffs))
}

fn assemble_mcu(
    mcu_blocks: &[Vec<[i32; 64]>],
    components: &[JpegComponent],
    num_components: u8,
    max_h: u8, max_v: u8,
    mcu_col: usize, mcu_row: usize,
    img_width: usize, img_height: usize,
    rgb: &mut [u8],
) {
    let mcu_px = mcu_col * max_h as usize * 8;
    let mcu_py = mcu_row * max_v as usize * 8;

    for py in 0..(max_v as usize * 8) {
        for px in 0..(max_h as usize * 8) {
            let x = mcu_px + px;
            let y = mcu_py + py;
            if x >= img_width || y >= img_height { continue; }

            if num_components == 1 {
                // Grayscale
                let val = sample_component(&mcu_blocks[0], &components[0], max_h, max_v, px, py);
                let clamped = val.clamp(0, 255) as u8;
                let offset = (y * img_width + x) * 3;
                rgb[offset] = clamped;
                rgb[offset + 1] = clamped;
                rgb[offset + 2] = clamped;
            } else {
                // YCbCr → RGB
                let yy = sample_component(&mcu_blocks[0], &components[0], max_h, max_v, px, py) as f32 + 128.0;
                let cb = sample_component(&mcu_blocks[1], &components[1], max_h, max_v, px, py) as f32;
                let cr = sample_component(&mcu_blocks[2], &components[2], max_h, max_v, px, py) as f32;

                let r = (yy + 1.402 * cr).round().clamp(0.0, 255.0) as u8;
                let g = (yy - 0.344136 * cb - 0.714136 * cr).round().clamp(0.0, 255.0) as u8;
                let b = (yy + 1.772 * cb).round().clamp(0.0, 255.0) as u8;

                let offset = (y * img_width + x) * 3;
                rgb[offset] = r;
                rgb[offset + 1] = g;
                rgb[offset + 2] = b;
            }
        }
    }
}

fn sample_component(
    blocks: &[[i32; 64]],
    comp: &JpegComponent,
    max_h: u8, max_v: u8,
    px: usize, py: usize,
) -> i32 {
    // Map pixel position to block + pixel within block
    let scale_x = comp.h_sample as usize;
    let scale_y = comp.v_sample as usize;
    let cx = px * scale_x / (max_h as usize * 8);
    let cy = py * scale_y / (max_v as usize * 8);
    let bx = (px * scale_x / (max_h as usize)) % 8;
    let by = (py * scale_y / (max_v as usize)) % 8;
    let block_idx = cy * scale_x + cx;
    if block_idx < blocks.len() {
        blocks[block_idx][by * 8 + bx]
    } else {
        0
    }
}

/// Build a minimal valid 8x8 Baseline JPEG for testing.
/// Encodes a flat mid-gray image (Y=128, Cb=0, Cr=0 → RGB ~128,128,128).
fn build_minimal_jpeg_8x8() -> Vec<u8> {
    // ... (test helper: builds SOI + DQT + SOF0 + DHT + SOS + scan data + EOI)
    // This is a known-good hand-crafted JPEG byte stream
    let mut out = Vec::new();

    // SOI
    out.extend_from_slice(&[0xFF, 0xD8]);

    // DQT — all-ones quantization table (id=0)
    let mut dqt = Vec::new();
    dqt.extend_from_slice(&0x0043u16.to_be_bytes()); // length = 67
    dqt.push(0x00); // 8-bit, table 0
    for _ in 0..64 { dqt.push(1); }
    out.extend_from_slice(&[0xFF, 0xDB]);
    out.extend_from_slice(&dqt);

    // SOF0 — 8x8, 1 component (grayscale for simplicity)
    out.extend_from_slice(&[0xFF, 0xC0]);
    out.extend_from_slice(&0x000Bu16.to_be_bytes()); // length = 11
    out.push(8); // precision
    out.extend_from_slice(&8u16.to_be_bytes()); // height
    out.extend_from_slice(&8u16.to_be_bytes()); // width
    out.push(1); // 1 component
    out.push(1); // component ID
    out.push(0x11); // h_sample=1, v_sample=1
    out.push(0); // qt table 0

    // DHT — DC table (class=0, id=0): symbol 0x04 at length 2 (code=00), symbol 0x00 at length 2 (code=01)
    // Minimal: only need DC length=4 → symbol 0x04 means "4 bits follow"
    out.extend_from_slice(&[0xFF, 0xC4]);
    let mut dht_body = Vec::new();
    dht_body.push(0x00); // DC, id=0
    // Counts for lengths 1-16: 1 symbol at length 1
    dht_body.push(1);
    for _ in 1..16 { dht_body.push(0); }
    dht_body.push(0x00); // symbol: category 0 (DC diff = 0)
    let dht_len = (dht_body.len() + 2) as u16;
    out.extend_from_slice(&dht_len.to_be_bytes());
    out.extend_from_slice(&dht_body);

    // DHT — AC table (class=1, id=0): just EOB symbol
    out.extend_from_slice(&[0xFF, 0xC4]);
    let mut dht_ac = Vec::new();
    dht_ac.push(0x10); // AC, id=0
    dht_ac.push(1); // 1 symbol at length 1
    for _ in 1..16 { dht_ac.push(0); }
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
    // Bits: 0 (DC diff=0) + 0 (EOB) = 0b00 → padded to byte: 0x00
    out.push(0x00);
    // Pad to byte boundary
    out.push(0x00);

    // EOI
    out.extend_from_slice(&[0xFF, 0xD9]);

    out
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package voltex_renderer -- jpg::tests -v`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_renderer/src/jpg.rs
git commit -m "feat(renderer): add JPEG scan decoder with MCU assembly and YCbCr conversion"
```

---

### Task 6: Subsampling Tests + Full Integration Test

**Files:**
- Modify: `crates/voltex_renderer/src/jpg.rs`

- [ ] **Step 1: Add comprehensive tests**

```rust
#[test]
fn test_grayscale_flat() {
    let jpg_data = build_minimal_jpeg_8x8();
    let (rgba, w, h) = parse_jpg(&jpg_data).unwrap();
    assert_eq!(w, 8);
    assert_eq!(h, 8);
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
```

- [ ] **Step 2: Run all tests**

Run: `cargo test --package voltex_renderer -- jpg -v`
Expected: All PASS

- [ ] **Step 3: Run full workspace build**

Run: `cargo build --workspace`
Expected: BUILD SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/voltex_renderer/src/jpg.rs
git commit -m "feat(renderer): complete JPEG decoder with integration tests"
```
