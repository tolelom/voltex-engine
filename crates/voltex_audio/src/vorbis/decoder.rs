// ---------------------------------------------------------------------------
// Audio packet decode (per-frame, post-headers)
// ---------------------------------------------------------------------------
//
// Vorbis spec § 4.3 uses cross-channel index loops (per-channel arrays
// addressed by the same index). Iterator translation hides the parallel
// addressing — keep index loops.
#![allow(clippy::needless_range_loop)]

use super::bit_reader::{ilog, BitReader};
use super::floor::decode_floor1_packet;
use super::headers::{VorbisIdHeader, VorbisSetup};
use super::imdct::{imdct, vorbis_window};
use super::residue::decode_residue;

pub(super) fn decode_audio_packet(
    packet: &[u8],
    id: &VorbisIdHeader,
    setup: &VorbisSetup,
    prev_window: &mut Vec<Vec<f32>>,
    prev_block_flag: &mut Option<bool>,
) -> crate::Result<Vec<f32>> {
    let mut reader = BitReader::new(packet);

    // Packet type must be 0 (audio)
    let ptype = reader.read_bit()?;
    if ptype {
        return Err("Expected audio packet (type 0)".into());
    }

    let mode_bits = ilog(setup.modes.len() as u32 - 1);
    let mode_number = reader.read_bits(mode_bits)? as usize;
    if mode_number >= setup.modes.len() {
        return Err(format!("Mode number {} out of range", mode_number).into());
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
            Err(_e) => {
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

    let overlap = if prev_n > 0 { prev_n.min(n) / 2 } else { 0 };

    let output_samples = if prev_window.is_empty() || prev_n == 0 {
        // First frame: no overlap, just store for next frame
        *prev_window = pcm_channels;
        *prev_block_flag = Some(block_flag);
        return Ok(Vec::new()); // No output for first frame
    } else {
        // Overlap-add
        let prev_right_start = prev_n / 2;

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

        frame_output
    };

    *prev_window = pcm_channels;
    *prev_block_flag = Some(block_flag);

    Ok(output_samples)
}
