// ---------------------------------------------------------------------------
// Codebook (scalar + VQ decode + setup parsing)
// ---------------------------------------------------------------------------
//
// Vorbis spec § 3.2 uses indexed equations on parallel arrays (lengths +
// codewords + values). Iterator translations sacrifice spec line-by-line
// traceability without measurable benefit — keep index loops.
#![allow(clippy::needless_range_loop)]

use super::bit_reader::{bit_reverse, float32_unpack, ilog, lookup1_values, BitReader};

#[derive(Debug, Clone)]
pub(super) struct Codebook {
    pub(super) dimensions: u16,
    // `entries` / `lengths` parsed per Vorbis spec but not consumed by the
    // current decoder path; retained for full PCM-output wire (v1.1+).
    #[allow(dead_code)]
    pub(super) entries: u32,
    /// Codeword lengths for each entry (0 = unused).
    #[allow(dead_code)]
    pub(super) lengths: Vec<u8>,
    /// Lookup type (0 = no lookup, 1 = implicitly defined, 2 = explicitly defined).
    pub(super) lookup_type: u8,
    /// VQ lookup table (if lookup_type != 0).
    pub(super) vq_table: Vec<Vec<f32>>,
    /// Huffman decode tree: (left_child, right_child) indexed by node id.
    /// Leaf nodes store the entry index as a negative-offset value.
    /// We use a flat sorted approach instead.
    pub(super) sorted_entries: Vec<(u32, u32, u8)>, // (codeword, entry_index, length)
}

impl Codebook {
    pub(super) fn decode_scalar(&self, reader: &mut BitReader) -> crate::Result<u32> {
        // Walk the codeword bit by bit, matching against sorted entries.
        // This is a simple but not optimal approach.
        let mut code: u32 = 0;
        let mut code_len: u8 = 0;

        for _ in 0..33 {
            if reader.bits_remaining() == 0 && code_len > 0 {
                // Try to match what we have
                break;
            }
            let bit = reader.read_bits(1)?;
            code |= bit << code_len;
            code_len += 1;

            // Check for a match at this length
            for &(cw, entry, len) in &self.sorted_entries {
                if len == code_len && cw == code {
                    return Ok(entry);
                }
            }
        }

        Err("Codebook: no matching codeword found".into())
    }

    pub(super) fn decode_vq(&self, reader: &mut BitReader) -> crate::Result<&[f32]> {
        let entry = self.decode_scalar(reader)? as usize;
        if entry < self.vq_table.len() {
            Ok(&self.vq_table[entry])
        } else {
            Err(format!("VQ entry {} out of range", entry).into())
        }
    }
}

pub(super) fn decode_codebook(reader: &mut BitReader) -> crate::Result<Codebook> {
    // Sync pattern
    let sync = reader.read_bits(24)?;
    if sync != 0x564342 {
        return Err(format!("Invalid codebook sync pattern: 0x{:06X}", sync).into());
    }

    let dimensions = reader.read_bits(16)? as u16;
    let entries = reader.read_bits(24)?;

    // Read codeword lengths
    let ordered = reader.read_bit()?;
    let mut lengths = vec![0u8; entries as usize];

    if !ordered {
        let sparse = reader.read_bit()?;
        for i in 0..entries as usize {
            if sparse {
                let flag = reader.read_bit()?;
                if flag {
                    lengths[i] = reader.read_bits(5)? as u8 + 1;
                } else {
                    lengths[i] = 0; // unused
                }
            } else {
                lengths[i] = reader.read_bits(5)? as u8 + 1;
            }
        }
    } else {
        let mut current_length = reader.read_bits(5)? as u8 + 1;
        let mut i = 0usize;
        while i < entries as usize {
            let num = ilog(entries - i as u32);
            let number = reader.read_bits(num)? as usize;
            if i + number > entries as usize {
                return Err("Codebook ordered length overflow".into());
            }
            for j in i..i + number {
                lengths[j] = current_length;
            }
            i += number;
            current_length += 1;
        }
    }

    // Build Huffman codewords from lengths (canonical Huffman)
    let max_len = lengths.iter().copied().max().unwrap_or(0);
    let mut sorted_entries: Vec<(u32, u32, u8)> = Vec::new();

    if max_len > 0 {
        // Assign canonical Huffman codes
        // Count entries per length
        let mut bl_count = vec![0u32; max_len as usize + 1];
        for &l in &lengths {
            if l > 0 {
                bl_count[l as usize] += 1;
            }
        }

        // Compute starting code for each length
        let mut next_code = vec![0u32; max_len as usize + 1];
        let mut code = 0u32;
        for bits in 1..=max_len as usize {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }

        // Assign codes — but Vorbis uses bit-reversed codes (LSB first)
        for (i, &l) in lengths.iter().enumerate() {
            if l > 0 {
                let c = next_code[l as usize];
                next_code[l as usize] += 1;
                // Bit-reverse the code for Vorbis (LSB-first reading)
                let reversed = bit_reverse(c, l);
                sorted_entries.push((reversed, i as u32, l));
            }
        }
    }

    // Read VQ lookup
    let lookup_type = reader.read_bits(4)? as u8;
    let mut vq_table: Vec<Vec<f32>> = Vec::new();

    if lookup_type == 1 || lookup_type == 2 {
        let minimum_value = float32_unpack(reader.read_u32()?);
        let delta_value = float32_unpack(reader.read_u32()?);
        let value_bits = reader.read_bits(4)? + 1;
        let sequence_p = reader.read_bit()?;

        let lookup_values = if lookup_type == 1 {
            lookup1_values(entries, dimensions as u32)
        } else {
            entries * dimensions as u32
        };

        let mut multiplicands = Vec::with_capacity(lookup_values as usize);
        for _ in 0..lookup_values {
            multiplicands.push(reader.read_bits(value_bits)?);
        }

        // Build VQ vectors
        for entry_idx in 0..entries as usize {
            let mut vec = vec![0.0f32; dimensions as usize];
            let mut last = 0.0f32;

            if lookup_type == 1 {
                let mut index_divisor = 1u32;
                for dim in 0..dimensions as usize {
                    let offset = (entry_idx as u32 / index_divisor) % lookup_values;
                    vec[dim] = multiplicands[offset as usize] as f32 * delta_value + minimum_value + last;
                    if sequence_p {
                        last = vec[dim];
                    }
                    index_divisor = index_divisor.saturating_mul(lookup_values);
                }
            } else {
                // lookup_type 2
                for dim in 0..dimensions as usize {
                    let offset = entry_idx * dimensions as usize + dim;
                    if offset < multiplicands.len() {
                        vec[dim] = multiplicands[offset] as f32 * delta_value + minimum_value + last;
                        if sequence_p {
                            last = vec[dim];
                        }
                    }
                }
            }

            vq_table.push(vec);
        }
    } else if lookup_type != 0 {
        return Err(format!("Unsupported codebook lookup type: {}", lookup_type).into());
    }

    Ok(Codebook {
        dimensions,
        entries,
        lengths,
        lookup_type,
        vq_table,
        sorted_entries,
    })
}
