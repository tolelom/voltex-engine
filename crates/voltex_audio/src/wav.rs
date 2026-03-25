use crate::AudioClip;

// ---------------------------------------------------------------------------
// Helper readers
// ---------------------------------------------------------------------------

fn read_u16_le(data: &[u8], offset: usize) -> Result<u16, String> {
    if offset + 2 > data.len() {
        return Err(format!("read_u16_le: offset {} out of bounds (len={})", offset, data.len()));
    }
    Ok(u16::from_le_bytes([data[offset], data[offset + 1]]))
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, String> {
    if offset + 4 > data.len() {
        return Err(format!("read_u32_le: offset {} out of bounds (len={})", offset, data.len()));
    }
    Ok(u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

fn read_i16_le(data: &[u8], offset: usize) -> Result<i16, String> {
    if offset + 2 > data.len() {
        return Err(format!("read_i16_le: offset {} out of bounds (len={})", offset, data.len()));
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

/// Parse a PCM 16-bit WAV file from raw bytes into an [`AudioClip`].
pub fn parse_wav(data: &[u8]) -> Result<AudioClip, String> {
    // Minimum viable WAV: RIFF(4) + size(4) + WAVE(4) = 12 bytes
    if data.len() < 12 {
        return Err("WAV data too short".to_string());
    }

    // RIFF header
    if &data[0..4] != b"RIFF" {
        return Err("Missing RIFF header".to_string());
    }
    if &data[8..12] != b"WAVE" {
        return Err("Missing WAVE format identifier".to_string());
    }

    // --- fmt chunk (search from byte 12) ---
    let (fmt_offset, fmt_size) =
        find_chunk(data, b"fmt ", 12).ok_or("Missing fmt chunk")?;

    if fmt_size < 16 {
        return Err(format!("fmt chunk too small: {}", fmt_size));
    }

    let format_tag = read_u16_le(data, fmt_offset)?;
    if format_tag != 1 {
        return Err(format!("Unsupported WAV format tag: {} (only PCM=1 is supported)", format_tag));
    }

    let channels = read_u16_le(data, fmt_offset + 2)?;
    if channels != 1 && channels != 2 {
        return Err(format!("Unsupported channel count: {}", channels));
    }

    let sample_rate = read_u32_le(data, fmt_offset + 4)?;
    // byte_rate = fmt_offset + 8  (skip)
    // block_align = fmt_offset + 12 (skip)
    let bits_per_sample = read_u16_le(data, fmt_offset + 14)?;
    if bits_per_sample != 16 {
        return Err(format!("Unsupported bits per sample: {} (only 16-bit is supported)", bits_per_sample));
    }

    // --- data chunk ---
    let (data_offset, data_size) =
        find_chunk(data, b"data", 12).ok_or("Missing data chunk")?;

    let data_end = data_offset + data_size as usize;
    if data_end > data.len() {
        return Err("data chunk extends beyond end of file".to_string());
    }

    // Each sample is 2 bytes (16-bit PCM).
    let sample_count = data_size as usize / 2;
    let mut samples = Vec::with_capacity(sample_count);

    for i in 0..sample_count {
        let raw = read_i16_le(data, data_offset + i * 2)?;
        // Convert i16 [-32768, 32767] to f32 [-1.0, ~1.0]
        samples.push(raw as f32 / 32768.0);
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
        assert!(result.unwrap_err().contains("RIFF"));
    }

    #[test]
    fn too_short() {
        let short_data = b"RIF";
        let result = parse_wav(short_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too short"));
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
}
