// ---------------------------------------------------------------------------
// IDCT
// ---------------------------------------------------------------------------

/// Zig-zag order for 8x8 block
pub(super) const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27,
    20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

pub(super) fn idct(coeffs: &[i32; 64]) -> [i32; 64] {
    let mut workspace = [0.0f64; 64];

    // Arrange from zigzag to row-major
    let mut block = [0.0f64; 64];
    for i in 0..64 {
        block[ZIGZAG[i]] = coeffs[i] as f64;
    }

    // 1D IDCT on rows
    for row in 0..8 {
        let off = row * 8;
        idct_1d(&mut block, off);
    }

    // Transpose
    for r in 0..8 {
        for c in 0..8 {
            workspace[c * 8 + r] = block[r * 8 + c];
        }
    }

    // 1D IDCT on columns (now rows after transpose)
    for row in 0..8 {
        let off = row * 8;
        idct_1d(&mut workspace, off);
    }

    // Transpose back and round
    let mut result = [0i32; 64];
    for r in 0..8 {
        for c in 0..8 {
            result[r * 8 + c] = workspace[c * 8 + r].round() as i32;
        }
    }
    result
}

fn idct_1d(data: &mut [f64], off: usize) {
    use std::f64::consts::PI;
    let mut tmp = [0.0f64; 8];
    for (x, slot) in tmp.iter_mut().enumerate() {
        let mut sum = 0.0;
        for u in 0..8 {
            let cu = if u == 0 { 1.0 / 2.0f64.sqrt() } else { 1.0 };
            sum += cu * data[off + u] * ((2.0 * x as f64 + 1.0) * u as f64 * PI / 16.0).cos();
        }
        *slot = sum / 2.0;
    }
    data[off..off + 8].copy_from_slice(&tmp);
}
