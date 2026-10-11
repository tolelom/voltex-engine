//! End-to-end decode of a real `.ogg` file through the public entry point.
//!
//! Until now the Vorbis path was only covered by unit tests on <=100-byte
//! synthetic inputs, all of which asserted *rejection*. Nothing exercised
//! container framing -> header parsing -> codebook/floor/residue decode ->
//! IMDCT -> overlap-add, so a regression anywhere in that chain would have
//! gone unnoticed (`decode_vorbis` even swallows per-packet errors).
//!
//! The fixture below writes a genuine Ogg/Vorbis I bitstream byte by byte:
//! real page framing with real CRCs, a spec-shaped setup header (one flat
//! 256-entry codebook, floor type 1, residue type 2, two modes), and audio
//! packets carrying a chosen MDCT spectrum. It is deliberately built from the
//! Vorbis I spec layout rather than from what this decoder happens to read,
//! so a decoder that drifts away from the spec fails these tests.
//!
//! Scope: this proves the engine's own decoder reads a well-formed stream. It
//! does not prove compatibility with libvorbis-encoded content, which uses VQ
//! codebooks and channel coupling that this simplified decoder does not
//! implement. A file from a real encoder remains a manual/asset-level check.

use voltex_audio::{decode_ogg_vorbis_bytes, parse_ogg};

// ---------------------------------------------------------------------------
// Stream parameters
// ---------------------------------------------------------------------------

const SAMPLE_RATE: u32 = 8000;
const CHANNELS: u8 = 1;
const BLOCKSIZE_0: u32 = 256;
const BLOCKSIZE_1: u32 = 2048;
/// MDCT coefficients per short block.
const N_HALF: usize = BLOCKSIZE_0 as usize / 2;

/// One flat codebook: every entry has an 8-bit codeword, so entry `e` is
/// simply the 8-bit value `e` and no entry is a prefix of another.
const CODEBOOK_ENTRIES: u32 = 256;
const CODEWORD_BITS: u32 = 8;

const RESIDUE_PARTITION_SIZE: usize = 32;
/// `floor1_inverse_db(140) == 1.0`, i.e. a flat unity floor curve, which
/// leaves the decoded spectrum equal to the residue we wrote.
const FLOOR_Y: u32 = 140;
/// `1 << 7 == 128 == N_HALF`, so the floor's final x point lands on the end
/// of the spectrum.
const FLOOR_RANGEBITS: u32 = 7;

const SERIAL: u32 = 0x0056_4F4C;

// ---------------------------------------------------------------------------
// Bit writer (LSB-first within each byte, matching the Vorbis bitpacker)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    bit: u8,
}

impl BitWriter {
    fn write_bits(&mut self, value: u32, n: u32) {
        assert!(n <= 32, "write_bits: n > 32");
        for i in 0..n {
            if self.bit == 0 {
                self.bytes.push(0);
            }
            if (value >> i) & 1 == 1 {
                let last = self.bytes.last_mut().expect("byte pushed above");
                *last |= 1 << self.bit;
            }
            self.bit = (self.bit + 1) % 8;
        }
    }

    fn write_bit(&mut self, value: bool) {
        self.write_bits(value as u32, 1);
    }

    fn write_u8(&mut self, value: u8) {
        self.write_bits(value as u32, 8);
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u8(b);
        }
    }

    /// Emit the codeword for `entry` of the flat book. Vorbis writes codewords
    /// most-significant bit first while the bitpacker fills bytes LSB-first,
    /// so the 8-bit codeword for entry `e` reaches the decoder reversed.
    fn write_codeword(&mut self, entry: u32) {
        self.write_bits(reverse_bits(entry, CODEWORD_BITS), CODEWORD_BITS);
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn reverse_bits(value: u32, bits: u32) -> u32 {
    (0..bits).fold(0, |acc, i| (acc << 1) | ((value >> i) & 1))
}

// ---------------------------------------------------------------------------
// Vorbis packets
// ---------------------------------------------------------------------------

fn id_header_packet() -> Vec<u8> {
    let mut w = BitWriter::default();
    w.write_u8(1);
    w.write_bytes(b"vorbis");
    w.write_bits(0, 32); // version
    w.write_u8(CHANNELS);
    w.write_bits(SAMPLE_RATE, 32);
    w.write_bits(0, 32); // bitrate maximum
    w.write_bits(0, 32); // bitrate nominal
    w.write_bits(0, 32); // bitrate minimum
    w.write_u8((BLOCKSIZE_1.trailing_zeros() << 4) as u8 | BLOCKSIZE_0.trailing_zeros() as u8);
    w.write_bit(true); // framing
    w.finish()
}

fn comment_header_packet() -> Vec<u8> {
    let vendor = b"voltex_audio test fixture";
    let mut w = BitWriter::default();
    w.write_u8(3);
    w.write_bytes(b"vorbis");
    w.write_bits(vendor.len() as u32, 32);
    w.write_bytes(vendor);
    w.write_bits(0, 32); // user comment count
    w.write_bit(true); // framing
    w.finish()
}

fn setup_header_packet() -> Vec<u8> {
    let mut w = BitWriter::default();
    w.write_u8(5);
    w.write_bytes(b"vorbis");

    // Codebooks: a single flat scalar book, used as both the floor subclass
    // book and the residue classbook/value book.
    w.write_bits(0, 8); // count - 1
    w.write_bits(0x0056_4342, 24); // "BCV" sync pattern
    w.write_bits(1, 16); // dimensions
    w.write_bits(CODEBOOK_ENTRIES, 24);
    w.write_bit(false); // lengths are not ordered
    w.write_bit(false); // and not sparse
    for _ in 0..CODEBOOK_ENTRIES {
        w.write_bits(CODEWORD_BITS - 1, 5);
    }
    w.write_bits(0, 4); // lookup type 0 (no VQ)

    // Time domain transforms: one placeholder, always zero.
    w.write_bits(0, 6); // count - 1
    w.write_bits(0, 16);

    // Floor 1: one partition holding one class with one x point.
    w.write_bits(0, 6); // count - 1
    w.write_bits(1, 16); // floor type
    w.write_bits(1, 5); // partitions
    w.write_bits(0, 4); // partition_class[0]
    w.write_bits(0, 3); // class_dimensions[0] - 1
    w.write_bits(0, 2); // class_subclasses[0] == 0 -> no masterbook
    w.write_bits(1, 8); // subclass book 0 -> codebook 0 (stored as index + 1)
    w.write_bits(0, 2); // multiplier - 1
    w.write_bits(FLOOR_RANGEBITS, 4);
    w.write_bits(N_HALF as u32 / 2, FLOOR_RANGEBITS); // interior x point

    // Residue 2 (interleaved) covering the whole spectrum.
    w.write_bits(0, 6); // count - 1
    w.write_bits(2, 16); // residue type
    w.write_bits(0, 24); // begin
    w.write_bits(N_HALF as u32, 24); // end
    w.write_bits(RESIDUE_PARTITION_SIZE as u32 - 1, 24);
    w.write_bits(0, 6); // classifications - 1
    w.write_bits(0, 8); // classbook -> codebook 0
    w.write_bits(1, 3); // cascade[0]: pass 0 present
    w.write_bit(false); // no high cascade bits
    w.write_bits(0, 8); // books[0][0] -> codebook 0

    // Mapping 0: mono, a single submap over floor 0 / residue 0.
    w.write_bits(0, 6); // count - 1
    w.write_bits(0, 16); // mapping type
    w.write_bit(false); // submaps == 1
    w.write_bit(false); // no coupling steps
    w.write_bits(0, 2); // reserved
    w.write_bits(0, 8); // submap time configuration
    w.write_bits(0, 8); // submap floor
    w.write_bits(0, 8); // submap residue

    // Modes: short and long, so the mode number costs one bit like a real file.
    w.write_bits(1, 6); // count - 1
    for block_flag in [false, true] {
        w.write_bit(block_flag);
        w.write_bits(0, 16); // window type
        w.write_bits(0, 16); // transform type
        w.write_bits(0, 8); // mapping
    }

    w.write_bit(true); // framing
    w.finish()
}

/// A short-block audio packet whose decoded spectrum is `residue`
/// (the floor curve is flat unity, so the coefficients pass through).
fn audio_packet(residue: &[u32; N_HALF]) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.write_bit(false); // packet type 0 = audio
    w.write_bits(0, 1); // mode 0 (short block)

    // Floor 1: y values are read with ilog(range - 1) == 8 bits.
    w.write_bit(true); // channel carries audio this frame
    w.write_bits(FLOOR_Y, 8); // y at x = 0
    w.write_bits(FLOOR_Y, 8); // y at x = N_HALF
    w.write_codeword(FLOOR_Y); // y at the interior x point

    // Residue 2: one classification word per partition, then the values.
    for partition in 0..N_HALF / RESIDUE_PARTITION_SIZE {
        w.write_codeword(0); // single classification -> value is irrelevant
        let base = partition * RESIDUE_PARTITION_SIZE;
        for i in 0..RESIDUE_PARTITION_SIZE {
            w.write_codeword(residue[base + i]);
        }
    }

    w.finish()
}

// ---------------------------------------------------------------------------
// Ogg container
// ---------------------------------------------------------------------------

/// Ogg's CRC-32: polynomial 0x04c11db7, zero initial value, no reflection and
/// no final inversion (RFC 3533 section 6).
fn ogg_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;
    for &byte in data {
        crc ^= (byte as u32) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn ogg_page(header_type: u8, granule: u64, sequence: u32, packets: &[&[u8]]) -> Vec<u8> {
    let mut segments: Vec<u8> = Vec::new();
    let mut body: Vec<u8> = Vec::new();
    for packet in packets {
        segments.resize(segments.len() + packet.len() / 255, 255);
        segments.push((packet.len() % 255) as u8);
        body.extend_from_slice(packet);
    }

    let mut page = Vec::new();
    page.extend_from_slice(b"OggS");
    page.push(0); // stream structure version
    page.push(header_type);
    page.extend_from_slice(&granule.to_le_bytes());
    page.extend_from_slice(&SERIAL.to_le_bytes());
    page.extend_from_slice(&sequence.to_le_bytes());
    page.extend_from_slice(&[0u8; 4]); // CRC is computed over a zeroed field
    page.push(segments.len() as u8);
    page.extend_from_slice(&segments);
    page.extend_from_slice(&body);

    let crc = ogg_crc32(&page);
    page[22..26].copy_from_slice(&crc.to_le_bytes());
    page
}

/// Assemble a complete `.ogg` file: a beginning-of-stream page holding the
/// identification header, a page holding comment + setup, and an
/// end-of-stream page holding every audio packet.
fn ogg_vorbis_file(audio_packets: &[Vec<u8>]) -> Vec<u8> {
    let id = id_header_packet();
    let comment = comment_header_packet();
    let setup = setup_header_packet();
    let audio: Vec<&[u8]> = audio_packets.iter().map(Vec::as_slice).collect();

    // Each short block after the first advances the stream by N_HALF samples.
    let granule = (audio_packets.len().saturating_sub(1) * N_HALF) as u64;

    let mut file = ogg_page(0x02, 0, 0, &[&id]);
    file.extend_from_slice(&ogg_page(0x00, 0, 1, &[&comment, &setup]));
    file.extend_from_slice(&ogg_page(0x04, granule, 2, &audio));
    file
}

/// A spectrum with a single loud coefficient — one MDCT bin, which the inverse
/// transform turns into a windowed sinusoid rather than a DC offset.
fn tone_spectrum() -> [u32; N_HALF] {
    let mut residue = [0u32; N_HALF];
    // The IMDCT scales by 2/n, so 128 in one bin lands near unit amplitude.
    residue[4] = 128;
    residue
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn generated_file_splits_into_three_header_packets_and_the_audio_packets() {
    let file = ogg_vorbis_file(&[audio_packet(&tone_spectrum())]);
    let packets = parse_ogg(&file).expect("container parse failed");

    assert_eq!(packets.len(), 4, "3 headers + 1 audio packet");
    assert_eq!(packets[0][0], 1, "identification header");
    assert_eq!(packets[1][0], 3, "comment header");
    assert_eq!(packets[2][0], 5, "setup header");
    assert_eq!(packets[0], id_header_packet());
    assert_eq!(packets[2], setup_header_packet());
}

#[test]
fn tone_ogg_decodes_to_the_expected_waveform() {
    let packet = audio_packet(&tone_spectrum());
    let file = ogg_vorbis_file(&[packet.clone(), packet.clone(), packet]);

    let clip = decode_ogg_vorbis_bytes(&file).expect("decode failed");

    assert_eq!(clip.sample_rate, SAMPLE_RATE);
    assert_eq!(clip.channels, CHANNELS as u16);
    // The first block only primes the overlap buffer; each later block emits
    // one half-block of overlap-added samples.
    assert_eq!(clip.samples.len(), 2 * N_HALF);

    assert!(
        clip.samples.iter().all(|s| s.is_finite()),
        "decoder produced a non-finite sample"
    );

    let peak = clip.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(
        (0.5..4.0).contains(&peak),
        "expected roughly unit-amplitude output, got peak {peak}"
    );

    // A single MDCT bin must come back as an oscillation, not a constant
    // offset: a floor/residue/IMDCT mix-up that produced DC would pass a
    // bare "not silent" check.
    assert!(
        clip.samples.iter().any(|&s| s > 0.0) && clip.samples.iter().any(|&s| s < 0.0),
        "expected the tone to swing through zero"
    );

    // Identical packets over a steady overlap state must decode identically.
    assert_eq!(clip.samples[..N_HALF], clip.samples[N_HALF..]);
}

#[test]
fn ogg_whose_audio_packets_all_fail_is_an_error() {
    // A leading 1 bit marks the packet as a header, which no audio frame may
    // be. Before, a stream like this decoded to `Ok` with zero samples, so a
    // corrupt file was indistinguishable from a deliberately silent one.
    let broken = vec![vec![0xFF_u8; 8], vec![0xFF_u8; 8]];
    let file = ogg_vorbis_file(&broken);

    let err = decode_ogg_vorbis_bytes(&file).expect_err("expected a decode error");
    let msg = err.to_string();
    assert!(
        msg.contains("all 2 audio packet(s)"),
        "error should name the all-failed case, got: {msg}"
    );
}

#[test]
fn ogg_with_one_broken_audio_packet_keeps_the_frames_that_decoded() {
    // Partial damage must stay survivable: a truncated tail costs its own
    // frame, not the whole clip.
    let packet = audio_packet(&tone_spectrum());
    let file = ogg_vorbis_file(&[packet.clone(), packet, vec![0xFF_u8; 8]]);

    let clip = decode_ogg_vorbis_bytes(&file).expect("partial damage must still decode");
    assert_eq!(clip.samples.len(), N_HALF, "one good overlap-added frame");
}

#[test]
fn zero_residue_ogg_decodes_to_exact_silence() {
    // Control for the test above: same stream, same floor, empty spectrum.
    // If that test's signal came from anywhere other than the residue we
    // wrote, this one would not be silent.
    let packet = audio_packet(&[0u32; N_HALF]);
    let file = ogg_vorbis_file(&[packet.clone(), packet]);

    let clip = decode_ogg_vorbis_bytes(&file).expect("decode failed");

    assert_eq!(clip.samples.len(), N_HALF);
    assert!(
        clip.samples.iter().all(|&s| s == 0.0),
        "expected exact silence from an all-zero spectrum"
    );
}

/// Mutation fuzz: corrupt the setup header (codebooks, floors, residues,
/// mappings, modes) or an audio packet, then re-wrap in valid Ogg pages so
/// the CRC passes and the bytes actually reach the Vorbis decoder. Also
/// corrupt finished files directly to exercise the container parser.
#[test]
fn decoder_never_panics_on_corrupted_streams() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let mut state = 0x0DD5_EED5u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let audio = audio_packet(&tone_spectrum());
    let clean = ogg_vorbis_file(&[audio.clone(), audio.clone(), audio.clone()]);
    for i in 0..3000 {
        let file = match next() % 3 {
            0 => {
                let mut setup = setup_header_packet();
                for _ in 0..1 + next() % 4 {
                    let at = 7 + next() as usize % (setup.len() - 7); // keep the "\x05vorbis" tag
                    setup[at] ^= 1 << (next() % 8);
                }
                let mut f = ogg_page(0x02, 0, 0, &[&id_header_packet()]);
                f.extend_from_slice(&ogg_page(0x00, 0, 1, &[&comment_header_packet(), &setup]));
                f.extend_from_slice(&ogg_page(0x04, 2 * N_HALF as u64, 2, &[&audio, &audio, &audio]));
                f
            }
            1 => {
                let mut bad = audio.clone();
                for _ in 0..1 + next() % 4 {
                    let at = next() as usize % bad.len();
                    bad[at] = next() as u8;
                }
                bad.truncate(1 + next() as usize % bad.len());
                ogg_vorbis_file(&[audio.clone(), bad, audio.clone()])
            }
            _ => {
                let mut f = clean.clone();
                for _ in 0..1 + next() % 6 {
                    let at = next() as usize % f.len();
                    f[at] = next() as u8;
                }
                f.truncate(next() as usize % (f.len() + 1));
                f
            }
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(clip) = decode_ogg_vorbis_bytes(&file) {
                assert!(clip.samples.iter().all(|s| s.is_finite()), "non-finite sample");
            }
        }));
        assert!(outcome.is_ok(), "mutation {i} panicked the Ogg/Vorbis decoder");
    }
}
