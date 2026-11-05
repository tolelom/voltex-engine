//! 무압축 24/32bpp BMP 디코더 → RGBA8.

/// In-memory decoded BMP image (RGBA pixel rows).
pub struct BmpImage {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Pixel data — `width * height * 4` bytes, RGBA.
    pub pixels: Vec<u8>,
}

/// Hard cap on either dimension we'll decode. Matches the typical GPU texture
/// limit (`max_texture_dimension_2d = 16384`) on desktop adapters and prevents
/// adversarial BMP headers from triggering huge allocations or arithmetic
/// overflow on the size computation below.
pub const BMP_MAX_DIMENSION: u32 = 16_384;

/// BITMAPFILEHEADER(14) + BITMAPINFOHEADER(40).
const BMP_HEADER_SIZE: usize = 54;

/// Decode an uncompressed 24- or 32-bit BMP into top-down RGBA8 rows.
pub fn parse_bmp(data: &[u8]) -> crate::Result<BmpImage> {
    if data.len() < BMP_HEADER_SIZE {
        return Err(format!("BMP too small: {} bytes", data.len()).into());
    }
    if data[0] != b'B' || data[1] != b'M' {
        return Err("Not a BMP file: missing 'BM' signature".into());
    }

    let pixel_offset = u32::from_le_bytes(data[10..14].try_into().unwrap()) as usize;
    let width_raw = i32::from_le_bytes(data[18..22].try_into().unwrap());
    let height_raw = i32::from_le_bytes(data[22..26].try_into().unwrap());
    let bpp = u16::from_le_bytes(data[28..30].try_into().unwrap());
    let compression = u32::from_le_bytes(data[30..34].try_into().unwrap());

    if bpp != 24 && bpp != 32 {
        return Err(format!("Unsupported BMP bpp: {} (only 24 and 32 supported)", bpp).into());
    }
    if compression != 0 {
        return Err(format!(
            "Unsupported BMP compression: {} (only 0/uncompressed supported)",
            compression
        )
        .into());
    }

    let width = width_raw.unsigned_abs();
    let height = height_raw.unsigned_abs();
    // 양수 높이 = 아래 행부터 저장된 bottom-up 비트맵.
    let bottom_up = height_raw > 0;

    if width == 0 || height == 0 {
        return Err(format!("BMP zero dimension: {}x{}", width, height).into());
    }
    if width > BMP_MAX_DIMENSION || height > BMP_MAX_DIMENSION {
        return Err(format!(
            "BMP exceeds max dimension {}: {}x{}",
            BMP_MAX_DIMENSION, width, height
        )
        .into());
    }

    let bytes_per_pixel = (bpp as usize) / 8;
    // 각 행은 4바이트 경계로 패딩된다.
    let row_size = (bpp as usize)
        .checked_mul(width as usize)
        .ok_or("BMP row size overflow")?
        .div_ceil(32)
        .checked_mul(4)
        .ok_or("BMP row size overflow")?;
    let row_bytes = row_size
        .checked_mul(height as usize)
        .ok_or("BMP pixel block size overflow")?;
    let required_data_size = pixel_offset
        .checked_add(row_bytes)
        .ok_or("BMP data offset overflow")?;

    if data.len() < required_data_size {
        return Err(format!(
            "BMP data too small: need {} bytes, got {}",
            required_data_size,
            data.len()
        )
        .into());
    }

    let dst_size = (width as usize)
        .checked_mul(height as usize)
        .and_then(|v| v.checked_mul(4))
        .ok_or("BMP output size overflow")?;
    let mut pixels = vec![0u8; dst_size];

    for row in 0..height as usize {
        let src_row = if bottom_up {
            height as usize - 1 - row
        } else {
            row
        };
        let row_start = pixel_offset + src_row * row_size;

        for col in 0..width as usize {
            let src_offset = row_start + col * bytes_per_pixel;
            let dst_offset = (row * width as usize + col) * 4;

            // BMP stores BGR(A), convert to RGBA
            let b = data[src_offset];
            let g = data[src_offset + 1];
            let r = data[src_offset + 2];
            let a = if bytes_per_pixel == 4 {
                data[src_offset + 3]
            } else {
                255
            };

            pixels[dst_offset] = r;
            pixels[dst_offset + 1] = g;
            pixels[dst_offset + 2] = b;
            pixels[dst_offset + 3] = a;
        }
    }

    Ok(BmpImage {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests;
