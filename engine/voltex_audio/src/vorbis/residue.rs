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

/// Decodes the residue vectors of `ch` channels from an audio packet
/// (Vorbis spec § 8.6). Type 2 decodes one interleaved vector and splits it
/// per channel; types 0/1 decode each channel separately.
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
    let partitions = PartitionRange { begin: limit_begin, count: partitions_to_read };

    if residue.residue_type == 2 {
        // Interleaved residue: decode into a single big vector, then deinterleave
        let all_not_decoded = do_not_decode.iter().all(|&d| d);
        if all_not_decoded {
            return Ok(residue_vectors);
        }

        let mut interleaved = vec![0.0f32; actual_size];
        decode_partitions(reader, residue, codebooks, classbook, partitions, &mut interleaved)?;

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
            decode_partitions(reader, residue, codebooks, classbook, partitions, &mut residue_vectors[c])?;
        }
    }

    Ok(residue_vectors)
}

/// Which partitions of a residue vector to decode: the first sample offset
/// and the number of partitions after it.
#[derive(Clone, Copy)]
struct PartitionRange {
    begin: usize,
    count: usize,
}

/// Decodes `partitions` into `target`, one classword group at a time: read
/// the classifications, then each partition's values (pass 0 only).
fn decode_partitions(
    reader: &mut BitReader,
    residue: &ResidueConfig,
    codebooks: &[Codebook],
    classbook: &Codebook,
    partitions: PartitionRange,
    target: &mut [f32],
) -> crate::Result<()> {
    let classwords = classbook.dimensions as usize;
    let partition_size = residue.partition_size as usize;

    let mut partition_idx = 0usize;
    while partition_idx < partitions.count {
        let classifications = read_classifications(reader, classbook, classwords, residue.classifications)?;

        for j in 0..classwords {
            if partition_idx + j >= partitions.count {
                break;
            }

            let vqclass = classifications[j] as usize;
            let offset = partitions.begin + (partition_idx + j) * partition_size;

            // Pass 0 only (simplified — only read first pass)
            if residue.cascade[vqclass] & 1 != 0 {
                let book_idx = residue.books[vqclass][0];
                if book_idx >= 0 {
                    let book = &codebooks[book_idx as usize];
                    decode_partition_values(reader, book, target, offset, partition_size)?;
                }
            }
        }

        partition_idx += classwords;
    }

    Ok(())
}

/// Reads one classbook scalar and splits it into `classwords` per-partition
/// classifications, most significant first.
fn read_classifications(
    reader: &mut BitReader,
    classbook: &Codebook,
    classwords: usize,
    class_count: u8,
) -> crate::Result<Vec<u8>> {
    let temp = classbook.decode_scalar(reader)?;
    let mut classifications = vec![0u8; classwords];
    let mut t = temp;
    for j in (0..classwords).rev() {
        classifications[j] = (t % class_count as u32) as u8;
        t /= class_count as u32;
    }
    Ok(classifications)
}

/// Adds one partition's decoded values into `target[offset..]`, clipped to
/// the end of `target`.
fn decode_partition_values(
    reader: &mut BitReader,
    book: &Codebook,
    target: &mut [f32],
    offset: usize,
    partition_size: usize,
) -> crate::Result<()> {
    let limit = target.len();
    if book.lookup_type != 0 {
        // VQ decode
        let mut pos = offset;
        while pos < offset + partition_size && pos < limit {
            let vq = book.decode_vq(reader)?;
            for &v in vq {
                if pos < limit {
                    target[pos] += v;
                    pos += 1;
                }
            }
        }
    } else {
        // Scalar decode
        for pos in offset..(offset + partition_size).min(limit) {
            let val = book.decode_scalar(reader)?;
            target[pos] += val as f32;
        }
    }
    Ok(())
}
