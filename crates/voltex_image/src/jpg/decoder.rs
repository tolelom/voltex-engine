use super::bit_reader::BitReader;
use super::headers::{HuffTable, JpegComponent};
use super::idct::idct;
use super::mcu::assemble_mcu;

// ---------------------------------------------------------------------------
// Scan decoder
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub(super) fn decode_scan(
    data: &[u8],
    pos: usize,
    width: u16,
    height: u16,
    num_components: u8,
    components: &[JpegComponent],
    qt_tables: &[[u16; 64]; 4],
    dc_tables: &[Option<HuffTable>; 4],
    ac_tables: &[Option<HuffTable>; 4],
    restart_interval: u16,
) -> crate::Result<(Vec<u8>, usize)> {
    // Parse SOS header
    if pos + 2 > data.len() {
        return Err("SOS too short".into());
    }
    let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    let ns = data[pos + 2] as usize;

    let mut scan_components = components.to_vec();
    let mut off = pos + 3;
    for comp in scan_components.iter_mut().take(ns) {
        let _cs = data[off]; // component selector
        let td_ta = data[off + 1];
        comp.dc_table = td_ta >> 4;
        comp.ac_table = td_ta & 0x0F;
        off += 2;
    }
    let scan_data_start = pos + seg_len;

    let mut reader = BitReader::new(data, scan_data_start);

    // Calculate MCU dimensions
    let max_h = scan_components
        .iter()
        .take(num_components as usize)
        .map(|c| c.h_sample)
        .max()
        .unwrap_or(1);
    let max_v = scan_components
        .iter()
        .take(num_components as usize)
        .map(|c| c.v_sample)
        .max()
        .unwrap_or(1);
    let mcu_width = max_h as u16 * 8;
    let mcu_height = max_v as u16 * 8;
    let mcus_x = width.div_ceil(mcu_width);
    let mcus_y = height.div_ceil(mcu_height);

    let mut dc_pred = vec![0i32; num_components as usize];
    let mut rgb = vec![0u8; (width as usize) * (height as usize) * 3];

    let mut mcu_count: u16 = 0;

    for mcu_row in 0..mcus_y {
        for mcu_col in 0..mcus_x {
            // Handle restart interval
            if restart_interval > 0 && mcu_count > 0 && mcu_count.is_multiple_of(restart_interval) {
                // Reset DC predictors
                for dc in dc_pred.iter_mut() {
                    *dc = 0;
                }
                reader.skip_to_rst_marker()?;
            }

            let mut mcu_blocks: Vec<Vec<[i32; 64]>> = Vec::new();

            for (ci, comp) in scan_components
                .iter()
                .enumerate()
                .take(num_components as usize)
            {
                let blocks_h = comp.h_sample as usize;
                let blocks_v = comp.v_sample as usize;
                let mut blocks = Vec::with_capacity(blocks_h * blocks_v);

                for _ in 0..(blocks_h * blocks_v) {
                    let block = decode_block(
                        &mut reader,
                        dc_tables[comp.dc_table as usize]
                            .as_ref()
                            .ok_or("Missing DC Huffman table")?,
                        ac_tables[comp.ac_table as usize]
                            .as_ref()
                            .ok_or("Missing AC Huffman table")?,
                        &mut dc_pred[ci],
                        &qt_tables[comp.qt_id as usize],
                    )?;
                    blocks.push(block);
                }
                mcu_blocks.push(blocks);
            }

            assemble_mcu(
                &mcu_blocks,
                &scan_components,
                num_components,
                max_h,
                max_v,
                mcu_col as usize,
                mcu_row as usize,
                width as usize,
                height as usize,
                &mut rgb,
            );

            mcu_count = mcu_count.wrapping_add(1);
        }
    }

    Ok((rgb, reader.scan_end_pos()))
}

fn decode_block(
    reader: &mut BitReader,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
    dc_pred: &mut i32,
    qt: &[u16; 64],
) -> crate::Result<[i32; 64]> {
    let mut coeffs = [0i32; 64];

    // DC coefficient
    let dc_len = reader.decode_huffman(dc_table)?;
    let dc_val = if dc_len > 0 {
        let bits = reader.read_bits(dc_len)? as i32;
        if bits < (1 << (dc_len - 1)) {
            bits - (1 << dc_len) + 1
        } else {
            bits
        }
    } else {
        0
    };
    *dc_pred += dc_val;
    coeffs[0] = *dc_pred * qt[0] as i32;

    // AC coefficients
    let mut k = 1;
    while k < 64 {
        let rs = reader.decode_huffman(ac_table)?;
        let run = (rs >> 4) as usize;
        let size = rs & 0x0F;

        if size == 0 {
            if run == 0 {
                break;
            } // EOB
            if run == 15 {
                k += 16;
                continue;
            } // ZRL (16 zeros)
            break;
        }

        k += run;
        if k >= 64 {
            break;
        }

        let bits = reader.read_bits(size)? as i32;
        let val = if bits < (1 << (size - 1)) {
            bits - (1 << size) + 1
        } else {
            bits
        };
        coeffs[k] = val * qt[k] as i32;
        k += 1;
    }

    Ok(idct(&coeffs))
}
