//! PNG decode against real encoder output. Lossless, so every supported
//! format must match the reference decode byte-for-byte.
//!
//! - `png_rgb.png` / `png_rgba.png`: Pillow 12.1.1 `optimize=True`.
//! - `png_all_filters.png`: hand-rolled encoder (zlib + CRC) cycling filter
//!   types None/Sub/Up/Average/Paeth row by row, IDAT split into 3 chunks —
//!   Pillow's adaptive filtering only picked Sub and Paeth.
//! - palette 8/4-bit, palette + tRNS, 1-bit and 8-bit gray, 16-bit gray
//!   (`png_gray16.png`, Pillow `I;16`, values x257 so the high byte is the
//!   8-bit source), gray+alpha, RGB + tRNS colour key:
//!   Pillow, expected = Pillow's own `convert("RGBA")`.
//! - `png_rgba16.png`: hand-rolled 16-bit RGBA; expected = high bytes.
//! - `png_interlaced.png`: the all-filters image with the Adam7 flag set —
//!   still unsupported, must be an error rather than garbage.

use voltex_image::decode_png;

fn assert_exact(name: &str, png: &[u8], expected_rgba: &[u8]) {
    let img = decode_png(png).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!((img.width, img.height), (23, 17), "{name}");
    let rgba = img.rgba;
    if rgba != expected_rgba {
        let first = rgba
            .iter()
            .zip(expected_rgba)
            .position(|(a, b)| a != b)
            .unwrap_or(rgba.len().min(expected_rgba.len()));
        panic!(
            "{name}: first differing byte {first} (pixel {}): got {:?} want {:?}",
            first / 4,
            &rgba[first / 4 * 4..(first / 4 * 4 + 4).min(rgba.len())],
            &expected_rgba[first / 4 * 4..(first / 4 * 4 + 4).min(expected_rgba.len())]
        );
    }
}

macro_rules! exact {
    ($test:ident, $file:literal) => {
        #[test]
        fn $test() {
            assert_exact(
                $file,
                include_bytes!(concat!("fixtures/", $file)),
                include_bytes!(concat!("fixtures/", $file, ".rgba")),
            );
        }
    };
}

exact!(pillow_rgba_decodes_exactly, "png_rgba.png");
exact!(pillow_rgb_decodes_exactly_with_opaque_alpha, "png_rgb.png");
exact!(every_filter_type_and_split_idat_decode_exactly, "png_all_filters.png");
exact!(palette_8bit_decodes_exactly, "png_palette.png");
exact!(palette_4bit_unpacks_exactly, "png_palette4.png");
exact!(palette_with_trns_alpha_decodes_exactly, "png_palette_trns.png");
exact!(gray_8bit_decodes_exactly, "png_gray.png");
exact!(gray_1bit_unpacks_and_scales_exactly, "png_gray1.png");
exact!(gray_16bit_keeps_high_bytes, "png_gray16.png");
exact!(gray_alpha_decodes_exactly, "png_gray_alpha.png");
exact!(rgb_with_trns_colour_key_decodes_exactly, "png_rgb_trns.png");
exact!(rgba_16bit_takes_high_bytes, "png_rgba16.png");

#[test]
fn adam7_interlaced_is_still_rejected() {
    assert!(decode_png(include_bytes!("fixtures/png_interlaced.png")).is_err());
}
