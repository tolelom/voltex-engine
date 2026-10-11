//! Typed vertex-attribute reads for glTF accessors: `componentType`
//! (FLOAT or normalized/unnormalized (UNSIGNED_)BYTE/SHORT) and
//! `bufferView.byteStride`. Split from `accessor.rs` (600-line rule).

use super::accessor::{get_buffer_data, validate_count};
use crate::json_parser::JsonValue;

/// Size in bytes of one glTF component, or `None` for an unknown type.
fn component_size(comp_type: u32) -> Option<usize> {
    match comp_type {
        5120 | 5121 => Some(1), // BYTE / UNSIGNED_BYTE
        5122 | 5123 => Some(2), // SHORT / UNSIGNED_SHORT
        5126 => Some(4),        // FLOAT
        _ => None,
    }
}

/// One component as f32. Normalized integers map to [0, 1] / [-1, 1]
/// (signed: `max(c / MAX, -1)`, glTF 2.0 §3.11); others are cast.
fn read_component(b: &[u8], comp_type: u32, normalized: bool) -> f32 {
    match comp_type {
        5126 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        5121 => {
            let v = b[0] as f32;
            if normalized { v / 255.0 } else { v }
        }
        5120 => {
            let v = b[0] as i8 as f32;
            if normalized { (v / 127.0).max(-1.0) } else { v }
        }
        5123 => {
            let v = u16::from_le_bytes([b[0], b[1]]) as f32;
            if normalized { v / 65535.0 } else { v }
        }
        5122 => {
            let v = i16::from_le_bytes([b[0], b[1]]) as f32;
            if normalized { (v / 32767.0).max(-1.0) } else { v }
        }
        _ => unreachable!("component_size rejects other types"),
    }
}

/// Read an `N`-component accessor as f32 vectors, honouring `componentType`
/// (FLOAT or (UNSIGNED_)BYTE/SHORT, optionally `normalized` — e.g. quantized
/// TEXCOORD / WEIGHTS) and `bufferView.byteStride` (interleaved vertices).
pub(super) fn read_accessor_vecn<const N: usize>(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<[f32; N]>> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = validate_count(
        acc.get("count")
            .and_then(|v| v.as_u32())
            .ok_or("Missing count")?,
    )?;
    let comp_type = acc
        .get("componentType")
        .and_then(|v| v.as_u32())
        .unwrap_or(5126);
    let comp_size = component_size(comp_type)
        .ok_or_else(|| format!("Unsupported componentType {} for VEC{}", comp_type, N))?;
    let normalized = acc
        .get("normalized")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let elem = comp_size * N;
    let (buffer, offset, stride) = get_buffer_data(acc, buffer_views, buffers)?;
    let step = stride.unwrap_or(elem);
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * step;
        if o + elem > buffer.len() {
            return Err(format!("Buffer overflow reading VEC{}", N).into());
        }
        let mut v = [0.0f32; N];
        for (c, slot) in v.iter_mut().enumerate() {
            let at = o + c * comp_size;
            *slot = read_component(&buffer[at..at + comp_size], comp_type, normalized);
        }
        result.push(v);
    }
    Ok(result)
}
