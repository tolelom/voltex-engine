use super::accessor::extract_meshes;
use super::types::GltfData;
use crate::json_parser;

const GLB_MAGIC: u32 = 0x46546C67;
const GLB_VERSION: u32 = 2;
const CHUNK_JSON: u32 = 0x4E4F534A;
const CHUNK_BIN: u32 = 0x004E4942;

pub fn parse_gltf(data: &[u8]) -> crate::Result<GltfData> {
    if data.len() < 4 {
        return Err("Data too short".into());
    }

    // Detect format: GLB (binary) or JSON
    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic == GLB_MAGIC {
        parse_glb(data)
    } else if data[0] == b'{' {
        parse_gltf_json(data)
    } else {
        Err("Unknown glTF format: not GLB or JSON".into())
    }
}

fn parse_glb(data: &[u8]) -> crate::Result<GltfData> {
    if data.len() < 12 {
        return Err("GLB header too short".into());
    }
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version != GLB_VERSION {
        return Err(format!("Unsupported GLB version: {} (expected 2)", version).into());
    }
    let _total_len = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;

    // Parse chunks
    let mut pos = 12;
    let mut json_str = String::new();
    let mut bin_data: Vec<u8> = Vec::new();

    while pos + 8 <= data.len() {
        let chunk_len =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
        pos += 8;

        if pos + chunk_len > data.len() {
            return Err("Chunk extends past data".into());
        }

        match chunk_type {
            CHUNK_JSON => {
                json_str = std::str::from_utf8(&data[pos..pos + chunk_len])
                    .map_err(|_| crate::ImageError::from("Invalid UTF-8 in JSON chunk"))?
                    .to_string();
            }
            CHUNK_BIN => {
                bin_data = data[pos..pos + chunk_len].to_vec();
            }
            _ => {} // skip unknown chunks
        }
        pos += chunk_len;
        // Chunks are 4-byte aligned
        pos = (pos + 3) & !3;
    }

    if json_str.is_empty() {
        return Err("No JSON chunk found in GLB".into());
    }

    let json = json_parser::parse_json(&json_str)?;
    let buffers = vec![bin_data]; // GLB has one implicit binary buffer
    extract_meshes(&json, &buffers)
}

fn parse_gltf_json(data: &[u8]) -> crate::Result<GltfData> {
    let json_str =
        std::str::from_utf8(data).map_err(|_| crate::ImageError::from("Invalid UTF-8"))?;
    let json = json_parser::parse_json(json_str)?;

    // Resolve buffers (embedded base64 URIs)
    let mut buffers = Vec::new();
    if let Some(bufs) = json.get("buffers").and_then(|v| v.as_array()) {
        for buf in bufs {
            if let Some(uri) = buf.get("uri").and_then(|v| v.as_str()) {
                if let Some(b64) = uri.strip_prefix("data:application/octet-stream;base64,") {
                    buffers.push(decode_base64(b64)?);
                } else if let Some(b64) = uri.strip_prefix("data:application/gltf-buffer;base64,") {
                    buffers.push(decode_base64(b64)?);
                } else {
                    return Err(format!("External buffer URIs not supported: {}", uri).into());
                }
            } else {
                buffers.push(Vec::new());
            }
        }
    }

    extract_meshes(&json, &buffers)
}

pub(super) fn decode_base64(input: &str) -> crate::Result<Vec<u8>> {
    let table = |c: u8| -> crate::Result<u8> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0), // padding
            _ => Err(format!("Invalid base64 character: {}", c as char).into()),
        }
    };

    let bytes: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'\n' && b != b'\r' && b != b' ')
        .collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);

    for chunk in bytes.chunks(4) {
        let b0 = table(chunk[0])?;
        let b1 = if chunk.len() > 1 { table(chunk[1])? } else { 0 };
        let b2 = if chunk.len() > 2 { table(chunk[2])? } else { 0 };
        let b3 = if chunk.len() > 3 { table(chunk[3])? } else { 0 };

        out.push((b0 << 2) | (b1 >> 4));
        if chunk.len() > 2 && chunk[2] != b'=' {
            out.push((b1 << 4) | (b2 >> 2));
        }
        if chunk.len() > 3 && chunk[3] != b'=' {
            out.push((b2 << 6) | b3);
        }
    }
    Ok(out)
}
