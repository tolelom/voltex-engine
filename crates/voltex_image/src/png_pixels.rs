//! Unfiltered PNG scanlines → RGBA8, for every non-interlaced colour type and
//! bit depth the PNG spec allows (§11.2.2): grayscale 1/2/4/8/16, RGB 8/16,
//! palette 1/2/4/8, gray+alpha 8/16, RGBA 8/16. 16-bit samples keep their
//! high byte; sub-byte grayscale scales to 0..=255; `tRNS` supplies palette
//! alpha or a gray/RGB colour key.

/// Bits per pixel for a valid (colour type, bit depth) pair, else an error.
pub(crate) fn bits_per_pixel(color_type: u8, bit_depth: u8) -> crate::Result<usize> {
    let channels = match (color_type, bit_depth) {
        (0, 1 | 2 | 4 | 8 | 16) => 1,
        (2, 8 | 16) => 3,
        (3, 1 | 2 | 4 | 8) => 1,
        (4, 8 | 16) => 2,
        (6, 8 | 16) => 4,
        _ => {
            return Err(format!(
                "Invalid PNG colour type {} / bit depth {}",
                color_type, bit_depth
            )
            .into())
        }
    };
    Ok(channels * bit_depth as usize)
}

/// Sample `i` (0-based, in units of `bit_depth`) of a scanline, MSB-first.
fn sample(row: &[u8], i: usize, bit_depth: u8) -> u16 {
    match bit_depth {
        16 => u16::from_be_bytes([row[i * 2], row[i * 2 + 1]]),
        8 => row[i] as u16,
        d => {
            let per_byte = 8 / d as usize;
            let byte = row[i / per_byte];
            let shift = 8 - d as usize * (i % per_byte + 1);
            ((byte >> shift) & ((1u8 << d) - 1)) as u16
        }
    }
}

/// Convert a native-depth sample to 8 bits (high byte for 16-bit, scaled for
/// sub-byte grayscale).
fn to_u8(v: u16, bit_depth: u8) -> u8 {
    match bit_depth {
        16 => (v >> 8) as u8,
        8 => v as u8,
        d => ((v as u32 * 255) / ((1u32 << d) - 1)) as u8,
    }
}

/// `image` is the unfiltered image: `height` rows of `stride` bytes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn to_rgba8(
    image: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    color_type: u8,
    bit_depth: u8,
    palette: Option<&[u8]>,
    trns: Option<&[u8]>,
) -> crate::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(width * height * 4);
    // Colour key from tRNS (compared at native depth) for gray / RGB.
    let key = |i: usize| -> Option<u16> {
        let t = trns?;
        (t.len() >= i * 2 + 2).then(|| u16::from_be_bytes([t[i * 2], t[i * 2 + 1]]))
    };
    let palette = match color_type {
        3 => Some(palette.ok_or("Palette PNG without PLTE chunk")?),
        _ => None,
    };
    for y in 0..height {
        let row = &image[y * stride..(y + 1) * stride];
        for x in 0..width {
            let px = match color_type {
                0 => {
                    let g = sample(row, x, bit_depth);
                    let a = if key(0) == Some(g) { 0 } else { 255 };
                    let g = to_u8(g, bit_depth);
                    [g, g, g, a]
                }
                2 => {
                    let (r, g, b) = (
                        sample(row, x * 3, bit_depth),
                        sample(row, x * 3 + 1, bit_depth),
                        sample(row, x * 3 + 2, bit_depth),
                    );
                    let keyed = key(0) == Some(r) && key(1) == Some(g) && key(2) == Some(b);
                    let a = if keyed { 0 } else { 255 };
                    [to_u8(r, bit_depth), to_u8(g, bit_depth), to_u8(b, bit_depth), a]
                }
                3 => {
                    let idx = sample(row, x, bit_depth) as usize;
                    let pal = palette.unwrap_or_default();
                    let rgb = pal
                        .get(idx * 3..idx * 3 + 3)
                        .ok_or_else(|| format!("Palette index {} out of range", idx))?;
                    let a = trns.and_then(|t| t.get(idx).copied()).unwrap_or(255);
                    [rgb[0], rgb[1], rgb[2], a]
                }
                4 => {
                    let g = to_u8(sample(row, x * 2, bit_depth), bit_depth);
                    let a = to_u8(sample(row, x * 2 + 1, bit_depth), bit_depth);
                    [g, g, g, a]
                }
                6 => [0, 1, 2, 3].map(|c| to_u8(sample(row, x * 4 + c, bit_depth), bit_depth)),
                _ => unreachable!("bits_per_pixel rejects other colour types"),
            };
            out.extend_from_slice(&px);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_byte_samples_unpack_msb_first() {
        let row = [0b1011_0001u8];
        assert_eq!((0..8).map(|i| sample(&row, i, 1)).collect::<Vec<_>>(), [1, 0, 1, 1, 0, 0, 0, 1]);
        assert_eq!((0..4).map(|i| sample(&row, i, 2)).collect::<Vec<_>>(), [2, 3, 0, 1]);
        assert_eq!((0..2).map(|i| sample(&row, i, 4)).collect::<Vec<_>>(), [11, 1]);
    }

    #[test]
    fn invalid_depth_colour_pairs_are_rejected() {
        assert!(bits_per_pixel(2, 4).is_err()); // RGB must be 8/16
        assert!(bits_per_pixel(3, 16).is_err()); // palette max 8
        assert!(bits_per_pixel(5, 8).is_err()); // no colour type 5
        assert_eq!(bits_per_pixel(0, 1).unwrap(), 1);
        assert_eq!(bits_per_pixel(6, 16).unwrap(), 64);
    }
}
