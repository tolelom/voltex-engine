//! Vorbis residue: `decode_residue_config` parses the setup-header config and
//! `decode_residue` decodes the per-channel residue vectors of an audio packet.

// Vorbis spec § 8.6 residue type 2 (interleaved) uses indexed partition +
// classification loops where the index addresses parallel arrays
// (cascade[i], books[i][j], classifications[j]). Keep index loops.
#![allow(clippy::needless_range_loop)]

use super::bit_reader::BitReader;
use super::codebook::Codebook;

#[derive(Debug, Clone)]
pub(super) struct ResidueConfig {
    pub(super) residue_type: u16,
    pub(super) begin: u32,
    pub(super) end: u32,
    pub(super) partition_size: u32,
    pub(super) classifications: u8,
    pub(super) classbook: u8,
    pub(super) cascade: Vec<u8>,
    pub(super) books: Vec<Vec<i16>>,
}

pub(super) fn decode_residue_config(reader: &mut BitReader, residue_type: u16) -> crate::Result<ResidueConfig> {
    let begin = reader.read_bits(24)?;
    let end = reader.read_bits(24)?;
    let partition_size = reader.read_bits(24)? + 1;
    let classifications = reader.read_bits(6)? as u8 + 1;
    let classbook = reader.read_bits(8)? as u8;

    let mut cascade = vec![0u8; classifications as usize];
    for i in 0..classifications as usize {
        let low_bits = reader.read_bits(3)? as u8;
        let bit_flag = reader.read_bit()?;
        if bit_flag {
            let high_bits = reader.read_bits(5)? as u8;
            cascade[i] = low_bits | (high_bits << 3);
        } else {
            cascade[i] = low_bits;
        }
    }

    let mut books = vec![vec![-1i16; 8]; classifications as usize];
    for i in 0..classifications as usize {
        for j in 0..8 {
            if cascade[i] & (1 << j) != 0 {
                books[i][j] = reader.read_bits(8)? as i16;
            }
        }
    }

    Ok(ResidueConfig {
        residue_type,
        begin,
        end,
        partition_size,
        classifications,
        classbook,
        cascade,
        books,
    })
}

pub(super) fn decode_residue(
    reader: &mut BitReader,
    residue: &ResidueConfig,
    codebooks: &[Codebook],
    n_half: usize,
    ch: usize,
    do_not_decode: &[bool],
) -> crate::Result<Vec<Vec<f32>>> {
    let mut residue_vectors = vec![vec![0.0f32; n_half]; ch];

    let actual_size = if residue.residue_type == 2 {
        n_half * ch
    } else {
        n_half
    };

    let limit_begin = residue.begin.min(actual_size as u32) as usize;
    let limit_end = residue.end.min(actual_size as u32) as usize;

    if limit_begin >= limit_end {
        return Ok(residue_vectors);
    }

    let partitions_to_read = (limit_end - limit_begin) / residue.partition_size as usize;
    let classbook = &codebooks[residue.classbook as usize];
    let classwords = classbook.dimensions as usize;

    if residue.residue_type == 2 {
        // Interleaved residue: decode into a single big vector, then deinterleave
        let all_not_decoded = do_not_decode.iter().all(|&d| d);
        if all_not_decoded {
            return Ok(residue_vectors);
        }

        let mut interleaved = vec![0.0f32; actual_size];

        let mut partition_idx = 0usize;
        while partition_idx < partitions_to_read {
            // Decode classification
            let temp = classbook.decode_scalar(reader)?;
            let mut classifications = vec![0u8; classwords];
            let mut t = temp;
            for j in (0..classwords).rev() {
                classifications[j] = (t % residue.classifications as u32) as u8;
                t /= residue.classifications as u32;
            }

            for j in 0..classwords {
                if partition_idx + j >= partitions_to_read {
                    break;
                }

                let vqclass = classifications[j] as usize;
                let offset = limit_begin + (partition_idx + j) * residue.partition_size as usize;

                // Pass 0 only (simplified — only read first pass)
                if residue.cascade[vqclass] & 1 != 0 {
                    let book_idx = residue.books[vqclass][0];
                    if book_idx >= 0 {
                        let book = &codebooks[book_idx as usize];
                        if book.lookup_type != 0 {
                            // VQ decode
                            let mut pos = offset;
                            while pos < offset + residue.partition_size as usize && pos < actual_size {
                                let vq = book.decode_vq(reader)?;
                                for &v in vq {
                                    if pos < actual_size {
                                        interleaved[pos] += v;
                                        pos += 1;
                                    }
                                }
                            }
                        } else {
                            // Scalar decode
                            for pos in offset..(offset + residue.partition_size as usize).min(actual_size) {
                                let val = book.decode_scalar(reader)?;
                                interleaved[pos] += val as f32;
                            }
                        }
                    }
                }
            }

            partition_idx += classwords;
        }

        // Deinterleave
        for i in 0..actual_size {
            let channel = i % ch;
            let sample = i / ch;
            if sample < n_half {
                residue_vectors[channel][sample] = interleaved[i];
            }
        }
    } else {
        // Type 0 or 1: decode per channel (simplified)
        for c in 0..ch {
            if do_not_decode[c] {
                continue;
            }

            let mut partition_idx = 0usize;
            while partition_idx < partitions_to_read {
                let temp = classbook.decode_scalar(reader)?;
                let mut classifications = vec![0u8; classwords];
                let mut t = temp;
                for j in (0..classwords).rev() {
                    classifications[j] = (t % residue.classifications as u32) as u8;
                    t /= residue.classifications as u32;
                }

                for j in 0..classwords {
                    if partition_idx + j >= partitions_to_read {
                        break;
                    }

                    let vqclass = classifications[j] as usize;
                    let offset = limit_begin + (partition_idx + j) * residue.partition_size as usize;

                    if residue.cascade[vqclass] & 1 != 0 {
                        let book_idx = residue.books[vqclass][0];
                        if book_idx >= 0 {
                            let book = &codebooks[book_idx as usize];
                            if book.lookup_type != 0 {
                                let mut pos = offset;
                                while pos < offset + residue.partition_size as usize && pos < n_half {
                                    let vq = book.decode_vq(reader)?;
                                    for &v in vq {
                                        if pos < n_half {
                                            residue_vectors[c][pos] += v;
                                            pos += 1;
                                        }
                                    }
                                }
                            } else {
                                for pos in offset..(offset + residue.partition_size as usize).min(n_half) {
                                    let val = book.decode_scalar(reader)?;
                                    residue_vectors[c][pos] += val as f32;
                                }
                            }
                        }
                    }
                }

                partition_idx += classwords;
            }
        }
    }

    Ok(residue_vectors)
}
