//! Vorbis floor type 1: `decode_floor1_config` parses the setup-header config,
//! `decode_floor1_packet` rebuilds the per-channel spectral envelope from an audio packet.

// Vorbis spec § 7.2 uses indexed equations on the sorted x_list / y_list
// arrays. Iterator translation obscures the spec equations — keep
// index loops where the index is itself part of the equation.
#![allow(clippy::needless_range_loop)]

use super::bit_reader::{ilog, BitReader};
use super::codebook::Codebook;

#[derive(Debug, Clone)]
pub(super) struct Floor1Config {
    pub(super) partitions: u8,
    pub(super) partition_class: Vec<u8>,
    pub(super) class_dimensions: Vec<u8>,
    pub(super) class_subclasses: Vec<u8>,
    pub(super) class_masterbook: Vec<u8>,
    pub(super) class_subclass_books: Vec<Vec<i16>>,
    pub(super) multiplier: u8,
    pub(super) x_list: Vec<u16>,
    pub(super) sorted_order: Vec<usize>,
}

pub(super) fn decode_floor1_config(reader: &mut BitReader) -> crate::Result<Floor1Config> {
    let partitions = reader.read_bits(5)? as u8;
    let mut partition_class = Vec::with_capacity(partitions as usize);
    let mut max_class: i32 = -1;

    for _ in 0..partitions {
        let cls = reader.read_bits(4)? as u8;
        partition_class.push(cls);
        if cls as i32 > max_class {
            max_class = cls as i32;
        }
    }

    let num_classes = (max_class + 1) as usize;
    let mut class_dimensions = vec![0u8; num_classes];
    let mut class_subclasses = vec![0u8; num_classes];
    let mut class_masterbook = vec![0u8; num_classes];
    let mut class_subclass_books = vec![Vec::new(); num_classes];

    for i in 0..num_classes {
        class_dimensions[i] = reader.read_bits(3)? as u8 + 1;
        class_subclasses[i] = reader.read_bits(2)? as u8;
        if class_subclasses[i] != 0 {
            class_masterbook[i] = reader.read_bits(8)? as u8;
        }
        let num_sub = 1usize << class_subclasses[i];
        let mut books = Vec::with_capacity(num_sub);
        for _ in 0..num_sub {
            books.push(reader.read_bits(8)? as i16 - 1);
        }
        class_subclass_books[i] = books;
    }

    let multiplier = reader.read_bits(2)? as u8 + 1;
    let rangebits = reader.read_bits(4)?;

    let mut x_list = vec![0u16, 1u16 << rangebits];

    for i in 0..partitions as usize {
        let cls = partition_class[i] as usize;
        let dim = class_dimensions[cls];
        for _ in 0..dim {
            x_list.push(reader.read_bits(rangebits)? as u16);
        }
    }

    // Compute sorted order
    let mut sorted_order: Vec<usize> = (0..x_list.len()).collect();
    sorted_order.sort_by_key(|&i| x_list[i]);

    Ok(Floor1Config {
        partitions,
        partition_class,
        class_dimensions,
        class_subclasses,
        class_masterbook,
        class_subclass_books,
        multiplier,
        x_list,
        sorted_order,
    })
}

pub(super) fn decode_floor1_packet(
    reader: &mut BitReader,
    floor: &Floor1Config,
    codebooks: &[Codebook],
    n_half: usize,
) -> crate::Result<Option<Vec<f32>>> {
    let nonzero = reader.read_bit()?;
    if !nonzero {
        return Ok(None); // This channel is unused in this frame
    }

    let range = match floor.multiplier {
        1 => 256u32,
        2 => 128,
        3 => 86,
        4 => 64,
        _ => return Err(format!("Invalid floor multiplier: {}", floor.multiplier).into()),
    };

    let range_bits = ilog(range - 1);

    // Read Y values for each X position
    let mut y_list = vec![0i32; floor.x_list.len()];
    y_list[0] = reader.read_bits(range_bits)? as i32;
    y_list[1] = reader.read_bits(range_bits)? as i32;

    let mut offset = 2;
    for i in 0..floor.partitions as usize {
        let cls = floor.partition_class[i] as usize;
        let dim = floor.class_dimensions[cls] as usize;
        let subclass_bits = floor.class_subclasses[cls];

        let cval = if subclass_bits > 0 {
            let masterbook_idx = floor.class_masterbook[cls] as usize;
            if masterbook_idx >= codebooks.len() {
                return Err("Floor masterbook index out of range".into());
            }
            codebooks[masterbook_idx].decode_scalar(reader)?
        } else {
            0
        };

        for j in 0..dim {
            let subclass = (cval >> (j as u32 * subclass_bits as u32))
                & ((1 << subclass_bits) - 1);
            let book_idx = floor.class_subclass_books[cls][subclass as usize];

            if book_idx >= 0 {
                let book_idx = book_idx as usize;
                if book_idx >= codebooks.len() {
                    return Err("Floor subclass book index out of range".into());
                }
                y_list[offset] = codebooks[book_idx].decode_scalar(reader)? as i32;
            } else {
                y_list[offset] = 0;
            }
            offset += 1;
        }
    }

    // Amplitude synthesis: convert Y values to the actual floor curve
    // Simplified: render as a piecewise linear curve
    let mut floor_output = vec![0.0f32; n_half];

    // Sort x_list positions and compute the floor curve
    let n_points = floor.x_list.len();
    let mut final_y = vec![0i32; n_points];

    // Step 1: Amplitude value synthesis (simplified — just use raw y values)
    final_y[..n_points].copy_from_slice(&y_list[..n_points]);

    // Step 2: Curve synthesis — iterate through sorted x positions
    let sorted = &floor.sorted_order;

    // Render piecewise linear from sorted x positions
    let mult = floor.multiplier as f32;
    let mut lx = 0usize;
    let mut ly = final_y[sorted[0]] as f32 * mult;

    for i in 1..sorted.len() {
        let hx = floor.x_list[sorted[i]] as usize;
        let hy = final_y[sorted[i]] as f32 * mult;

        if hx >= n_half {
            // Fill to end
            for x in lx..n_half {
                let t = if hx > lx {
                    (x - lx) as f32 / (hx - lx) as f32
                } else {
                    0.0
                };
                let db = ly + t * (hy - ly);
                floor_output[x] = floor1_inverse_db(db);
            }
            break;
        }

        for x in lx..hx {
            let t = if hx > lx {
                (x - lx) as f32 / (hx - lx) as f32
            } else {
                0.0
            };
            let db = ly + t * (hy - ly);
            floor_output[x] = floor1_inverse_db(db);
        }

        lx = hx;
        ly = hy;
    }

    // Fill remaining
    if lx < n_half {
        let db = ly;
        for x in lx..n_half {
            floor_output[x] = floor1_inverse_db(db);
        }
    }

    Ok(Some(floor_output))
}

/// Static floor1 inverse dB lookup (simplified exponential approximation).
pub(super) fn floor1_inverse_db(val: f32) -> f32 {
    // In Vorbis, the inverse dB table maps integer values to linear amplitude.
    // val should be in [0, multiplier*range).
    // Simplified: treat as a dB-like scale
    if val <= 0.0 {
        return 0.0;
    }
    // Vorbis uses a specific lookup table; we approximate:
    // The Vorbis spec floor1_inverse_dB_table maps values 0..255 to amplitudes.
    // Approximation: 10^((val - 140) / 40)
    let db = (val - 140.0) / 40.0;
    (10.0f32).powf(db).min(1.0)
}
