use super::*;

fn make_bmp_24bit(width: u32, height: u32, pixel_bgr: [u8; 3]) -> Vec<u8> {
    let row_size = ((24 * width).div_ceil(32) * 4) as usize;
    let pixel_data_size = row_size * height as usize;
    let file_size = 54 + pixel_data_size;
    let mut data = vec![0u8; file_size];
    data[0] = b'B';
    data[1] = b'M';
    data[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
    data[10..14].copy_from_slice(&54u32.to_le_bytes());
    data[14..18].copy_from_slice(&40u32.to_le_bytes());
    data[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    data[22..26].copy_from_slice(&(height as i32).to_le_bytes());
    data[26..28].copy_from_slice(&1u16.to_le_bytes());
    data[28..30].copy_from_slice(&24u16.to_le_bytes());
    for row in 0..height {
        for col in 0..width {
            let offset = 54 + (row as usize) * row_size + (col as usize) * 3;
            data[offset] = pixel_bgr[0];
            data[offset + 1] = pixel_bgr[1];
            data[offset + 2] = pixel_bgr[2];
        }
    }
    data
}

#[test]
fn test_parse_bmp_24bit() {
    let bmp = make_bmp_24bit(2, 2, [255, 0, 0]); // BGR blue
    let img = parse_bmp(&bmp).unwrap();
    assert_eq!(img.width, 2);
    assert_eq!(img.height, 2);
    assert_eq!(img.pixels[0], 0); // R
    assert_eq!(img.pixels[1], 0); // G
    assert_eq!(img.pixels[2], 255); // B
    assert_eq!(img.pixels[3], 255); // A
}

#[test]
fn test_parse_bmp_not_bmp() {
    let data = vec![0u8; 100];
    assert!(parse_bmp(&data).is_err());
}

#[test]
fn test_parse_bmp_too_small() {
    let data = vec![0u8; 10];
    assert!(parse_bmp(&data).is_err());
}

fn make_bmp_header_only(width: i32, height: i32, bpp: u16) -> Vec<u8> {
    let mut data = vec![0u8; 54];
    data[0] = b'B';
    data[1] = b'M';
    data[10..14].copy_from_slice(&54u32.to_le_bytes());
    data[18..22].copy_from_slice(&width.to_le_bytes());
    data[22..26].copy_from_slice(&height.to_le_bytes());
    data[28..30].copy_from_slice(&bpp.to_le_bytes());
    // compression = 0 (default), planes = 1 not strictly required
    data
}

#[test]
fn parse_bmp_rejects_zero_dimensions() {
    // Zero width or height is degenerate; we error rather than vec![0; 0].
    let bmp = make_bmp_header_only(0, 4, 24);
    assert!(parse_bmp(&bmp).is_err());
    let bmp = make_bmp_header_only(4, 0, 24);
    assert!(parse_bmp(&bmp).is_err());
}

#[test]
fn parse_bmp_rejects_huge_dimensions() {
    // Adversarial header with extreme dims used to overflow the size math
    // (`width * height * 4` as u32) or trigger a multi-GB allocation.
    // Now bounded to 16384 with an early error.
    let bmp = make_bmp_header_only(50_000, 50_000, 24);
    assert!(parse_bmp(&bmp).is_err());
    // i32::MAX is also far past the cap.
    let bmp = make_bmp_header_only(i32::MAX, 1, 24);
    assert!(parse_bmp(&bmp).is_err());
}

#[test]
fn parse_bmp_accepts_max_allowed_dimension_header() {
    // Header at exactly the cap (16384x1) parses headers OK, then bails
    // out on data-too-small because no pixel data is present. The point of
    // the test is that we hit *that* error, not an overflow or alloc panic.
    let bmp = make_bmp_header_only(16_384, 1, 24);
    let err = parse_bmp(&bmp).err().expect("expected error").to_string();
    assert!(err.contains("data too small"), "unexpected error: {err}");
}
