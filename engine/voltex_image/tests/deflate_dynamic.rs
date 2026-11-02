//! Inflate against real zlib output. The unit tests only build stored and
//! fixed-Huffman blocks by hand, and every PNG in the repo comes from the
//! engine's own (stored-block) encoder — but PNGs from image editors are
//! almost always dynamic-Huffman (BTYPE=2) streams.
//!
//! Fixtures were produced once with CPython's `zlib.compress(data, 9)`
//! (zlib 1.3.1); the payload generators below mirror the Python ones exactly.

use voltex_image::deflate::inflate;

fn text_payload() -> Vec<u8> {
    (0..3000)
        .map(|i| format!("voltex {} ", i % 97))
        .collect::<String>()
        .into_bytes()
}

/// Same LCG as the fixture script: s = (s * 1103515245 + 12345) & 0x7fffffff.
fn lcg(n: usize, mut s: u32) -> Vec<u8> {
    (0..n)
        .map(|_| {
            s = (s.wrapping_mul(1_103_515_245).wrapping_add(12_345)) & 0x7fff_ffff;
            ((s >> 16) & 0xff) as u8
        })
        .collect()
}

/// 1000 random bytes, 30000 zeros, the same 1000 bytes again — the repeat is
/// a back-reference ~31000 bytes back, near deflate's 32 KiB window limit.
fn long_distance_payload() -> Vec<u8> {
    let r = lcg(1000, 12_345);
    let mut out = r.clone();
    out.extend(std::iter::repeat_n(0u8, 30_000));
    out.extend(r);
    out
}

#[test]
fn inflates_zlib_dynamic_huffman_stream() {
    let z = include_bytes!("fixtures/deflate_dynamic_text.zz");
    assert_eq!((z[2] >> 1) & 3, 2, "fixture must start with a dynamic block");
    let out = inflate(z).expect("dynamic-Huffman stream inflates");
    assert_eq!(out.len(), 29_690);
    assert_eq!(out, text_payload());
}

#[test]
fn inflates_back_references_near_the_window_limit() {
    let z = include_bytes!("fixtures/deflate_long_distance.zz");
    let out = inflate(z).expect("long-distance stream inflates");
    assert_eq!(out, long_distance_payload());
}

#[test]
fn truncated_dynamic_stream_is_an_error_not_a_panic() {
    let z = include_bytes!("fixtures/deflate_dynamic_text.zz");
    for cut in [3, 10, 50, z.len() / 2] {
        let _ = inflate(&z[..cut]); // must return (Ok or Err), never panic
    }
}
