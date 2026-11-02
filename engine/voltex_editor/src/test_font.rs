//! Minimal TrueType font built in memory, so font tests run on any host
//! without depending on installed system fonts.
//!
//! Glyphs: 0 = `.notdef` (empty), 1 = space (empty), 2 = 'A' (one contour
//! with an off-curve point). cmap format 4 maps U+0020 and U+0041.

const UNITS_PER_EM: u16 = 1000;

fn put16(buf: &mut [u8], off: usize, value: u16) {
    buf[off..off + 2].copy_from_slice(&value.to_be_bytes());
}

fn put32(buf: &mut [u8], off: usize, value: u32) {
    buf[off..off + 4].copy_from_slice(&value.to_be_bytes());
}

fn push16(buf: &mut Vec<u8>, values: &[u16]) {
    for value in values {
        buf.extend_from_slice(&value.to_be_bytes());
    }
}

fn head() -> Vec<u8> {
    let mut t = vec![0u8; 54];
    put32(&mut t, 0, 0x0001_0000);
    put32(&mut t, 12, 0x5F0F_3CF5);
    put16(&mut t, 18, UNITS_PER_EM);
    put16(&mut t, 50, 0); // short loca
    t
}

fn hhea() -> Vec<u8> {
    let mut t = vec![0u8; 36];
    put32(&mut t, 0, 0x0001_0000);
    put16(&mut t, 4, 800);
    put16(&mut t, 6, (-200i16) as u16);
    put16(&mut t, 10, 1000);
    put16(&mut t, 34, 3);
    t
}

fn maxp() -> Vec<u8> {
    let mut t = vec![0u8; 6];
    put32(&mut t, 0, 0x0000_5000);
    put16(&mut t, 4, 3);
    t
}

fn hmtx() -> Vec<u8> {
    let mut t = Vec::new();
    push16(&mut t, &[500, 0, 250, 0, 1000, 100]);
    t
}

fn glyf() -> Vec<u8> {
    let mut t = Vec::new();
    // numberOfContours, xMin, yMin, xMax, yMax, endPtsOfContours,
    // instructionLength
    push16(&mut t, &[1, 100, 0, 900, 700, 3, 0]);
    // Flags: on, on, off, on. No short or repeat bits, so every
    // coordinate delta is an i16.
    t.extend_from_slice(&[0x01, 0x01, 0x00, 0x01]);
    for dx in [100i16, 800, 0, -800] {
        t.extend_from_slice(&dx.to_be_bytes());
    }
    for dy in [0i16, 0, 700, 0] {
        t.extend_from_slice(&dy.to_be_bytes());
    }
    t
}

fn loca(glyf_len: usize) -> Vec<u8> {
    let mut t = Vec::new();
    push16(&mut t, &[0, 0, 0, (glyf_len / 2) as u16]);
    t
}

fn cmap() -> Vec<u8> {
    let mut t = Vec::new();
    // Header and one (3, 1) encoding record pointing at offset 12.
    push16(&mut t, &[0, 1, 3, 1]);
    t.extend_from_slice(&12u32.to_be_bytes());
    let delta = |gid: u16, code: u16| gid.wrapping_sub(code);
    // Format 4: format, length, language, segCountX2, searchRange,
    // entrySelector, rangeShift.
    push16(&mut t, &[4, 40, 0, 6, 4, 1, 2]);
    push16(&mut t, &[0x20, 0x41, 0xFFFF]); // endCode
    push16(&mut t, &[0]); // reservedPad
    push16(&mut t, &[0x20, 0x41, 0xFFFF]); // startCode
    push16(&mut t, &[delta(1, 0x20), delta(2, 0x41), 1]); // idDelta
    push16(&mut t, &[0, 0, 0]); // idRangeOffset
    t
}

pub(crate) fn minimal_ttf() -> Vec<u8> {
    let glyf = glyf();
    let loca = loca(glyf.len());
    let tables: [(&[u8; 4], Vec<u8>); 7] = [
        (b"cmap", cmap()),
        (b"glyf", glyf),
        (b"head", head()),
        (b"hhea", hhea()),
        (b"hmtx", hmtx()),
        (b"loca", loca),
        (b"maxp", maxp()),
    ];

    let num_tables = tables.len();
    let mut font = vec![0u8; 12 + num_tables * 16];
    put32(&mut font, 0, 0x0001_0000);
    put16(&mut font, 4, num_tables as u16);
    put16(&mut font, 6, 64);
    put16(&mut font, 8, 2);
    put16(&mut font, 10, (num_tables * 16 - 64) as u16);

    for (i, (tag, data)) in tables.iter().enumerate() {
        let record = 12 + i * 16;
        let offset = font.len();
        font[record..record + 4].copy_from_slice(*tag);
        put32(&mut font, record + 8, offset as u32);
        put32(&mut font, record + 12, data.len() as u32);
        font.extend_from_slice(data);
        font.resize(font.len().next_multiple_of(4), 0);
    }
    font
}
