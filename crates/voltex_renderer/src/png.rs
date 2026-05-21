//! PNG decoder that uses the self-contained deflate decompressor.
//! Supports 8-bit RGB (color_type=2) and RGBA (color_type=6) images.
#![allow(missing_docs)]

use crate::deflate;

/// Hard cap on either PNG dimension we'll decode. Matches the typical GPU
/// texture limit (`max_texture_dimension_2d = 16384`) and prevents adversarial
/// headers from triggering huge allocations or arithmetic overflow on the size
/// computation.
const PNG_MAX_DIMENSION: u32 = 16_384;

pub fn parse_png(data: &[u8]) -> crate::Result<(Vec<u8>, u32, u32)> {
    // 1. Verify PNG signature
    let signature: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if data.len() < 8 || data[..8] != signature {
        return Err("Invalid PNG signature".into());
    }

    let mut pos = 8;
    let mut width: u32 = 0;
    let mut height: u32 = 0;
    let mut bit_depth: u8;
    let mut color_type: u8 = 0;
    let mut idat_data = Vec::new();
    let mut got_ihdr = false;

    // 2. Parse chunks
    while pos + 8 <= data.len() {
        let chunk_len = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        pos += 8;

        if pos + chunk_len > data.len() {
            return Err("Chunk extends past end of data".into());
        }
        let chunk_data = &data[pos..pos + chunk_len];
        pos += chunk_len;

        // Skip CRC (4 bytes)
        if pos + 4 > data.len() {
            return Err("Missing chunk CRC".into());
        }
        pos += 4;

        match chunk_type {
            b"IHDR" => {
                if chunk_len < 13 {
                    return Err("IHDR chunk too small".into());
                }
                width = u32::from_be_bytes([chunk_data[0], chunk_data[1], chunk_data[2], chunk_data[3]]);
                height = u32::from_be_bytes([chunk_data[4], chunk_data[5], chunk_data[6], chunk_data[7]]);
                bit_depth = chunk_data[8];
                color_type = chunk_data[9];
                let compression = chunk_data[10];
                let _filter = chunk_data[11];
                let interlace = chunk_data[12];

                if bit_depth != 8 {
                    return Err(format!("Unsupported bit depth: {} (only 8 supported)", bit_depth).into());
                }
                if color_type != 2 && color_type != 6 {
                    return Err(format!(
                        "Unsupported color type: {} (only RGB=2 and RGBA=6 supported)",
                        color_type
                    ).into());
                }
                if compression != 0 {
                    return Err("Unsupported compression method".into());
                }
                if interlace != 0 {
                    return Err("Interlaced PNGs not supported".into());
                }
                got_ihdr = true;
            }
            b"IDAT" => {
                idat_data.extend_from_slice(chunk_data);
            }
            b"IEND" => {
                break;
            }
            _ => {
                // Skip unknown chunks
            }
        }
    }

    if !got_ihdr {
        return Err("Missing IHDR chunk".into());
    }
    if width == 0 || height == 0 {
        return Err(format!("PNG zero dimension: {}x{}", width, height).into());
    }
    if width > PNG_MAX_DIMENSION || height > PNG_MAX_DIMENSION {
        return Err(format!(
            "PNG exceeds max dimension {}: {}x{}",
            PNG_MAX_DIMENSION, width, height
        ).into());
    }
    if idat_data.is_empty() {
        return Err("No IDAT chunks found".into());
    }

    // 3. Decompress IDAT data
    let decompressed = deflate::inflate(&idat_data)?;

    // 4. Apply PNG filters
    let channels: usize = match color_type {
        2 => 3,  // RGB
        6 => 4,  // RGBA
        _ => unreachable!(),
    };
    let bpp = channels; // bytes per pixel (bit_depth=8)
    let stride = (width as usize)
        .checked_mul(channels)
        .ok_or("PNG stride overflow")?;
    let expected_size = (height as usize)
        .checked_mul(1usize.checked_add(stride).ok_or("PNG row width overflow")?)
        .ok_or("PNG expected size overflow")?;

    if decompressed.len() < expected_size {
        return Err(format!(
            "Decompressed data too small: got {} expected {}",
            decompressed.len(),
            expected_size
        ).into());
    }

    let image_size = (height as usize)
        .checked_mul(stride)
        .ok_or("PNG image size overflow")?;
    let mut image_data = vec![0u8; image_size];
    let mut prev_row = vec![0u8; stride];

    for y in 0..height as usize {
        let row_start = y * (1 + stride);
        let filter_type = decompressed[row_start];
        let src = &decompressed[row_start + 1..row_start + 1 + stride];

        let dst_start = y * stride;
        let dst_end = dst_start + stride;
        image_data[dst_start..dst_end].copy_from_slice(src);

        let (before, current) = image_data.split_at_mut(dst_start);
        let current = &mut current[..stride];
        let previous = if y == 0 { &prev_row[..] } else { &before[dst_start - stride..dst_start] };

        reconstruct_row(filter_type, current, previous, bpp)?;

        if y == 0 {
            prev_row.copy_from_slice(current);
        }
    }

    // 5. Convert to RGBA if needed
    let rgba = if color_type == 2 {
        // RGB -> RGBA
        let rgba_size = (width as usize)
            .checked_mul(height as usize)
            .and_then(|v| v.checked_mul(4))
            .ok_or("PNG RGBA output size overflow")?;
        let mut rgba = Vec::with_capacity(rgba_size);
        for pixel in image_data.chunks_exact(3) {
            rgba.push(pixel[0]);
            rgba.push(pixel[1]);
            rgba.push(pixel[2]);
            rgba.push(255);
        }
        rgba
    } else {
        // Already RGBA
        image_data
    };

    Ok((rgba, width, height))
}

fn reconstruct_row(filter_type: u8, current: &mut [u8], previous: &[u8], bpp: usize) -> crate::Result<()> {
    match filter_type {
        0 => {} // None
        1 => {
            // Sub
            for i in bpp..current.len() {
                current[i] = current[i].wrapping_add(current[i - bpp]);
            }
        }
        2 => {
            // Up
            for i in 0..current.len() {
                current[i] = current[i].wrapping_add(previous[i]);
            }
        }
        3 => {
            // Average
            for i in 0..current.len() {
                let a = if i >= bpp { current[i - bpp] as u16 } else { 0 };
                let b = previous[i] as u16;
                current[i] = current[i].wrapping_add(((a + b) / 2) as u8);
            }
        }
        4 => {
            // Paeth
            for i in 0..current.len() {
                let a = if i >= bpp { current[i - bpp] as i32 } else { 0 };
                let b = previous[i] as i32;
                let c = if i >= bpp { previous[i - bpp] as i32 } else { 0 };
                current[i] = current[i].wrapping_add(paeth_predictor(a, b, c) as u8);
            }
        }
        _ => return Err(format!("Unknown PNG filter type: {}", filter_type).into()),
    }
    Ok(())
}

fn paeth_predictor(a: i32, b: i32, c: i32) -> i32 {
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crc32(chunk_type: &[u8], data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFFFFFF;
        for &b in chunk_type.iter().chain(data.iter()) {
            crc ^= b as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
            }
        }
        crc ^ 0xFFFFFFFF
    }

    fn write_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(chunk_type);
        out.extend_from_slice(data);
        let crc = crc32(chunk_type, data);
        out.extend_from_slice(&crc.to_be_bytes());
    }

    fn adler32(data: &[u8]) -> u32 {
        let mut a: u32 = 1;
        let mut b: u32 = 0;
        for &byte in data {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn deflate_stored(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(0x78); // CMF
        out.push(0x01); // FLG

        let chunks: Vec<&[u8]> = data.chunks(65535).collect();
        if chunks.is_empty() {
            out.push(0x01);
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&(!0u16).to_le_bytes());
        } else {
            for (i, chunk) in chunks.iter().enumerate() {
                let bfinal = if i == chunks.len() - 1 { 1u8 } else { 0u8 };
                out.push(bfinal);
                let len = chunk.len() as u16;
                out.extend_from_slice(&len.to_le_bytes());
                out.extend_from_slice(&(!len).to_le_bytes());
                out.extend_from_slice(chunk);
            }
        }

        let adler = adler32(data);
        out.extend_from_slice(&adler.to_be_bytes());
        out
    }

    fn create_test_png(width: u32, height: u32, rgba_pixels: &[u8]) -> Vec<u8> {
        let stride = (width as usize) * 4;
        let mut raw = Vec::new();
        for y in 0..height as usize {
            raw.push(0); // filter type None
            raw.extend_from_slice(&rgba_pixels[y * stride..(y + 1) * stride]);
        }

        let compressed = deflate_stored(&raw);

        let mut png = Vec::new();
        png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

        // IHDR
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.push(8); // bit depth
        ihdr.push(6); // color type RGBA
        ihdr.push(0); // compression
        ihdr.push(0); // filter
        ihdr.push(0); // interlace
        write_chunk(&mut png, b"IHDR", &ihdr);

        // IDAT
        write_chunk(&mut png, b"IDAT", &compressed);

        // IEND
        write_chunk(&mut png, b"IEND", &[]);

        png
    }

    fn create_test_png_rgb(width: u32, height: u32, rgb_pixels: &[u8]) -> Vec<u8> {
        let stride = (width as usize) * 3;
        let mut raw = Vec::new();
        for y in 0..height as usize {
            raw.push(0); // filter type None
            raw.extend_from_slice(&rgb_pixels[y * stride..(y + 1) * stride]);
        }

        let compressed = deflate_stored(&raw);

        let mut png = Vec::new();
        png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.push(8); // bit depth
        ihdr.push(2); // color type RGB
        ihdr.push(0);
        ihdr.push(0);
        ihdr.push(0);
        write_chunk(&mut png, b"IHDR", &ihdr);

        write_chunk(&mut png, b"IDAT", &compressed);
        write_chunk(&mut png, b"IEND", &[]);

        png
    }

    #[test]
    fn test_parse_small_png_rgba() {
        let pixels = [
            255, 0, 0, 255, 0, 255, 0, 255,   // row 0: red, green
            0, 0, 255, 255, 255, 255, 0, 255,  // row 1: blue, yellow
        ];
        let png_data = create_test_png(2, 2, &pixels);
        let (result, w, h) = parse_png(&png_data).unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(result.len(), 16);
        // Red pixel
        assert_eq!(result[0], 255);
        assert_eq!(result[1], 0);
        assert_eq!(result[2], 0);
        assert_eq!(result[3], 255);
        // Green pixel
        assert_eq!(result[4], 0);
        assert_eq!(result[5], 255);
        assert_eq!(result[6], 0);
        assert_eq!(result[7], 255);
        // Blue pixel
        assert_eq!(result[8], 0);
        assert_eq!(result[9], 0);
        assert_eq!(result[10], 255);
        assert_eq!(result[11], 255);
        // Yellow pixel
        assert_eq!(result[12], 255);
        assert_eq!(result[13], 255);
        assert_eq!(result[14], 0);
        assert_eq!(result[15], 255);
    }

    #[test]
    fn test_parse_small_png_rgb() {
        let pixels = [
            255, 0, 0, 0, 255, 0,   // row 0: red, green
            0, 0, 255, 255, 255, 0,  // row 1: blue, yellow
        ];
        let png_data = create_test_png_rgb(2, 2, &pixels);
        let (result, w, h) = parse_png(&png_data).unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(result.len(), 16); // converted to RGBA
        // Red pixel with alpha=255
        assert_eq!(&result[0..4], &[255, 0, 0, 255]);
        // Green pixel
        assert_eq!(&result[4..8], &[0, 255, 0, 255]);
    }

    #[test]
    fn test_parse_1x1_png() {
        let pixels = [128, 64, 32, 200];
        let png_data = create_test_png(1, 1, &pixels);
        let (result, w, h) = parse_png(&png_data).unwrap();
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        assert_eq!(result, vec![128, 64, 32, 200]);
    }

    #[test]
    fn test_invalid_signature() {
        let data = [0u8; 100];
        assert!(parse_png(&data).is_err());
    }

    #[test]
    fn test_png_filter_sub() {
        // Test Sub filter reconstruction
        let mut row = vec![10, 20, 30, 40, 5, 10, 15, 20];
        let prev = vec![0u8; 8];
        reconstruct_row(1, &mut row, &prev, 4).unwrap();
        // After Sub: bytes 4..8 get previous pixel added
        assert_eq!(row[4], 5u8.wrapping_add(10));  // 15
        assert_eq!(row[5], 10u8.wrapping_add(20));  // 30
        assert_eq!(row[6], 15u8.wrapping_add(30));  // 45
        assert_eq!(row[7], 20u8.wrapping_add(40));  // 60
    }

    #[test]
    fn test_png_filter_up() {
        let mut row = vec![10, 20, 30, 40];
        let prev = vec![5, 10, 15, 20];
        reconstruct_row(2, &mut row, &prev, 4).unwrap();
        assert_eq!(row, vec![15, 30, 45, 60]);
    }

    #[test]
    fn test_png_filter_average() {
        let mut row = vec![10, 20, 30, 40, 5, 10, 15, 20];
        let prev = vec![0, 0, 0, 0, 0, 0, 0, 0];
        reconstruct_row(3, &mut row, &prev, 4).unwrap();
        // First pixel: avg(0, 0) = 0, so unchanged
        assert_eq!(row[0], 10);
        // Second pixel (i=4): avg(row[0]=10, prev[4]=0) = 5, so 5+5=10
        assert_eq!(row[4], 5u8.wrapping_add((10u16 / 2) as u8));
    }

    #[test]
    fn test_paeth_predictor() {
        assert_eq!(paeth_predictor(10, 20, 5), 20);
        assert_eq!(paeth_predictor(0, 0, 0), 0);
        assert_eq!(paeth_predictor(100, 100, 100), 100);
    }

    #[test]
    fn test_larger_png() {
        // 8x8 gradient image
        let mut pixels = Vec::new();
        for y in 0..8u32 {
            for x in 0..8u32 {
                pixels.push((x * 32) as u8);
                pixels.push((y * 32) as u8);
                pixels.push(128);
                pixels.push(255);
            }
        }
        let png_data = create_test_png(8, 8, &pixels);
        let (result, w, h) = parse_png(&png_data).unwrap();
        assert_eq!(w, 8);
        assert_eq!(h, 8);
        assert_eq!(result, pixels);
    }

    // ── Panic-resistance fuzz tests ────────────────────────────────
    //
    // parse_png must always return Result, never panic, on any input.
    // Truncated, corrupted, or random byte sequences must surface as
    // Err — never as an index-out-of-bounds / unwrap / slice panic.

    /// Deterministic 8-bit byte generator (xorshift32). External
    /// dependencies are forbidden by the engine's dependency policy,
    /// so a tiny inline PRNG is used instead of `rand` / `proptest`.
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
    fn fuzz_parse_png_no_panic_on_empty() {
        let _ = parse_png(&[]);
    }

    #[test]
    fn fuzz_parse_png_no_panic_on_truncated_signature() {
        let signature: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
        for n in 0..=signature.len() {
            let _ = parse_png(&signature[..n]);
        }
    }

    #[test]
    fn fuzz_parse_png_no_panic_on_random_bytes() {
        for seed in 1..=128u32 {
            for &len in &[0usize, 1, 8, 13, 64, 256, 1024] {
                let _ = parse_png(&fuzz_bytes(seed, len));
            }
        }
    }

    #[test]
    fn fuzz_parse_png_no_panic_on_valid_signature_random_tail() {
        let signature: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
        for seed in 1..=128u32 {
            for &tail_len in &[0usize, 1, 16, 64, 256, 1024] {
                let mut data = signature.to_vec();
                data.extend_from_slice(&fuzz_bytes(seed, tail_len));
                let _ = parse_png(&data);
            }
        }
    }

    fn make_png_with_dims(width: u32, height: u32) -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.push(8); // bit depth
        ihdr.push(6); // color type RGBA
        ihdr.push(0);
        ihdr.push(0);
        ihdr.push(0);
        write_chunk(&mut png, b"IHDR", &ihdr);
        // Minimal IDAT (one byte) — parse will reject before reading it.
        write_chunk(&mut png, b"IDAT", &[0]);
        write_chunk(&mut png, b"IEND", &[]);
        png
    }

    #[test]
    fn parse_png_rejects_zero_dimensions() {
        // Zero dimensions used to silently allocate an empty vec and then
        // attempt to slice into a non-existent row.
        assert!(parse_png(&make_png_with_dims(0, 4)).is_err());
        assert!(parse_png(&make_png_with_dims(4, 0)).is_err());
    }

    #[test]
    fn parse_png_rejects_huge_dimensions() {
        // Adversarial IHDR with extreme dims used to overflow `width * height * 4`
        // before the size cap was added.
        assert!(parse_png(&make_png_with_dims(50_000, 50_000)).is_err());
        assert!(parse_png(&make_png_with_dims(u32::MAX, 1)).is_err());
        assert!(parse_png(&make_png_with_dims(1, u32::MAX)).is_err());
    }
}
