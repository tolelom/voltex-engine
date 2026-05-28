use super::headers::JpegComponent;

// ---------------------------------------------------------------------------
// MCU assembly + color conversion
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub(super) fn assemble_mcu(
    mcu_blocks: &[Vec<[i32; 64]>],
    components: &[JpegComponent],
    num_components: u8,
    max_h: u8,
    max_v: u8,
    mcu_col: usize,
    mcu_row: usize,
    img_width: usize,
    img_height: usize,
    rgb: &mut [u8],
) {
    let mcu_px = mcu_col * max_h as usize * 8;
    let mcu_py = mcu_row * max_v as usize * 8;

    for py in 0..(max_v as usize * 8) {
        for px in 0..(max_h as usize * 8) {
            let x = mcu_px + px;
            let y = mcu_py + py;
            if x >= img_width || y >= img_height {
                continue;
            }

            if num_components == 1 {
                // Grayscale: IDCT output centered at 0, add 128 for level shift
                let val =
                    sample_component(&mcu_blocks[0], &components[0], max_h, max_v, px, py) + 128;
                let clamped = val.clamp(0, 255) as u8;
                let offset = (y * img_width + x) * 3;
                rgb[offset] = clamped;
                rgb[offset + 1] = clamped;
                rgb[offset + 2] = clamped;
            } else {
                // YCbCr -> RGB
                // IDCT output centered at 0; Y needs +128 level shift, Cb/Cr centered at 0 (128 subtracted)
                let yy = sample_component(&mcu_blocks[0], &components[0], max_h, max_v, px, py)
                    as f32
                    + 128.0;
                let cb =
                    sample_component(&mcu_blocks[1], &components[1], max_h, max_v, px, py) as f32;
                let cr =
                    sample_component(&mcu_blocks[2], &components[2], max_h, max_v, px, py) as f32;

                let r = (yy + 1.402 * cr).round().clamp(0.0, 255.0) as u8;
                let g = (yy - 0.344136 * cb - 0.714136 * cr)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let b = (yy + 1.772 * cb).round().clamp(0.0, 255.0) as u8;

                let offset = (y * img_width + x) * 3;
                rgb[offset] = r;
                rgb[offset + 1] = g;
                rgb[offset + 2] = b;
            }
        }
    }
}

pub(super) fn sample_component(
    blocks: &[[i32; 64]],
    comp: &JpegComponent,
    max_h: u8,
    max_v: u8,
    px: usize,
    py: usize,
) -> i32 {
    let scale_x = comp.h_sample as usize;
    let scale_y = comp.v_sample as usize;
    let cx = px * scale_x / (max_h as usize * 8);
    let cy = py * scale_y / (max_v as usize * 8);
    let bx = (px * scale_x / max_h as usize) % 8;
    let by = (py * scale_y / max_v as usize) % 8;
    let block_idx = cy * scale_x + cx;
    if block_idx < blocks.len() {
        blocks[block_idx][by * 8 + bx]
    } else {
        0
    }
}
