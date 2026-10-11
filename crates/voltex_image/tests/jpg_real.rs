//! Baseline JPEG decode against real encoder output (Pillow 12.1.1 / libjpeg),
//! compared with Pillow's own decode of the same file. The unit tests only use
//! hand-built minimal streams; these cover every subsampling the decoder claims
//! (4:4:4, 4:2:2, 4:2:0, grayscale), restart markers (DRI every MCU), and an
//! image size that is not a multiple of the MCU (37x29) so edge cropping runs.
//!
//! The image is a smooth gradient plus a hard-edged block (x 8..20, y 6..18).
//! libjpeg upsamples chroma with a triangle filter ("fancy upsampling"); this
//! decoder replicates samples, so subsampled variants legitimately differ
//! right at the block's chroma edges and are compared away from them.

use voltex_image::parse_jpg;

const W: usize = 37;

fn decode(jpg: &[u8]) -> Vec<u8> {
    let (rgba, w, h) = parse_jpg(jpg).expect("decodes");
    assert_eq!((w, h), (37, 29));
    rgba
}

/// Largest per-channel difference vs. Pillow's decode, over pixels that
/// `include(x, y)` accepts.
fn max_diff(jpg: &[u8], expected_rgb: &[u8], include: impl Fn(usize, usize) -> bool) -> u8 {
    let rgba = decode(jpg);
    assert_eq!(rgba.len(), expected_rgb.len() / 3 * 4);
    rgba.chunks_exact(4)
        .zip(expected_rgb.chunks_exact(3))
        .enumerate()
        .filter(|(i, _)| include(i % W, i / W))
        .flat_map(|(_, (a, e))| (0..3).map(move |c| a[c].abs_diff(e[c])))
        .max()
        .unwrap()
}

/// At least 2 px away from the hard block edges (x = 8 / 20, y = 6 / 18).
fn away_from_block_edges(x: usize, y: usize) -> bool {
    let far = |v: usize, a: usize, b: usize| v.abs_diff(a) >= 2 && v.abs_diff(b) >= 2;
    far(x, 8, 20) && far(y, 6, 18)
}

macro_rules! fixture {
    ($name:literal) => {
        (
            &include_bytes!(concat!("fixtures/", $name))[..],
            &include_bytes!(concat!("fixtures/", $name, ".rgb"))[..],
        )
    };
}

#[test]
fn full_resolution_chroma_matches_libjpeg() {
    let (jpg, rgb) = fixture!("jpg_444.jpg");
    let d = max_diff(jpg, rgb, |_, _| true);
    assert!(d <= 4, "4:4:4 max diff {d}");
}

#[test]
fn grayscale_matches_libjpeg() {
    let (jpg, rgb) = fixture!("jpg_gray.jpg");
    let d = max_diff(jpg, rgb, |_, _| true);
    assert!(d <= 4, "grayscale max diff {d}");
}

#[test]
fn subsampled_chroma_matches_libjpeg_away_from_hard_edges() {
    for (name, (jpg, rgb)) in [("4:2:2", fixture!("jpg_422.jpg")), ("4:2:0", fixture!("jpg_420.jpg"))] {
        let d = max_diff(jpg, rgb, away_from_block_edges);
        assert!(d <= 16, "{name} max diff away from edges {d}");
    }
}

#[test]
fn restart_markers_decode_identically_to_the_same_stream_without_them() {
    // Same image, quality and subsampling — only DRI/RSTn differ, so the
    // decoded pixels must be identical, not merely close.
    let (plain, _) = fixture!("jpg_420.jpg");
    let (restart, _) = fixture!("jpg_420_restart.jpg");
    assert!(restart.windows(2).any(|w| w == [0xFF, 0xDD]), "fixture has DRI");
    assert_eq!(decode(restart), decode(plain));
}

#[test]
fn progressive_jpeg_is_rejected_not_misdecoded() {
    // SOF2 is detected (progressive_jpeg::is_progressive_jpeg) but not decoded.
    let jpg = include_bytes!("fixtures/jpg_progressive.jpg");
    assert!(voltex_image::progressive_jpeg::is_progressive_jpeg(jpg));
    assert!(parse_jpg(jpg).is_err(), "progressive must be an error, not garbage pixels");
}
