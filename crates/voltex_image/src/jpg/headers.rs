// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(super) struct JpegComponent {
    #[allow(dead_code)]
    pub(super) id: u8,
    pub(super) h_sample: u8,
    pub(super) v_sample: u8,
    pub(super) qt_id: u8,
    pub(super) dc_table: u8,
    pub(super) ac_table: u8,
}

pub(super) struct SofData {
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) num_components: u8,
    pub(super) components: Vec<JpegComponent>,
}

pub(super) struct HuffTable {
    pub(super) symbols: Vec<u8>,
    pub(super) offsets: [u16; 17],
    pub(super) maxcode: [i32; 17],
    pub(super) mincode: [u16; 17],
}

// ---------------------------------------------------------------------------
// Marker parsers
// ---------------------------------------------------------------------------

pub(super) fn parse_dqt(
    data: &[u8],
    pos: usize,
    qt_tables: &mut [[u16; 64]; 4],
) -> crate::Result<usize> {
    if pos + 2 > data.len() {
        return Err("DQT too short".into());
    }
    let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    if pos + seg_len > data.len() {
        return Err("DQT segment extends past data".into());
    }

    let mut off = pos + 2;
    let seg_end = pos + seg_len;
    while off < seg_end {
        let pq_tq = data[off];
        let precision = pq_tq >> 4;
        let table_id = (pq_tq & 0x0F) as usize;
        off += 1;
        if table_id >= 4 {
            return Err(format!("DQT table id {} out of range", table_id).into());
        }

        if precision == 0 {
            if off + 64 > seg_end {
                return Err("DQT 8-bit data too short".into());
            }
            for i in 0..64 {
                qt_tables[table_id][i] = data[off + i] as u16;
            }
            off += 64;
        } else {
            if off + 128 > seg_end {
                return Err("DQT 16-bit data too short".into());
            }
            for i in 0..64 {
                qt_tables[table_id][i] =
                    u16::from_be_bytes([data[off + i * 2], data[off + i * 2 + 1]]);
            }
            off += 128;
        }
    }
    Ok(seg_len)
}

pub(super) fn parse_sof(data: &[u8], pos: usize) -> crate::Result<(SofData, usize)> {
    if pos + 2 > data.len() {
        return Err("SOF too short".into());
    }
    let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    if pos + seg_len > data.len() {
        return Err("SOF segment extends past data".into());
    }

    let precision = data[pos + 2];
    if precision != 8 {
        return Err(format!("Unsupported sample precision: {}", precision).into());
    }

    let height = u16::from_be_bytes([data[pos + 3], data[pos + 4]]);
    let width = u16::from_be_bytes([data[pos + 5], data[pos + 6]]);
    let num_comp = data[pos + 7];

    let mut components = Vec::new();
    let mut off = pos + 8;
    for _ in 0..num_comp {
        if off + 3 > pos + seg_len {
            return Err("SOF component data too short".into());
        }
        let id = data[off];
        let sampling = data[off + 1];
        let h_sample = sampling >> 4;
        let v_sample = sampling & 0x0F;
        // ITU T.81 B.2.2: sampling factors are 1..=4. Zero divided by zero
        // in the MCU geometry; larger values blow up the per-MCU block count.
        if !(1..=4).contains(&h_sample) || !(1..=4).contains(&v_sample) {
            return Err(format!("Invalid JPEG sampling factors {h_sample}x{v_sample}").into());
        }
        let qt_id = data[off + 2];
        components.push(JpegComponent {
            id,
            h_sample,
            v_sample,
            qt_id,
            dc_table: 0,
            ac_table: 0,
        });
        off += 3;
    }

    Ok((
        SofData {
            width,
            height,
            num_components: num_comp,
            components,
        },
        seg_len,
    ))
}

pub(super) fn parse_dht(
    data: &[u8],
    pos: usize,
    dc_tables: &mut [Option<HuffTable>; 4],
    ac_tables: &mut [Option<HuffTable>; 4],
) -> crate::Result<usize> {
    if pos + 2 > data.len() {
        return Err("DHT too short".into());
    }
    let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    if pos + seg_len > data.len() {
        return Err("DHT segment extends past data".into());
    }

    let mut off = pos + 2;
    let seg_end = pos + seg_len;
    while off < seg_end {
        let tc_th = data[off];
        let table_class = tc_th >> 4;
        let table_id = (tc_th & 0x0F) as usize;
        off += 1;
        if table_id >= 4 {
            return Err(format!("DHT table id {} out of range", table_id).into());
        }

        if off + 16 > seg_end {
            return Err("DHT counts too short".into());
        }
        let mut counts = [0u8; 16];
        counts.copy_from_slice(&data[off..off + 16]);
        off += 16;

        let total_symbols: usize = counts.iter().map(|&c| c as usize).sum();
        if off + total_symbols > seg_end {
            return Err("DHT symbols too short".into());
        }

        let symbols: Vec<u8> = data[off..off + total_symbols].to_vec();
        off += total_symbols;

        // Build lookup tables
        let mut offsets = [0u16; 17];
        let mut maxcode = [-1i32; 17];
        let mut mincode = [0u16; 17];
        let mut code: u16 = 0;
        let mut sym_offset: u16 = 0;

        for i in 0..16 {
            // A valid prefix code has at most 2^(i+1) codes of length i+1;
            // a corrupt (over-subscribed) table overflowed `code` here.
            if code as u32 + counts[i] as u32 > 1u32 << (i + 1) {
                return Err("DHT table is over-subscribed".into());
            }
            offsets[i] = sym_offset;
            if counts[i] > 0 {
                mincode[i] = code;
                maxcode[i] = (code + counts[i] as u16 - 1) as i32;
                sym_offset += counts[i] as u16;
            }
            code = (code + counts[i] as u16) << 1;
        }
        offsets[16] = sym_offset;

        let table = HuffTable {
            symbols,
            offsets,
            maxcode,
            mincode,
        };
        if table_class == 0 {
            dc_tables[table_id] = Some(table);
        } else {
            ac_tables[table_id] = Some(table);
        }
    }
    Ok(seg_len)
}
