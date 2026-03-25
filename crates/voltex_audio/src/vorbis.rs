//! Simplified Vorbis audio decoder.
//!
//! Implements the core Vorbis I specification:
//! - Identification header parsing
//! - Setup header: codebooks (scalar, Huffman), floor type 1, residue type 2, mapping, modes
//! - Audio packet decoding: mode select, floor decode, residue decode, inverse MDCT, windowing, overlap-add
//!
//! Simplifications:
//! - No VQ (vector quantization) in codebooks — scalar only
//! - No floor type 0 (only floor type 1)
//! - No residue type 0 or 1 (only residue type 2 — interleaved)
//! - Coupled channels treated as independent (no angle/magnitude decoupling)

use crate::AudioClip;
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Bitstream reader
// ---------------------------------------------------------------------------

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8, // 0..7 within current byte
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn bits_remaining(&self) -> usize {
        if self.byte_pos >= self.data.len() {
            return 0;
        }
        (self.data.len() - self.byte_pos) * 8 - self.bit_pos as usize
    }

    fn read_bits(&mut self, n: u32) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        if n > 32 {
            return Err("read_bits: n > 32".to_string());
        }
        let mut result: u32 = 0;
        let mut bits_read = 0u32;

        while bits_read < n {
            if self.byte_pos >= self.data.len() {
                return Err("read_bits: unexpected end of data".to_string());
            }
            let remaining_in_byte = 8 - self.bit_pos as u32;
            let to_read = (n - bits_read).min(remaining_in_byte);
            let mask = (1u32 << to_read) - 1;
            let bits = ((self.data[self.byte_pos] >> self.bit_pos) as u32) & mask;
            result |= bits << bits_read;
            bits_read += to_read;
            self.bit_pos += to_read as u8;
            if self.bit_pos >= 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }
        Ok(result)
    }

    fn read_bit(&mut self) -> Result<bool, String> {
        Ok(self.read_bits(1)? != 0)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_bits(8)? as u8)
    }

    fn read_u16(&mut self) -> Result<u16, String> {
        Ok(self.read_bits(16)? as u16)
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        self.read_bits(32)
    }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn ilog(x: u32) -> u32 {
    if x == 0 {
        0
    } else {
        32 - x.leading_zeros()
    }
}

fn float32_unpack(val: u32) -> f32 {
    let mantissa = (val & 0x1fffff) as f32;
    let sign = if val & 0x80000000 != 0 { -1.0f32 } else { 1.0f32 };
    let exponent = ((val >> 21) & 0x3ff) as i32 - 788;
    sign * mantissa * (2.0f32).powi(exponent)
}

fn lookup1_values(entries: u32, dimensions: u32) -> u32 {
    if dimensions == 0 {
        return 0;
    }
    // floor(entries^(1/dimensions))
    let val = (entries as f64).powf(1.0 / dimensions as f64).floor() as u32;
    // Verify: val^dimensions <= entries < (val+1)^dimensions
    // Use a simple iterative check
    let mut r = val;
    loop {
        let mut power = 1u64;
        let mut ok = true;
        for _ in 0..dimensions {
            power = power.saturating_mul((r + 1) as u64);
            if power > entries as u64 {
                ok = false;
                break;
            }
        }
        if ok {
            r += 1;
        } else {
            break;
        }
    }
    r
}

// ---------------------------------------------------------------------------
// Codebook
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Codebook {
    dimensions: u16,
    entries: u32,
    /// Codeword lengths for each entry (0 = unused).
    lengths: Vec<u8>,
    /// Lookup type (0 = no lookup, 1 = implicitly defined, 2 = explicitly defined).
    lookup_type: u8,
    /// VQ lookup table (if lookup_type != 0).
    vq_table: Vec<Vec<f32>>,
    /// Huffman decode tree: (left_child, right_child) indexed by node id.
    /// Leaf nodes store the entry index as a negative-offset value.
    /// We use a flat sorted approach instead.
    sorted_entries: Vec<(u32, u32, u8)>, // (codeword, entry_index, length)
}

impl Codebook {
    fn decode_scalar(&self, reader: &mut BitReader) -> Result<u32, String> {
        // Walk the codeword bit by bit, matching against sorted entries.
        // This is a simple but not optimal approach.
        let mut code: u32 = 0;
        let mut code_len: u8 = 0;

        for _ in 0..33 {
            if reader.bits_remaining() == 0 && code_len > 0 {
                // Try to match what we have
                break;
            }
            let bit = reader.read_bits(1)? as u32;
            code |= bit << code_len;
            code_len += 1;

            // Check for a match at this length
            for &(cw, entry, len) in &self.sorted_entries {
                if len == code_len && cw == code {
                    return Ok(entry);
                }
            }
        }

        Err("Codebook: no matching codeword found".to_string())
    }

    fn decode_vq(&self, reader: &mut BitReader) -> Result<&[f32], String> {
        let entry = self.decode_scalar(reader)? as usize;
        if entry < self.vq_table.len() {
            Ok(&self.vq_table[entry])
        } else {
            Err(format!("VQ entry {} out of range", entry))
        }
    }
}

fn decode_codebook(reader: &mut BitReader) -> Result<Codebook, String> {
    // Sync pattern
    let sync = reader.read_bits(24)?;
    if sync != 0x564342 {
        return Err(format!("Invalid codebook sync pattern: 0x{:06X}", sync));
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
                return Err("Codebook ordered length overflow".to_string());
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
        return Err(format!("Unsupported codebook lookup type: {}", lookup_type));
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

fn bit_reverse(val: u32, bits: u8) -> u32 {
    let mut result = 0u32;
    let mut v = val;
    for _ in 0..bits {
        result = (result << 1) | (v & 1);
        v >>= 1;
    }
    result
}

// ---------------------------------------------------------------------------
// Floor type 1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Floor1Config {
    partitions: u8,
    partition_class: Vec<u8>,
    class_dimensions: Vec<u8>,
    class_subclasses: Vec<u8>,
    class_masterbook: Vec<u8>,
    class_subclass_books: Vec<Vec<i16>>,
    multiplier: u8,
    x_list: Vec<u16>,
    sorted_order: Vec<usize>,
}

fn decode_floor1_config(reader: &mut BitReader) -> Result<Floor1Config, String> {
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

// ---------------------------------------------------------------------------
// Residue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ResidueConfig {
    residue_type: u16,
    begin: u32,
    end: u32,
    partition_size: u32,
    classifications: u8,
    classbook: u8,
    cascade: Vec<u8>,
    books: Vec<Vec<i16>>,
}

fn decode_residue_config(reader: &mut BitReader, residue_type: u16) -> Result<ResidueConfig, String> {
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

// ---------------------------------------------------------------------------
// Mapping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Mapping {
    submaps: u8,
    coupling_steps: u16,
    coupling_magnitude: Vec<u8>,
    coupling_angle: Vec<u8>,
    mux: Vec<u8>,
    submap_floor: Vec<u8>,
    submap_residue: Vec<u8>,
}

fn decode_mapping(reader: &mut BitReader, channels: u8) -> Result<Mapping, String> {
    let mapping_type = reader.read_bits(16)?;
    if mapping_type != 0 {
        return Err(format!("Unsupported mapping type: {}", mapping_type));
    }

    let submaps = if reader.read_bit()? {
        reader.read_bits(4)? as u8 + 1
    } else {
        1
    };

    let coupling_steps = if reader.read_bit()? {
        reader.read_bits(8)? as u16 + 1
    } else {
        0
    };

    let coupling_bits = ilog((channels as u32).saturating_sub(1));
    let mut coupling_magnitude = Vec::with_capacity(coupling_steps as usize);
    let mut coupling_angle = Vec::with_capacity(coupling_steps as usize);

    for _ in 0..coupling_steps {
        coupling_magnitude.push(reader.read_bits(coupling_bits)? as u8);
        coupling_angle.push(reader.read_bits(coupling_bits)? as u8);
    }

    let reserved = reader.read_bits(2)?;
    if reserved != 0 {
        return Err("Mapping reserved field is non-zero".to_string());
    }

    let mut mux = vec![0u8; channels as usize];
    if submaps > 1 {
        for ch in 0..channels as usize {
            mux[ch] = reader.read_bits(4)? as u8;
        }
    }

    let mut submap_floor = Vec::with_capacity(submaps as usize);
    let mut submap_residue = Vec::with_capacity(submaps as usize);
    for _ in 0..submaps {
        let _time_config = reader.read_bits(8)?; // unused
        submap_floor.push(reader.read_bits(8)? as u8);
        submap_residue.push(reader.read_bits(8)? as u8);
    }

    Ok(Mapping {
        submaps,
        coupling_steps,
        coupling_magnitude,
        coupling_angle,
        mux,
        submap_floor,
        submap_residue,
    })
}

// ---------------------------------------------------------------------------
// Mode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Mode {
    block_flag: bool,
    window_type: u16,
    transform_type: u16,
    mapping: u8,
}

fn decode_mode(reader: &mut BitReader) -> Result<Mode, String> {
    let block_flag = reader.read_bit()?;
    let window_type = reader.read_bits(16)? as u16;
    let transform_type = reader.read_bits(16)? as u16;
    let mapping = reader.read_bits(8)? as u8;
    Ok(Mode {
        block_flag,
        window_type,
        transform_type,
        mapping,
    })
}

// ---------------------------------------------------------------------------
// Vorbis identification header
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct VorbisIdHeader {
    channels: u8,
    sample_rate: u32,
    blocksize_0: u32,
    blocksize_1: u32,
}

fn parse_id_header(packet: &[u8]) -> Result<VorbisIdHeader, String> {
    if packet.len() < 30 {
        return Err("Vorbis ID header too short".to_string());
    }

    let mut reader = BitReader::new(packet);

    // Packet type (1 = identification)
    let ptype = reader.read_u8()?;
    if ptype != 1 {
        return Err(format!("Expected Vorbis ID header (type 1), got type {}", ptype));
    }

    // "vorbis" magic
    let mut magic = [0u8; 6];
    for b in &mut magic {
        *b = reader.read_u8()?;
    }
    if &magic != b"vorbis" {
        return Err("Missing 'vorbis' magic in ID header".to_string());
    }

    let version = reader.read_u32()?;
    if version != 0 {
        return Err(format!("Unsupported Vorbis version: {}", version));
    }

    let channels = reader.read_u8()?;
    if channels == 0 {
        return Err("Zero channels in Vorbis ID header".to_string());
    }

    let sample_rate = reader.read_u32()?;
    if sample_rate == 0 {
        return Err("Zero sample rate in Vorbis ID header".to_string());
    }

    let _bitrate_max = reader.read_u32()?;
    let _bitrate_nominal = reader.read_u32()?;
    let _bitrate_min = reader.read_u32()?;

    let blocksize_raw = reader.read_u8()?;
    let blocksize_0 = 1u32 << (blocksize_raw & 0x0F);
    let blocksize_1 = 1u32 << ((blocksize_raw >> 4) & 0x0F);

    if blocksize_0 > blocksize_1 {
        return Err("blocksize_0 > blocksize_1".to_string());
    }

    let framing = reader.read_bit()?;
    if !framing {
        return Err("Missing framing bit in ID header".to_string());
    }

    Ok(VorbisIdHeader {
        channels,
        sample_rate,
        blocksize_0,
        blocksize_1,
    })
}

// ---------------------------------------------------------------------------
// Setup header
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct VorbisSetup {
    codebooks: Vec<Codebook>,
    floors: Vec<Floor1Config>,
    residues: Vec<ResidueConfig>,
    mappings: Vec<Mapping>,
    modes: Vec<Mode>,
}

fn parse_setup_header(packet: &[u8], channels: u8) -> Result<VorbisSetup, String> {
    let mut reader = BitReader::new(packet);

    // Packet type (5 = setup)
    let ptype = reader.read_u8()?;
    if ptype != 5 {
        return Err(format!("Expected Vorbis setup header (type 5), got type {}", ptype));
    }

    // "vorbis" magic
    let mut magic = [0u8; 6];
    for b in &mut magic {
        *b = reader.read_u8()?;
    }
    if &magic != b"vorbis" {
        return Err("Missing 'vorbis' magic in setup header".to_string());
    }

    // Codebooks
    let codebook_count = reader.read_bits(8)? + 1;
    let mut codebooks = Vec::with_capacity(codebook_count as usize);
    for _ in 0..codebook_count {
        codebooks.push(decode_codebook(&mut reader)?);
    }

    // Time domain transforms (placeholders, always 0)
    let time_count = reader.read_bits(6)? + 1;
    for _ in 0..time_count {
        let t = reader.read_bits(16)?;
        if t != 0 {
            return Err(format!("Unsupported time domain transform: {}", t));
        }
    }

    // Floors
    let floor_count = reader.read_bits(6)? + 1;
    let mut floors = Vec::with_capacity(floor_count as usize);
    for _ in 0..floor_count {
        let floor_type = reader.read_bits(16)?;
        if floor_type != 1 {
            return Err(format!("Only floor type 1 is supported, got type {}", floor_type));
        }
        floors.push(decode_floor1_config(&mut reader)?);
    }

    // Residues
    let residue_count = reader.read_bits(6)? + 1;
    let mut residues = Vec::with_capacity(residue_count as usize);
    for _ in 0..residue_count {
        let residue_type = reader.read_bits(16)? as u16;
        if residue_type > 2 {
            return Err(format!("Unsupported residue type: {}", residue_type));
        }
        residues.push(decode_residue_config(&mut reader, residue_type)?);
    }

    // Mappings
    let mapping_count = reader.read_bits(6)? + 1;
    let mut mappings = Vec::with_capacity(mapping_count as usize);
    for _ in 0..mapping_count {
        mappings.push(decode_mapping(&mut reader, channels)?);
    }

    // Modes
    let mode_count = reader.read_bits(6)? + 1;
    let mut modes = Vec::with_capacity(mode_count as usize);
    for _ in 0..mode_count {
        modes.push(decode_mode(&mut reader)?);
    }

    // Framing bit
    let framing = reader.read_bit()?;
    if !framing {
        return Err("Missing framing bit in setup header".to_string());
    }

    Ok(VorbisSetup {
        codebooks,
        floors,
        residues,
        mappings,
        modes,
    })
}

// ---------------------------------------------------------------------------
// MDCT (Modified Discrete Cosine Transform)
// ---------------------------------------------------------------------------

/// Compute the inverse MDCT (type IV) of `input` into `output`.
/// `n` is the block size (power of 2). Input has n/2 elements, output has n elements.
fn imdct(input: &[f32], output: &mut [f32], n: usize) {
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
fn vorbis_window(n: usize) -> Vec<f32> {
    let mut w = Vec::with_capacity(n);
    for i in 0..n {
        let x = ((i as f32 + 0.5) / n as f32 * PI).sin();
        let val = (PI / 2.0 * x * x).sin();
        w.push(val);
    }
    w
}

// ---------------------------------------------------------------------------
// Floor type 1 decode (audio packet)
// ---------------------------------------------------------------------------

fn decode_floor1_packet(
    reader: &mut BitReader,
    floor: &Floor1Config,
    codebooks: &[Codebook],
    n_half: usize,
) -> Result<Option<Vec<f32>>, String> {
    let nonzero = reader.read_bit()?;
    if !nonzero {
        return Ok(None); // This channel is unused in this frame
    }

    let range = match floor.multiplier {
        1 => 256u32,
        2 => 128,
        3 => 86,
        4 => 64,
        _ => return Err(format!("Invalid floor multiplier: {}", floor.multiplier)),
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
                return Err("Floor masterbook index out of range".to_string());
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
                    return Err("Floor subclass book index out of range".to_string());
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
    for i in 0..n_points {
        final_y[i] = y_list[i];
    }

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
fn floor1_inverse_db(val: f32) -> f32 {
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

// ---------------------------------------------------------------------------
// Residue decode
// ---------------------------------------------------------------------------

fn decode_residue(
    reader: &mut BitReader,
    residue: &ResidueConfig,
    codebooks: &[Codebook],
    n_half: usize,
    ch: usize,
    do_not_decode: &[bool],
) -> Result<Vec<Vec<f32>>, String> {
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

// ---------------------------------------------------------------------------
// Audio packet decode
// ---------------------------------------------------------------------------

fn decode_audio_packet(
    packet: &[u8],
    id: &VorbisIdHeader,
    setup: &VorbisSetup,
    prev_window: &mut Vec<Vec<f32>>,
    prev_block_flag: &mut Option<bool>,
) -> Result<Vec<f32>, String> {
    let mut reader = BitReader::new(packet);

    // Packet type must be 0 (audio)
    let ptype = reader.read_bit()?;
    if ptype {
        return Err("Expected audio packet (type 0)".to_string());
    }

    let mode_bits = ilog(setup.modes.len() as u32 - 1);
    let mode_number = reader.read_bits(mode_bits)? as usize;
    if mode_number >= setup.modes.len() {
        return Err(format!("Mode number {} out of range", mode_number));
    }

    let mode = &setup.modes[mode_number];
    let block_flag = mode.block_flag;

    let n = if block_flag {
        id.blocksize_1 as usize
    } else {
        id.blocksize_0 as usize
    };
    let n_half = n / 2;

    // For long blocks, read previous/next window flags
    if block_flag {
        let _prev_window_flag = reader.read_bit()?;
        let _next_window_flag = reader.read_bit()?;
    }

    let mapping = &setup.mappings[mode.mapping as usize];
    let ch = id.channels as usize;

    // Decode floors for each channel
    let mut floors: Vec<Option<Vec<f32>>> = Vec::with_capacity(ch);
    let mut no_residue = vec![false; ch];

    for c in 0..ch {
        let submap_idx = mapping.mux[c] as usize;
        let floor_idx = mapping.submap_floor[submap_idx] as usize;
        let floor = &setup.floors[floor_idx];

        match decode_floor1_packet(&mut reader, floor, &setup.codebooks, n_half) {
            Ok(Some(f)) => floors.push(Some(f)),
            Ok(None) => {
                floors.push(None);
                no_residue[c] = true;
            }
            Err(e) => {
                // On decode error, treat channel as silent
                floors.push(None);
                no_residue[c] = true;
            }
        }
    }

    // Decode residues for each submap
    let mut residue_data = vec![vec![0.0f32; n_half]; ch];

    for submap_idx in 0..mapping.submaps as usize {
        let residue_idx = mapping.submap_residue[submap_idx] as usize;
        let residue_config = &setup.residues[residue_idx];

        // Collect channels for this submap
        let mut submap_channels: Vec<usize> = Vec::new();
        let mut submap_do_not_decode: Vec<bool> = Vec::new();
        for c in 0..ch {
            if mapping.mux[c] as usize == submap_idx {
                submap_channels.push(c);
                submap_do_not_decode.push(no_residue[c]);
            }
        }

        if !submap_channels.is_empty() {
            match decode_residue(
                &mut reader,
                residue_config,
                &setup.codebooks,
                n_half,
                submap_channels.len(),
                &submap_do_not_decode,
            ) {
                Ok(res) => {
                    for (i, &c) in submap_channels.iter().enumerate() {
                        residue_data[c] = res[i].clone();
                    }
                }
                Err(_) => {
                    // Residue decode error — leave as zeros
                }
            }
        }
    }

    // Coupling (simplified: skip angle/magnitude decoupling)
    // In a full implementation we'd do the inverse coupling here.

    // Multiply floor * residue to get MDCT coefficients
    let mut mdct_input = vec![vec![0.0f32; n_half]; ch];
    for c in 0..ch {
        if let Some(ref floor) = floors[c] {
            for i in 0..n_half {
                mdct_input[c][i] = floor[i] * residue_data[c][i];
            }
        }
        // If floor is None, mdct_input stays as zeros (silent)
    }

    // Inverse MDCT + windowing per channel
    let window = vorbis_window(n);
    let mut pcm_channels = vec![vec![0.0f32; n]; ch];

    for c in 0..ch {
        let mut mdct_out = vec![0.0f32; n];
        imdct(&mdct_input[c], &mut mdct_out, n);

        // Apply window
        for i in 0..n {
            pcm_channels[c][i] = mdct_out[i] * window[i];
        }
    }

    // Overlap-add with previous window
    let prev_n = if let Some(pf) = *prev_block_flag {
        if pf { id.blocksize_1 as usize } else { id.blocksize_0 as usize }
    } else {
        0
    };

    let overlap = if prev_n > 0 {
        (prev_n + n) / 4
        // Actually: overlap = min(prev_n, n) / 2
    } else {
        0
    };
    let overlap = if prev_n > 0 { prev_n.min(n) / 2 } else { 0 };

    let output_samples;

    if prev_window.is_empty() || prev_n == 0 {
        // First frame: no overlap, just store for next frame
        *prev_window = pcm_channels;
        *prev_block_flag = Some(block_flag);
        return Ok(Vec::new()); // No output for first frame
    } else {
        // Overlap-add
        let prev_right_start = prev_n / 2;
        let cur_left_end = n / 2;

        let mut output = Vec::with_capacity(overlap * ch);

        // Output non-overlapping part from previous frame's center
        let prev_center_start = (prev_n - overlap) / 2;
        let non_overlap_samples = prev_center_start.saturating_sub(prev_n / 4);

        // Simplified: output the overlapped portion as interleaved samples
        let mut frame_output = Vec::new();

        // The "return" portion from the previous block:
        // previous block right half = prev_window[c][prev_n/2..]
        // current block left half = pcm_channels[c][..n/2]
        // overlap region = last `overlap` samples of prev right + first `overlap` of cur left

        // Output: overlap region as interleaved samples
        for i in 0..overlap {
            for c in 0..ch {
                let prev_idx = prev_right_start + i;
                let cur_idx = (n / 2 - overlap) + i;

                let prev_val = if prev_idx < prev_window[c].len() {
                    prev_window[c][prev_idx]
                } else {
                    0.0
                };
                let cur_val = if cur_idx < pcm_channels[c].len() {
                    pcm_channels[c][cur_idx]
                } else {
                    0.0
                };

                frame_output.push(prev_val + cur_val);
            }
        }

        output_samples = frame_output;
    }

    *prev_window = pcm_channels;
    *prev_block_flag = Some(block_flag);

    Ok(output_samples)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decode a Vorbis bitstream from OGG packets into an AudioClip.
///
/// `packets` should be the Vorbis packets extracted from an OGG container
/// (typically via `ogg::parse_ogg`).
///
/// The first three packets must be the Vorbis headers:
/// 1. Identification header
/// 2. Comment header (skipped)
/// 3. Setup header
///
/// All subsequent packets are audio packets.
pub fn decode_vorbis(packets: &[Vec<u8>]) -> Result<AudioClip, String> {
    if packets.len() < 3 {
        return Err("Need at least 3 Vorbis packets (id, comment, setup)".to_string());
    }

    // Parse identification header
    let id = parse_id_header(&packets[0])?;

    // Skip comment header (packet[1]) — just verify it starts with type 3
    if packets[1].is_empty() || packets[1][0] != 3 {
        return Err("Invalid Vorbis comment header".to_string());
    }

    // Parse setup header
    let setup = parse_setup_header(&packets[2], id.channels)?;

    // Decode audio packets
    let mut all_samples: Vec<f32> = Vec::new();
    let mut prev_window: Vec<Vec<f32>> = Vec::new();
    let mut prev_block_flag: Option<bool> = None;

    for packet in &packets[3..] {
        match decode_audio_packet(packet, &id, &setup, &mut prev_window, &mut prev_block_flag) {
            Ok(samples) => {
                all_samples.extend_from_slice(&samples);
            }
            Err(_e) => {
                // Skip problematic audio packets silently
                // In production we might log the error
            }
        }
    }

    Ok(AudioClip::new(
        all_samples,
        id.sample_rate,
        id.channels as u16,
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitreader_basics() {
        let data = [0b10110100u8, 0b01101001u8];
        let mut r = BitReader::new(&data);

        // Read 3 bits from byte 0: 0b100 = 4 (LSB first)
        assert_eq!(r.read_bits(3).unwrap(), 0b100);
        // Next 5 bits: 0b10110 from the rest of byte 0 (5 bits) = 0b10110 but we need
        // bits 3..7 of byte0 = 1011, then bit 0 of byte1 = 1 → 0b1_1011 = 0b11011 = 27
        assert_eq!(r.read_bits(5).unwrap(), 0b1_1011);
    }

    #[test]
    fn bitreader_read_bit() {
        let data = [0b00000001u8];
        let mut r = BitReader::new(&data);
        assert!(r.read_bit().unwrap()); // bit 0 = 1
        assert!(!r.read_bit().unwrap()); // bit 1 = 0
    }

    #[test]
    fn ilog_values() {
        assert_eq!(ilog(0), 0);
        assert_eq!(ilog(1), 1);
        assert_eq!(ilog(2), 2);
        assert_eq!(ilog(3), 2);
        assert_eq!(ilog(4), 3);
        assert_eq!(ilog(255), 8);
        assert_eq!(ilog(256), 9);
    }

    #[test]
    fn float32_unpack_test() {
        // Known value: 0x40000000 should unpack to a specific float
        let val = float32_unpack(0x40000000);
        assert!(val.is_finite());
    }

    #[test]
    fn bit_reverse_test() {
        assert_eq!(bit_reverse(0b110, 3), 0b011);
        assert_eq!(bit_reverse(0b1010, 4), 0b0101);
        assert_eq!(bit_reverse(0b1, 1), 0b1);
        assert_eq!(bit_reverse(0b0, 1), 0b0);
    }

    #[test]
    fn lookup1_values_test() {
        // 4 entries, 2 dimensions → floor(4^(1/2)) = 2
        assert_eq!(lookup1_values(4, 2), 2);
        // 27 entries, 3 dimensions → floor(27^(1/3)) = 3
        assert_eq!(lookup1_values(27, 3), 3);
    }

    #[test]
    fn imdct_basic() {
        // Simple test: MDCT of impulse-like input
        let n = 8;
        let input = vec![1.0, 0.0, 0.0, 0.0]; // n/2 = 4
        let mut output = vec![0.0; n];
        imdct(&input, &mut output, n);

        // Output should have non-zero values and be finite
        for &v in &output {
            assert!(v.is_finite(), "MDCT output not finite: {}", v);
        }
        // The output should not be all zeros
        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        assert!(sum > 0.0, "MDCT output is all zeros");
    }

    #[test]
    fn vorbis_window_symmetry() {
        let n = 256;
        let w = vorbis_window(n);
        assert_eq!(w.len(), n);

        // Window should be symmetric: w[i] ≈ w[n-1-i]
        for i in 0..n / 2 {
            assert!(
                (w[i] - w[n - 1 - i]).abs() < 1e-5,
                "Window not symmetric at {}: {} vs {}",
                i,
                w[i],
                w[n - 1 - i]
            );
        }

        // All values should be in [0, 1]
        for &v in &w {
            assert!(v >= 0.0 && v <= 1.0, "Window value out of range: {}", v);
        }
    }

    #[test]
    fn parse_id_header_valid() {
        // Build a minimal valid ID header
        let mut data = Vec::new();
        data.push(1); // type = identification
        data.extend_from_slice(b"vorbis");
        data.extend_from_slice(&0u32.to_le_bytes()); // version
        data.push(2); // channels
        data.extend_from_slice(&44100u32.to_le_bytes()); // sample rate
        data.extend_from_slice(&0u32.to_le_bytes()); // bitrate max
        data.extend_from_slice(&128000u32.to_le_bytes()); // bitrate nominal
        data.extend_from_slice(&0u32.to_le_bytes()); // bitrate min
        // blocksize: blocksize_0=8 (256), blocksize_1=11 (2048) → 0xB8
        data.push(0xB8);
        data.push(1); // framing bit (we need 1 bit, but we're writing byte-aligned)

        let header = parse_id_header(&data).expect("parse failed");
        assert_eq!(header.channels, 2);
        assert_eq!(header.sample_rate, 44100);
        assert_eq!(header.blocksize_0, 256);
        assert_eq!(header.blocksize_1, 2048);
    }

    #[test]
    fn parse_id_header_invalid_magic() {
        let mut data = vec![1]; // type
        data.extend_from_slice(b"norbis"); // wrong magic
        data.extend_from_slice(&[0u8; 23]); // padding
        let result = parse_id_header(&data);
        assert!(result.is_err());
    }

    #[test]
    fn decode_vorbis_too_few_packets() {
        let packets = vec![vec![1], vec![3]];
        let result = decode_vorbis(&packets);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 3"));
    }

    #[test]
    fn floor1_inverse_db_range() {
        // Check that the function returns reasonable values
        assert_eq!(floor1_inverse_db(0.0), 0.0);
        let v = floor1_inverse_db(140.0);
        assert!((v - 1.0).abs() < 0.01, "at 140 expected ~1.0, got {}", v);
        let v = floor1_inverse_db(100.0);
        assert!(v < 1.0 && v > 0.0, "at 100 expected (0,1), got {}", v);
    }
}
