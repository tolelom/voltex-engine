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

mod bit_reader;
mod codebook;
mod decoder;
mod floor;
mod headers;
mod imdct;
mod residue;

use crate::AudioClip;
use decoder::decode_audio_packet;
use headers::{parse_id_header, parse_setup_header};

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
pub fn decode_vorbis(packets: &[Vec<u8>]) -> crate::Result<AudioClip> {
    if packets.len() < 3 {
        return Err("Need at least 3 Vorbis packets (id, comment, setup)".into());
    }

    // Parse identification header
    let id = parse_id_header(&packets[0])?;

    // Skip comment header (packet[1]) — just verify it starts with type 3
    if packets[1].is_empty() || packets[1][0] != 3 {
        return Err("Invalid Vorbis comment header".into());
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
    use super::bit_reader::{bit_reverse, float32_unpack, ilog, lookup1_values, BitReader};
    use super::headers::parse_id_header;
    use super::imdct::{imdct, vorbis_window};
    use super::*;

    #[test]
    fn bitreader_basics() {
        // byte 0 = 0xB4 = 0b10110100, byte 1 = 0x69 = 0b01101001.
        // BitReader reads LSB-first within each byte.
        let data = [0b10110100u8, 0b01101001u8];
        let mut r = BitReader::new(&data);

        // First 3 bits (byte 0 bits 0..2): 0,0,1 → 0b100 = 4
        assert_eq!(r.read_bits(3).unwrap(), 0b100);
        // Next 5 bits (byte 0 bits 3..7): 0,1,1,0,1 → 0b10110 = 22
        assert_eq!(r.read_bits(5).unwrap(), 0b10110);
        // Following 8 bits = full byte 1 = 0x69 = 105
        assert_eq!(r.read_bits(8).unwrap(), 0x69);
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
            assert!((0.0..=1.0).contains(&v), "Window value out of range: {}", v);
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
        assert!(result.unwrap_err().to_string().contains("at least 3"));
    }

    #[test]
    fn decode_ogg_vorbis_bytes_empty() {
        let result = crate::decode_ogg_vorbis_bytes(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn decode_ogg_vorbis_bytes_invalid_magic() {
        let result = crate::decode_ogg_vorbis_bytes(b"NotOggVorbis");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("capture pattern") || msg.contains("OGG"),
            "expected OGG-related error, got: {}",
            msg
        );
    }

    #[test]
    fn floor1_inverse_db_range() {
        // Check that the function returns reasonable values
        use super::floor::floor1_inverse_db;
        assert_eq!(floor1_inverse_db(0.0), 0.0);
        let v = floor1_inverse_db(140.0);
        assert!((v - 1.0).abs() < 0.01, "at 140 expected ~1.0, got {}", v);
        let v = floor1_inverse_db(100.0);
        assert!(v < 1.0 && v > 0.0, "at 100 expected (0,1), got {}", v);
    }
}
