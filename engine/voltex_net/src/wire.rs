//! Little-endian readers for fixed offsets in a byte buffer. Callers check
//! the length first; reading past the end panics like slice indexing.

fn bytes_at<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes[offset..offset + N]);
    out
}

pub(crate) fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes_at(bytes, offset))
}

pub(crate) fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes_at(bytes, offset))
}

pub(crate) fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes_at(bytes, offset))
}

pub(crate) fn read_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(bytes_at(bytes, offset))
}
