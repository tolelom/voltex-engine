//! Inverse MDCT (`imdct`) and the Vorbis power-sine window (`vorbis_window`) used
//! to turn decoded spectra back into overlapping time-domain blocks.

// IMDCT uses indices i,k in the cosine-table equation. Index loops mirror
// the standard MDCT formulation — iterator translation would obscure
// the closed-form summation.
#![allow(clippy::needless_range_loop)]

use std::f32::consts::PI;

/// Compute the inverse MDCT (type IV) of `input` into `output`.
/// `n` is the block size (power of 2). Input has n/2 elements, output has n elements.
pub(super) fn imdct(input: &[f32], output: &mut [f32], n: usize) {
    let n2 = n / 2;

    // Direct computation (O(n^2) — acceptable for our simplified decoder).
    for i in 0..n {
        let mut sum = 0.0f32;
        for k in 0..n2 {
            let angle = PI / n as f32 * (i as f32 + 0.5 + n as f32 * 0.25) * (k as f32 + 0.5);
            sum += input[k] * angle.cos();
        }
        output[i] = sum * 2.0 / n as f32;
    }
}

/// Compute the Vorbis window function for block size n.
pub(super) fn vorbis_window(n: usize) -> Vec<f32> {
    let mut w = Vec::with_capacity(n);
    for i in 0..n {
        let x = ((i as f32 + 0.5) / n as f32 * PI).sin();
        let val = (PI / 2.0 * x * x).sin();
        w.push(val);
    }
    w
}
