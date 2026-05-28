use super::material::extract_material;
use super::node::{parse_animations, parse_nodes, parse_skins};
use super::types::{GltfData, GltfMesh};
use crate::json_parser::JsonValue;
use crate::obj::compute_tangents;
use crate::vertex::MeshVertex;

/// Hard cap on the `count` field of any glTF accessor. A glTF file with a
/// realistic mesh has at most a few hundred thousand vertices; this cap
/// catches adversarial JSON that would otherwise force a multi-GB
/// `Vec::with_capacity` allocation before any data is even read.
const MAX_ACCESSOR_COUNT: usize = 4_000_000;

/// Validates an accessor `count` field. Returns the cast `usize` value or
/// an error if it exceeds the cap.
fn validate_count(count: u32) -> crate::Result<usize> {
    let count = count as usize;
    if count > MAX_ACCESSOR_COUNT {
        return Err(format!(
            "glTF accessor count {} exceeds limit {}",
            count, MAX_ACCESSOR_COUNT
        )
        .into());
    }
    Ok(count)
}

pub(super) fn extract_meshes(json: &JsonValue, buffers: &[Vec<u8>]) -> crate::Result<GltfData> {
    let empty_arr: Vec<JsonValue> = Vec::new();
    let accessors = json
        .get("accessors")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_arr);
    let buffer_views = json
        .get("bufferViews")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_arr);
    let materials_json = json.get("materials").and_then(|v| v.as_array());

    let mut meshes = Vec::new();

    let mesh_list = json
        .get("meshes")
        .and_then(|v| v.as_array())
        .ok_or("No meshes in glTF")?;

    for mesh_val in mesh_list {
        let name = mesh_val
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let primitives = mesh_val
            .get("primitives")
            .and_then(|v| v.as_array())
            .ok_or("Mesh has no primitives")?;

        for prim in primitives {
            let attrs = prim
                .get("attributes")
                .and_then(|v| v.as_object())
                .ok_or("Primitive has no attributes")?;

            // Read position data (required)
            let pos_idx = attrs
                .iter()
                .find(|(k, _)| k == "POSITION")
                .and_then(|(_, v)| v.as_u32())
                .ok_or("Missing POSITION attribute")? as usize;
            let positions = read_accessor_vec3(accessors, buffer_views, buffers, pos_idx)?;

            // Read normals (optional)
            let normals = if let Some(idx) = attrs
                .iter()
                .find(|(k, _)| k == "NORMAL")
                .and_then(|(_, v)| v.as_u32())
            {
                read_accessor_vec3(accessors, buffer_views, buffers, idx as usize)?
            } else {
                vec![[0.0, 1.0, 0.0]; positions.len()]
            };

            // Read UVs (optional)
            let uvs = if let Some(idx) = attrs
                .iter()
                .find(|(k, _)| k == "TEXCOORD_0")
                .and_then(|(_, v)| v.as_u32())
            {
                read_accessor_vec2(accessors, buffer_views, buffers, idx as usize)?
            } else {
                vec![[0.0, 0.0]; positions.len()]
            };

            // Read tangents (optional)
            let tangents = if let Some(idx) = attrs
                .iter()
                .find(|(k, _)| k == "TANGENT")
                .and_then(|(_, v)| v.as_u32())
            {
                Some(read_accessor_vec4(
                    accessors,
                    buffer_views,
                    buffers,
                    idx as usize,
                )?)
            } else {
                None
            };

            // Read indices
            let indices = if let Some(idx) = prim.get("indices").and_then(|v| v.as_u32()) {
                read_accessor_indices(accessors, buffer_views, buffers, idx as usize)?
            } else {
                // No indices — generate sequential
                (0..positions.len() as u32).collect()
            };

            // Defend against malformed glTF where attribute accessors disagree
            // on count. Pre-fix, the loop below could panic with index-out-of-
            // bounds when reading `normals[i]` / `uvs[i]` / `tangents[i]`.
            if normals.len() != positions.len()
                || uvs.len() != positions.len()
                || tangents
                    .as_ref()
                    .is_some_and(|t| t.len() != positions.len())
            {
                return Err(format!(
                    "glTF primitive attribute length mismatch: positions={}, normals={}, uvs={}, tangents={:?}",
                    positions.len(),
                    normals.len(),
                    uvs.len(),
                    tangents.as_ref().map(|t| t.len()),
                ).into());
            }

            // Assemble vertices
            let mut vertices: Vec<MeshVertex> = Vec::with_capacity(positions.len());
            for i in 0..positions.len() {
                vertices.push(MeshVertex {
                    position: positions[i],
                    normal: normals[i],
                    uv: uvs[i],
                    tangent: tangents.as_ref().map_or([0.0; 4], |t| t[i]),
                });
            }

            // Read JOINTS_0 (optional)
            let joints = if let Some(idx) = attrs
                .iter()
                .find(|(k, _)| k == "JOINTS_0")
                .and_then(|(_, v)| v.as_u32())
            {
                Some(read_accessor_joints(
                    accessors,
                    buffer_views,
                    buffers,
                    idx as usize,
                )?)
            } else {
                None
            };

            // Read WEIGHTS_0 (optional)
            let weights = if let Some(idx) = attrs
                .iter()
                .find(|(k, _)| k == "WEIGHTS_0")
                .and_then(|(_, v)| v.as_u32())
            {
                Some(read_accessor_vec4(
                    accessors,
                    buffer_views,
                    buffers,
                    idx as usize,
                )?)
            } else {
                None
            };

            // Compute tangents if not provided
            if tangents.is_none() {
                compute_tangents(&mut vertices, &indices);
            }

            // Read material
            let material = prim
                .get("material")
                .and_then(|v| v.as_u32())
                .and_then(|idx| materials_json?.get(idx as usize))
                .and_then(extract_material);

            meshes.push(GltfMesh {
                vertices,
                indices,
                name: name.clone(),
                material,
                joints,
                weights,
            });
        }
    }

    let nodes = parse_nodes(json);
    let skins = parse_skins(json, accessors, buffer_views, buffers);
    let animations = parse_animations(json, accessors, buffer_views, buffers);

    Ok(GltfData {
        meshes,
        nodes,
        skins,
        animations,
    })
}

fn get_buffer_data<'a>(
    accessor: &JsonValue,
    buffer_views: &[JsonValue],
    buffers: &'a [Vec<u8>],
) -> crate::Result<(&'a [u8], usize)> {
    let bv_idx = accessor
        .get("bufferView")
        .and_then(|v| v.as_u32())
        .ok_or("Accessor missing bufferView")? as usize;
    let bv = buffer_views
        .get(bv_idx)
        .ok_or("BufferView index out of range")?;
    let buf_idx = bv.get("buffer").and_then(|v| v.as_u32()).unwrap_or(0) as usize;
    let bv_offset = bv.get("byteOffset").and_then(|v| v.as_u32()).unwrap_or(0) as usize;
    let acc_offset = accessor
        .get("byteOffset")
        .and_then(|v| v.as_u32())
        .unwrap_or(0) as usize;
    let buffer = buffers.get(buf_idx).ok_or("Buffer index out of range")?;
    let offset = bv_offset + acc_offset;
    Ok((buffer, offset))
}

fn read_accessor_vec3(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<[f32; 3]>> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = validate_count(
        acc.get("count")
            .and_then(|v| v.as_u32())
            .ok_or("Missing count")?,
    )?;
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * 12;
        if o + 12 > buffer.len() {
            return Err("Buffer overflow reading vec3".into());
        }
        let x = f32::from_le_bytes([buffer[o], buffer[o + 1], buffer[o + 2], buffer[o + 3]]);
        let y = f32::from_le_bytes([buffer[o + 4], buffer[o + 5], buffer[o + 6], buffer[o + 7]]);
        let z = f32::from_le_bytes([buffer[o + 8], buffer[o + 9], buffer[o + 10], buffer[o + 11]]);
        result.push([x, y, z]);
    }
    Ok(result)
}

fn read_accessor_vec2(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<[f32; 2]>> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = validate_count(
        acc.get("count")
            .and_then(|v| v.as_u32())
            .ok_or("Missing count")?,
    )?;
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * 8;
        if o + 8 > buffer.len() {
            return Err("Buffer overflow reading vec2".into());
        }
        let x = f32::from_le_bytes([buffer[o], buffer[o + 1], buffer[o + 2], buffer[o + 3]]);
        let y = f32::from_le_bytes([buffer[o + 4], buffer[o + 5], buffer[o + 6], buffer[o + 7]]);
        result.push([x, y]);
    }
    Ok(result)
}

fn read_accessor_vec4(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<[f32; 4]>> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = validate_count(
        acc.get("count")
            .and_then(|v| v.as_u32())
            .ok_or("Missing count")?,
    )?;
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * 16;
        if o + 16 > buffer.len() {
            return Err("Buffer overflow reading vec4".into());
        }
        let x = f32::from_le_bytes([buffer[o], buffer[o + 1], buffer[o + 2], buffer[o + 3]]);
        let y = f32::from_le_bytes([buffer[o + 4], buffer[o + 5], buffer[o + 6], buffer[o + 7]]);
        let z = f32::from_le_bytes([buffer[o + 8], buffer[o + 9], buffer[o + 10], buffer[o + 11]]);
        let w = f32::from_le_bytes([
            buffer[o + 12],
            buffer[o + 13],
            buffer[o + 14],
            buffer[o + 15],
        ]);
        result.push([x, y, z, w]);
    }
    Ok(result)
}

fn read_accessor_indices(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<u32>> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = validate_count(
        acc.get("count")
            .and_then(|v| v.as_u32())
            .ok_or("Missing count")?,
    )?;
    let comp_type = acc
        .get("componentType")
        .and_then(|v| v.as_u32())
        .ok_or("Missing componentType")?;
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;

    let mut result = Vec::with_capacity(count);
    match comp_type {
        5121 => {
            // UNSIGNED_BYTE
            for i in 0..count {
                if offset + i >= buffer.len() {
                    return Err("Buffer overflow reading u8 indices".into());
                }
                result.push(buffer[offset + i] as u32);
            }
        }
        5123 => {
            // UNSIGNED_SHORT
            for i in 0..count {
                let o = offset + i * 2;
                if o + 2 > buffer.len() {
                    return Err("Buffer overflow reading u16 indices".into());
                }
                result.push(u16::from_le_bytes([buffer[o], buffer[o + 1]]) as u32);
            }
        }
        5125 => {
            // UNSIGNED_INT
            for i in 0..count {
                let o = offset + i * 4;
                if o + 4 > buffer.len() {
                    return Err("Buffer overflow reading u32 indices".into());
                }
                result.push(u32::from_le_bytes([
                    buffer[o],
                    buffer[o + 1],
                    buffer[o + 2],
                    buffer[o + 3],
                ]));
            }
        }
        _ => return Err(format!("Unsupported index component type: {}", comp_type).into()),
    }
    Ok(result)
}

fn read_accessor_joints(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<[u16; 4]>> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = validate_count(
        acc.get("count")
            .and_then(|v| v.as_u32())
            .ok_or("Missing count")?,
    )?;
    let comp_type = acc
        .get("componentType")
        .and_then(|v| v.as_u32())
        .unwrap_or(5123);
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;
    let mut result = Vec::with_capacity(count);
    match comp_type {
        5121 => {
            // UNSIGNED_BYTE
            for i in 0..count {
                let o = offset + i * 4;
                if o + 4 > buffer.len() {
                    return Err("Buffer overflow reading joints u8".into());
                }
                result.push([
                    buffer[o] as u16,
                    buffer[o + 1] as u16,
                    buffer[o + 2] as u16,
                    buffer[o + 3] as u16,
                ]);
            }
        }
        5123 => {
            // UNSIGNED_SHORT
            for i in 0..count {
                let o = offset + i * 8;
                if o + 8 > buffer.len() {
                    return Err("Buffer overflow reading joints u16".into());
                }
                result.push([
                    u16::from_le_bytes([buffer[o], buffer[o + 1]]),
                    u16::from_le_bytes([buffer[o + 2], buffer[o + 3]]),
                    u16::from_le_bytes([buffer[o + 4], buffer[o + 5]]),
                    u16::from_le_bytes([buffer[o + 6], buffer[o + 7]]),
                ]);
            }
        }
        _ => return Err(format!("Unsupported joints component type: {}", comp_type).into()),
    }
    Ok(result)
}

pub(super) fn read_accessor_mat4(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<[[f32; 4]; 4]>> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = validate_count(
        acc.get("count")
            .and_then(|v| v.as_u32())
            .ok_or("Missing count")?,
    )?;
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * 64;
        if o + 64 > buffer.len() {
            return Err("Buffer overflow reading mat4".into());
        }
        let mut mat = [[0.0f32; 4]; 4];
        for (col, col_data) in mat.iter_mut().enumerate() {
            for (row, slot) in col_data.iter_mut().enumerate() {
                let b = o + (col * 4 + row) * 4;
                *slot =
                    f32::from_le_bytes([buffer[b], buffer[b + 1], buffer[b + 2], buffer[b + 3]]);
            }
        }
        result.push(mat);
    }
    Ok(result)
}

pub(super) fn read_accessor_floats(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<f32>> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = validate_count(
        acc.get("count")
            .and_then(|v| v.as_u32())
            .ok_or("Missing count")?,
    )?;
    let acc_type = acc.get("type").and_then(|v| v.as_str()).unwrap_or("SCALAR");
    let components = match acc_type {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => 1,
    };
    let total = count
        .checked_mul(components)
        .ok_or("glTF accessor count*components overflow")?;
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;
    let mut result = Vec::with_capacity(total);
    for i in 0..total {
        let o = offset + i * 4;
        if o + 4 > buffer.len() {
            return Err("Buffer overflow reading floats".into());
        }
        result.push(f32::from_le_bytes([
            buffer[o],
            buffer[o + 1],
            buffer[o + 2],
            buffer[o + 3],
        ]));
    }
    Ok(result)
}

// Helper functions for tests
#[allow(dead_code)]
pub(super) fn read_floats(buffer: &[u8], offset: usize, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            let o = offset + i * 4;
            f32::from_le_bytes([buffer[o], buffer[o + 1], buffer[o + 2], buffer[o + 3]])
        })
        .collect()
}

#[allow(dead_code)]
pub(super) fn read_indices_u16(buffer: &[u8], offset: usize, count: usize) -> Vec<u32> {
    (0..count)
        .map(|i| {
            let o = offset + i * 2;
            u16::from_le_bytes([buffer[o], buffer[o + 1]]) as u32
        })
        .collect()
}
