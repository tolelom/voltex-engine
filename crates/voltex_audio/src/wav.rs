use crate::AudioClip;

// ---------------------------------------------------------------------------
// Helper readers
// ---------------------------------------------------------------------------

fn read_u16_le(data: &[u8], offset: usize) -> crate::Result<u16> {
    if offset + 2 > data.len() {
        return Err(format!("read_u16_le: offset {} out of bounds (len={})", offset, data.len()).into());
    }
    Ok(u16::from_le_bytes([data[offset], data[offset + 1]]))
}

fn read_u32_le(data: &[u8], offset: usize) -> crate::Result<u32> {
    if offset + 4 > data.len() {
        return Err(format!("read_u32_le: offset {} out of bounds (len={})", offset, data.len()).into());
    }
    Ok(u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

fn read_i16_le(data: &[u8], offset: usize) -> crate::Result<i16> {
    if offset + 2 > data.len() {
        return Err(format!("read_i16_le: offset {} out of bounds (len={})", offset, data.len()).into());
    }
    Ok(i16::from_le_bytes([data[offset], data[offset + 1]]))
}

/// Search for a four-byte chunk ID starting after `start` and return its
/// (data_offset, data_size) pair, where data_offset is the first byte of the
/// chunk's payload.
fn find_chunk(data: &[u8], id: &[u8; 4], start: usize) -> Option<(usize, u32)> {
    let mut pos = start;
    while pos + 8 <= data.len() {
        if &data[pos..pos + 4] == id {
            let size = u32::from_le_bytes([
                data[pos + 4],
                data[pos + 5],
                data[pos + 6],
                data[pos + 7],
            ]);
            return Some((pos + 8, size));
        }
        // Skip this chunk: header (8 bytes) + size, padded to even
        let size = u32::from_le_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;
        let padded = size + (size & 1); // RIFF chunks are word-aligned
        pos += 8 + padded;
    }
    None
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Read a 24-bit signed integer (little-endian) from 3 bytes and return as i32.
fn read_i24_le(data: &[u8], offset: usize) -> crate::Result<i32> {
    if offset + 3 > data.len() {
        return Err(format!("read_i24_le: offset {} out of bounds (len={})", offset, data.len()).into());
    }
    let lo = data[offset] as u32;
    let mid = data[offset + 1] as u32;
    let hi = data[offset + 2] as u32;
    let unsigned = lo | (mid << 8) | (hi << 16);
    // Sign-extend from 24-bit to 32-bit
    if unsigned & 0x800000 != 0 {
        Ok((unsigned | 0xFF000000) as i32)
    } else {
        Ok(unsigned as i32)
    }
}

/// Read a 32-bit float (little-endian).
fn read_f32_le(data: &[u8], offset: usize) -> crate::Result<f32> {
    if offset + 4 > data.len() {
        return Err(format!("read_f32_le: offset {} out of bounds (len={})", offset, data.len()).into());
    }
    Ok(f32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

/// Parse a WAV file from raw bytes into an [`AudioClip`].
///
/// Supported formats:
/// - PCM 16-bit (format_tag=1, bits_per_sample=16)
/// - PCM 24-bit (format_tag=1, bits_per_sample=24)
/// - IEEE float 32-bit (format_tag=3, bits_per_sample=32)
pub fn parse_wav(data: &[u8]) -> crate::Result<AudioClip> {
    // Minimum viable WAV: RIFF(4) + size(4) + WAVE(4) = 12 bytes
    if data.len() < 12 {
        return Err("WAV data too short".into());
    }

    // RIFF header
    if &data[0..4] != b"RIFF" {
        return Err("Missing RIFF header".into());
    }
    if &data[8..12] != b"WAVE" {
        return Err("Missing WAVE format identifier".into());
    }

    // --- fmt chunk (search from byte 12) ---
    let (fmt_offset, fmt_size) =
        find_chunk(data, b"fmt ", 12).ok_or_else(|| crate::AudioError::from("Missing fmt chunk"))?;

    if fmt_size < 16 {
        return Err(format!("fmt chunk too small: {}", fmt_size).into());
    }

    let format_tag = read_u16_le(data, fmt_offset)?;
    let channels = read_u16_le(data, fmt_offset + 2)?;
    if channels != 1 && channels != 2 {
        return Err(format!("Unsupported channel count: {}", channels).into());
    }

    let sample_rate = read_u32_le(data, fmt_offset + 4)?;
    // byte_rate = fmt_offset + 8  (skip)
    // block_align = fmt_offset + 12 (skip)
    let bits_per_sample = read_u16_le(data, fmt_offset + 14)?;

    // Validate format_tag + bits_per_sample combination
    let bytes_per_sample = match (format_tag, bits_per_sample) {
        (1, 16) => 2,   // PCM 16-bit
        (1, 24) => 3,   // PCM 24-bit
        (3, 32) => 4,   // IEEE float 32-bit
        (1, bps) => return Err(format!("Unsupported PCM bits per sample: {}", bps).into()),
        (3, bps) => return Err(format!("Unsupported float bits per sample: {} (only 32-bit supported)", bps).into()),
        (tag, _) => return Err(format!("Unsupported WAV format tag: {} (only PCM=1 and IEEE_FLOAT=3 are supported)", tag).into()),
    };

    // --- data chunk ---
    let (data_offset, data_size) =
        find_chunk(data, b"data", 12).ok_or_else(|| crate::AudioError::from("Missing data chunk"))?;

    let data_end = data_offset + data_size as usize;
    if data_end > data.len() {
        return Err("data chunk extends beyond end of file".into());
    }

    let sample_count = data_size as usize / bytes_per_sample;
    let mut samples = Vec::with_capacity(sample_count);

    match (format_tag, bits_per_sample) {
        (1, 16) => {
            for i in 0..sample_count {
                let raw = read_i16_le(data, data_offset + i * 2)?;
                samples.push(raw as f32 / 32768.0);
            }
        }
        (1, 24) => {
            for i in 0..sample_count {
                let raw = read_i24_le(data, data_offset + i * 3)?;
                // 24-bit range: [-8388608, 8388607]
                samples.push(raw as f32 / 8388608.0);
            }
        }
        (3, 32) => {
            for i in 0..sample_count {
                let raw = read_f32_le(data, data_offset + i * 4)?;
                samples.push(raw);
            }
        }
        _ => unreachable!(),
    }

    Ok(AudioClip::new(samples, sample_rate, channels))
}

/// Generate a minimal PCM 16-bit mono WAV file from f32 samples.
/// Used for round-trip testing.
pub fn generate_wav_bytes(samples_f32: &[f32], sample_rate: u32) -> Vec<u8> {
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align: u16 = channels * bits_per_sample / 8;
    let data_size = (samples_f32.len() * 2) as u32; // 2 bytes per i16 sample
    let riff_size = 4 + 8 + 16 + 8 + data_size; // "WAVE" + fmt chunk + data chunk

    let mut out: Vec<u8> = Vec::with_capacity(12 + 8 + 16 + 8 + data_size as usize);

    // RIFF header
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    // fmt chunk
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    out.extend_from_slice(&1u16.to_le_bytes());  // PCM format tag
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());

    for &s in samples_f32 {
        let clamped = s.clamp(-1.0, 1.0);
        let raw = (clamped * 32767.0) as i16;
        out.extend_from_slice(&raw.to_le_bytes());
    }

    out
}

/// Generate a minimal PCM 24-bit mono WAV file from f32 samples.
/// Used for round-trip testing.
pub fn generate_wav_bytes_24bit(samples_f32: &[f32], sample_rate: u32) -> Vec<u8> {
    let channels: u16 = 1;
    let bits_per_sample: u16 = 24;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align: u16 = channels * bits_per_sample / 8;
    let data_size = (samples_f32.len() * 3) as u32;
    let riff_size = 4 + 8 + 16 + 8 + data_size;

    let mut out: Vec<u8> = Vec::with_capacity(12 + 8 + 16 + 8 + data_size as usize);

    // RIFF header
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    // fmt chunk
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());

    for &s in samples_f32 {
        let clamped = s.clamp(-1.0, 1.0);
        let raw = (clamped * 8388607.0) as i32;
        // Write 3 bytes LE
        out.push((raw & 0xFF) as u8);
        out.push(((raw >> 8) & 0xFF) as u8);
        out.push(((raw >> 16) & 0xFF) as u8);
    }

    out
}

/// Generate a minimal IEEE float 32-bit mono WAV file from f32 samples.
/// Used for round-trip testing.
pub fn generate_wav_bytes_f32(samples_f32: &[f32], sample_rate: u32) -> Vec<u8> {
    let channels: u16 = 1;
    let bits_per_sample: u16 = 32;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align: u16 = channels * bits_per_sample / 8;
    let data_size = (samples_f32.len() * 4) as u32;
    let riff_size = 4 + 8 + 16 + 8 + data_size;

    let mut out: Vec<u8> = Vec::with_capacity(12 + 8 + 16 + 8 + data_size as usize);

    // RIFF header
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    // fmt chunk
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes()); // IEEE_FLOAT
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());

    for &s in samples_f32 {
        out.extend_from_slice(&s.to_le_bytes());
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_wav() {
        // Generate a 440 Hz sine wave at 44100 Hz, 0.1 s
        let sample_rate = 44100u32;
        let num_samples = 4410usize;
        let samples: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin())
            .collect();

        let wav_bytes = generate_wav_bytes(&samples, sample_rate);
        let clip = parse_wav(&wav_bytes).expect("parse_wav failed");

        assert_eq!(clip.sample_rate, sample_rate);
        assert_eq!(clip.channels, 1);
        assert_eq!(clip.frame_count(), num_samples);
    }

    #[test]
    fn sample_conversion_accuracy() {
        // A single-sample WAV: value = 16384 (half of i16 max positive)
        // Expected f32: 16384 / 32768 = 0.5
        let sample_rate = 44100u32;
        let samples_f32 = vec![0.5f32];
        let wav_bytes = generate_wav_bytes(&samples_f32, sample_rate);
        let clip = parse_wav(&wav_bytes).expect("parse_wav failed");

        assert_eq!(clip.samples.len(), 1);
        // 0.5 -> i16(16383) -> f32(16383/32768) ≈ 0.49997
        assert!((clip.samples[0] - 0.5f32).abs() < 0.001, "got {}", clip.samples[0]);
    }

    #[test]
    fn invalid_riff() {
        let bad_data = b"BADH\x00\x00\x00\x00WAVE";
        let result = parse_wav(bad_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("RIFF"));
    }

    #[test]
    fn too_short() {
        let short_data = b"RIF";
        let result = parse_wav(short_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too short"));
    }

    #[test]
    fn roundtrip() {
        let original: Vec<f32> = vec![0.0, 0.25, 0.5, -0.25, -0.5, 1.0, -1.0];
        let wav_bytes = generate_wav_bytes(&original, 44100);
        let clip = parse_wav(&wav_bytes).expect("roundtrip parse failed");

        assert_eq!(clip.samples.len(), original.len());
        for (orig, decoded) in original.iter().zip(clip.samples.iter()) {
            // i16 quantization error < 0.001
            assert!(
                (orig - decoded).abs() < 0.001,
                "orig={} decoded={}",
                orig,
                decoded
            );
        }
    }

    // -----------------------------------------------------------------------
    // 24-bit PCM tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_24bit_wav() {
        let sample_rate = 44100u32;
        let num_samples = 4410usize;
        let samples: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin())
            .collect();

        let wav_bytes = generate_wav_bytes_24bit(&samples, sample_rate);
        let clip = parse_wav(&wav_bytes).expect("parse_wav 24-bit failed");

        assert_eq!(clip.sample_rate, sample_rate);
        assert_eq!(clip.channels, 1);
        assert_eq!(clip.frame_count(), num_samples);
    }

    #[test]
    fn roundtrip_24bit() {
        let original: Vec<f32> = vec![0.0, 0.25, 0.5, -0.25, -0.5, 1.0, -1.0];
        let wav_bytes = generate_wav_bytes_24bit(&original, 44100);
        let clip = parse_wav(&wav_bytes).expect("roundtrip 24-bit parse failed");

        assert_eq!(clip.samples.len(), original.len());
        for (orig, decoded) in original.iter().zip(clip.samples.iter()) {
            // 24-bit quantization error should be < 0.0001
            assert!(
                (orig - decoded).abs() < 0.0001,
                "24-bit: orig={} decoded={}",
                orig,
                decoded
            );
        }
    }

    #[test]
    fn accuracy_24bit() {
        // 24-bit should be more accurate than 16-bit
        let samples = vec![0.5f32];
        let wav_bytes = generate_wav_bytes_24bit(&samples, 44100);
        let clip = parse_wav(&wav_bytes).expect("parse failed");
        // 0.5 * 8388607 = 4194303 -> 4194303 / 8388608 ≈ 0.49999988
        assert!((clip.samples[0] - 0.5).abs() < 0.0001, "24-bit got {}", clip.samples[0]);
    }

    // -----------------------------------------------------------------------
    // 32-bit float tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_32bit_float_wav() {
        let sample_rate = 44100u32;
        let num_samples = 4410usize;
        let samples: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin())
            .collect();

        let wav_bytes = generate_wav_bytes_f32(&samples, sample_rate);
        let clip = parse_wav(&wav_bytes).expect("parse_wav float32 failed");

        assert_eq!(clip.sample_rate, sample_rate);
        assert_eq!(clip.channels, 1);
        assert_eq!(clip.frame_count(), num_samples);
    }

    #[test]
    fn roundtrip_32bit_float() {
        let original: Vec<f32> = vec![0.0, 0.25, 0.5, -0.25, -0.5, 1.0, -1.0];
        let wav_bytes = generate_wav_bytes_f32(&original, 44100);
        let clip = parse_wav(&wav_bytes).expect("roundtrip float32 parse failed");

        assert_eq!(clip.samples.len(), original.len());
        for (orig, decoded) in original.iter().zip(clip.samples.iter()) {
            // 32-bit float should be exact
            assert_eq!(*orig, *decoded, "float32: orig={} decoded={}", orig, decoded);
        }
    }

    #[test]
    fn accuracy_32bit_float() {
        // 32-bit float should preserve exact values
        let samples = vec![0.123_456_79_f32, -0.987_654_3_f32];
        let wav_bytes = generate_wav_bytes_f32(&samples, 44100);
        let clip = parse_wav(&wav_bytes).expect("parse failed");
        assert_eq!(clip.samples[0], 0.123_456_79_f32);
        assert_eq!(clip.samples[1], -0.987_654_3_f32);
    }

    #[test]
    fn reject_unsupported_format_tag() {
        // Create a WAV with format_tag=2 (ADPCM), which we don't support
        let mut wav = generate_wav_bytes(&[0.0], 44100);
        // format_tag is at byte 20-21 (RIFF(4)+size(4)+WAVE(4)+fmt(4)+chunk_size(4))
        wav[20] = 2;
        wav[21] = 0;
        let result = parse_wav(&wav);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported WAV format tag"));
    }
}
