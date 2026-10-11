//! PNG decode against real encoder output. Lossless, so supported formats
//! must match byte-for-byte; unsupported ones must be a clear error.
//!
//! - `png_rgb.png` / `png_rgba.png`: Pillow 12.1.1 `optimize=True`.
//! - `png_all_filters.png`: hand-rolled encoder (zlib + CRC) cycling filter
//!   types None/Sub/Up/Average/Paeth row by row, IDAT split into 3 chunks —
//!   Pillow's adaptive filtering only picked Sub and Paeth.
//! - unsupported: palette (Pillow), 8-bit gray (Pillow), 16-bit gray
//!   (Pillow), and the all-filters image with the Adam7 interlace flag set.

use voltex_image::parse_png;

fn assert_exact(png: &[u8], expected_rgba: &[u8]) {
    let (rgba, w, h) = parse_png(png).expect("decodes");
    assert_eq!((w, h), (23, 17));
    assert!(rgba == expected_rgba, "pixels differ from the encoder's source");
}

#[test]
fn pillow_rgba_decodes_exactly() {
    assert_exact(
        include_bytes!("fixtures/png_rgba.png"),
        include_bytes!("fixtures/png_rgba.png.rgba"),
    );
}

#[test]
fn pillow_rgb_decodes_exactly_with_opaque_alpha() {
    assert_exact(
        include_bytes!("fixtures/png_rgb.png"),
        include_bytes!("fixtures/png_rgb.png.rgba"),
    );
}

#[test]
fn every_filter_type_and_split_idat_decode_exactly() {
    assert_exact(
        include_bytes!("fixtures/png_all_filters.png"),
        include_bytes!("fixtures/png_all_filters.png.rgba"),
    );
}

#[test]
fn unsupported_formats_are_errors_not_garbage() {
    for (name, png) in [
        ("palette", &include_bytes!("fixtures/png_palette.png")[..]),
        ("gray8", &include_bytes!("fixtures/png_gray.png")[..]),
        ("gray16", &include_bytes!("fixtures/png_rgb16.png")[..]),
        ("adam7", &include_bytes!("fixtures/png_interlaced.png")[..]),
    ] {
        assert!(parse_png(png).is_err(), "{name} should be rejected");
    }
}
