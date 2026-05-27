//! Minimal, dependency-free PNG encoder for capture output.
//!
//! Emits 8-bit RGBA (color type 6) using zlib *stored* (uncompressed) deflate
//! blocks. No compression — capture PNGs are short-lived verification
//! artifacts, so simplicity and zero deps win over file size. The output is a
//! fully valid PNG: it round-trips through [`voltex_renderer::parse_png`].

/// CRC-32 (ISO 3309, as used by PNG chunk checksums) of `bytes`.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Adler-32 (as used by the zlib stream wrapping the deflate data).
fn adler32(bytes: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in bytes {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    // CRC covers chunk type + data.
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

/// Wrap raw bytes in a zlib stream using stored (uncompressed) deflate blocks.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // zlib header: CM=8 (deflate), CINFO=7 (32K window) -> 0x78; FLG chosen so
    // (CMF*256 + FLG) % 31 == 0 with no preset dict and fastest level -> 0x01.
    out.push(0x78);
    out.push(0x01);

    if raw.is_empty() {
        // A single final empty stored block.
        out.push(0x01); // BFINAL=1, BTYPE=00
        out.extend_from_slice(&0u16.to_le_bytes()); // LEN
        out.extend_from_slice(&(!0u16).to_le_bytes()); // NLEN
    } else {
        let mut offset = 0;
        while offset < raw.len() {
            let remaining = raw.len() - offset;
            let block = remaining.min(0xFFFF);
            let is_final = offset + block >= raw.len();
            out.push(if is_final { 0x01 } else { 0x00 });
            let len = block as u16;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(&raw[offset..offset + block]);
            offset += block;
        }
    }

    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

/// Encode an 8-bit RGBA buffer (`width * height * 4` bytes, row-major, top row
/// first) into a complete PNG file.
///
/// # Panics
/// Panics if `rgba.len() != width * height * 4`.
pub fn encode_rgba8(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let expected = (width as usize) * (height as usize) * 4;
    assert_eq!(
        rgba.len(),
        expected,
        "encode_rgba8: expected {expected} bytes for {width}x{height}, got {}",
        rgba.len()
    );

    // Build the raw (pre-deflate) image data: each scanline is prefixed with a
    // filter-type byte (0 = None).
    let stride = (width as usize) * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0); // filter: None
        raw.extend_from_slice(&rgba[y * stride..(y + 1) * stride]);
    }

    let mut out = Vec::new();
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]); // signature

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_chunk(&mut out, b"IHDR", &ihdr);

    // IDAT
    let idat = zlib_stored(&raw);
    write_chunk(&mut out, b"IDAT", &idat);

    // IEND
    write_chunk(&mut out, b"IEND", &[]);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_vector() {
        // CRC-32 of "IEND" is a fixed PNG constant: 0xAE426082.
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn adler32_known_vector() {
        // Adler-32 of empty input is 1.
        assert_eq!(adler32(&[]), 1);
        // Adler-32 of "Wikipedia" is a documented example: 0x11E60398.
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn encodes_valid_signature_and_ihdr() {
        let png = encode_rgba8(1, 1, &[255, 0, 0, 255]);
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        // IHDR length is 13, type follows the 8-byte signature + 4-byte length.
        assert_eq!(&png[12..16], b"IHDR");
    }

    #[test]
    #[should_panic]
    fn rejects_wrong_buffer_length() {
        let _ = encode_rgba8(2, 2, &[0, 0, 0, 0]);
    }

    #[test]
    fn round_trips_through_renderer_decoder() {
        // A 3x2 image with distinct pixels — encode then decode with the
        // engine's own PNG parser and compare byte-for-byte.
        let w = 3u32;
        let h = 2u32;
        let mut src = Vec::new();
        for i in 0..(w * h) {
            let v = (i * 30) as u8;
            src.extend_from_slice(&[v, 255 - v, v.wrapping_add(7), 255]);
        }
        let png = encode_rgba8(w, h, &src);
        let (decoded, dw, dh) = voltex_renderer::parse_png(&png).expect("decode");
        assert_eq!((dw, dh), (w, h));
        assert_eq!(decoded, src);
    }

    #[test]
    fn round_trips_large_multi_block_image() {
        // Larger than one 65535-byte stored block to exercise block splitting:
        // 200x100 RGBA = 80,000 bytes raw payload + filter bytes.
        let w = 200u32;
        let h = 100u32;
        let mut src = Vec::with_capacity((w * h * 4) as usize);
        for i in 0..(w * h) {
            let v = (i % 256) as u8;
            src.extend_from_slice(&[v, v.wrapping_mul(3), v.wrapping_add(99), 255]);
        }
        let png = encode_rgba8(w, h, &src);
        let (decoded, dw, dh) = voltex_renderer::parse_png(&png).expect("decode");
        assert_eq!((dw, dh), (w, h));
        assert_eq!(decoded, src);
    }
}
