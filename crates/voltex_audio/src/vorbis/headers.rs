// ---------------------------------------------------------------------------
// Vorbis headers (id + setup) and the per-frame Mapping/Mode tables they own.
// ---------------------------------------------------------------------------

use super::bit_reader::{ilog, BitReader};
use super::codebook::{decode_codebook, Codebook};
use super::floor::{decode_floor1_config, Floor1Config};
use super::residue::{decode_residue_config, ResidueConfig};

// ---------------------------------------------------------------------------
// Mapping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(super) struct Mapping {
    pub(super) submaps: u8,
    // Coupling fields parsed per Vorbis spec but not consumed by the
    // current decoder path; retained for stereo-coupling decode (v1.1+).
    #[allow(dead_code)]
    pub(super) coupling_steps: u16,
    #[allow(dead_code)]
    pub(super) coupling_magnitude: Vec<u8>,
    #[allow(dead_code)]
    pub(super) coupling_angle: Vec<u8>,
    pub(super) mux: Vec<u8>,
    pub(super) submap_floor: Vec<u8>,
    pub(super) submap_residue: Vec<u8>,
}

pub(super) fn decode_mapping(reader: &mut BitReader, channels: u8) -> crate::Result<Mapping> {
    let mapping_type = reader.read_bits(16)?;
    if mapping_type != 0 {
        return Err(format!("Unsupported mapping type: {}", mapping_type).into());
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
        return Err("Mapping reserved field is non-zero".into());
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
pub(super) struct Mode {
    pub(super) block_flag: bool,
    // Vorbis I § 4.2.4 forces both fields to 0; parsed and retained for
    // forward compatibility but the decoder never branches on them.
    #[allow(dead_code)]
    pub(super) window_type: u16,
    #[allow(dead_code)]
    pub(super) transform_type: u16,
    pub(super) mapping: u8,
}

pub(super) fn decode_mode(reader: &mut BitReader) -> crate::Result<Mode> {
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
pub(super) struct VorbisIdHeader {
    pub(super) channels: u8,
    pub(super) sample_rate: u32,
    pub(super) blocksize_0: u32,
    pub(super) blocksize_1: u32,
}

pub(super) fn parse_id_header(packet: &[u8]) -> crate::Result<VorbisIdHeader> {
    if packet.len() < 30 {
        return Err("Vorbis ID header too short".into());
    }

    let mut reader = BitReader::new(packet);

    // Packet type (1 = identification)
    let ptype = reader.read_u8()?;
    if ptype != 1 {
        return Err(format!("Expected Vorbis ID header (type 1), got type {}", ptype).into());
    }

    // "vorbis" magic
    let mut magic = [0u8; 6];
    for b in &mut magic {
        *b = reader.read_u8()?;
    }
    if &magic != b"vorbis" {
        return Err("Missing 'vorbis' magic in ID header".into());
    }

    let version = reader.read_u32()?;
    if version != 0 {
        return Err(format!("Unsupported Vorbis version: {}", version).into());
    }

    let channels = reader.read_u8()?;
    if channels == 0 {
        return Err("Zero channels in Vorbis ID header".into());
    }

    let sample_rate = reader.read_u32()?;
    if sample_rate == 0 {
        return Err("Zero sample rate in Vorbis ID header".into());
    }

    let _bitrate_max = reader.read_u32()?;
    let _bitrate_nominal = reader.read_u32()?;
    let _bitrate_min = reader.read_u32()?;

    let blocksize_raw = reader.read_u8()?;
    let blocksize_0 = 1u32 << (blocksize_raw & 0x0F);
    let blocksize_1 = 1u32 << ((blocksize_raw >> 4) & 0x0F);

    if blocksize_0 > blocksize_1 {
        return Err("blocksize_0 > blocksize_1".into());
    }

    let framing = reader.read_bit()?;
    if !framing {
        return Err("Missing framing bit in ID header".into());
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
pub(super) struct VorbisSetup {
    pub(super) codebooks: Vec<Codebook>,
    pub(super) floors: Vec<Floor1Config>,
    pub(super) residues: Vec<ResidueConfig>,
    pub(super) mappings: Vec<Mapping>,
    pub(super) modes: Vec<Mode>,
}

pub(super) fn parse_setup_header(packet: &[u8], channels: u8) -> crate::Result<VorbisSetup> {
    let mut reader = BitReader::new(packet);

    // Packet type (5 = setup)
    let ptype = reader.read_u8()?;
    if ptype != 5 {
        return Err(format!("Expected Vorbis setup header (type 5), got type {}", ptype).into());
    }

    // "vorbis" magic
    let mut magic = [0u8; 6];
    for b in &mut magic {
        *b = reader.read_u8()?;
    }
    if &magic != b"vorbis" {
        return Err("Missing 'vorbis' magic in setup header".into());
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
            return Err(format!("Unsupported time domain transform: {}", t).into());
        }
    }

    // Floors
    let floor_count = reader.read_bits(6)? + 1;
    let mut floors = Vec::with_capacity(floor_count as usize);
    for _ in 0..floor_count {
        let floor_type = reader.read_bits(16)?;
        if floor_type != 1 {
            return Err(format!("Only floor type 1 is supported, got type {}", floor_type).into());
        }
        floors.push(decode_floor1_config(&mut reader)?);
    }

    // Residues
    let residue_count = reader.read_bits(6)? + 1;
    let mut residues = Vec::with_capacity(residue_count as usize);
    for _ in 0..residue_count {
        let residue_type = reader.read_bits(16)? as u16;
        if residue_type > 2 {
            return Err(format!("Unsupported residue type: {}", residue_type).into());
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
        return Err("Missing framing bit in setup header".into());
    }

    Ok(VorbisSetup {
        codebooks,
        floors,
        residues,
        mappings,
        modes,
    })
}
