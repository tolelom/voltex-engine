use super::material::extract_material;
use super::node::{parse_animations, parse_nodes, parse_skins};
use super::types::{GltfData, GltfMesh};
use super::component::{read_accessor_vecn, UNSIGNED_BYTE, UNSIGNED_INT, UNSIGNED_SHORT};
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
pub(super) fn validate_count(count: u32) -> crate::Result<usize> {
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
            let mesh = read_primitive(prim, accessors, buffer_views, buffers, materials_json)?;
            meshes.push(GltfMesh {
                name: name.clone(),
                ..mesh
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

/// Accessor index of a primitive attribute (`"POSITION"`, `"NORMAL"`, …).
fn attribute_accessor(attrs: &[(String, JsonValue)], name: &str) -> Option<usize> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .and_then(|(_, v)| v.as_u32())
        .map(|idx| idx as usize)
}

/// One `mesh.primitives[i]` → vertices (tangents generated when absent),
/// indices, optional skin attributes and material. `name` is left `None`.
fn read_primitive(
    prim: &JsonValue,
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    materials_json: Option<&[JsonValue]>,
) -> crate::Result<GltfMesh> {
    let attrs = prim
        .get("attributes")
        .and_then(|v| v.as_object())
        .ok_or("Primitive has no attributes")?;

    let pos_idx = attribute_accessor(attrs, "POSITION").ok_or("Missing POSITION attribute")?;
    let positions = read_accessor_vec3(accessors, buffer_views, buffers, pos_idx)?;

    // Optional attributes get neutral defaults (up normal, zero UV).
    let normals = match attribute_accessor(attrs, "NORMAL") {
        Some(idx) => read_accessor_vec3(accessors, buffer_views, buffers, idx)?,
        None => vec![[0.0, 1.0, 0.0]; positions.len()],
    };
    let uvs = match attribute_accessor(attrs, "TEXCOORD_0") {
        Some(idx) => read_accessor_vec2(accessors, buffer_views, buffers, idx)?,
        None => vec![[0.0, 0.0]; positions.len()],
    };
    let tangents = match attribute_accessor(attrs, "TANGENT") {
        Some(idx) => Some(read_accessor_vec4(accessors, buffer_views, buffers, idx)?),
        None => None,
    };

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

    let mut vertices: Vec<MeshVertex> = Vec::with_capacity(positions.len());
    for i in 0..positions.len() {
        vertices.push(MeshVertex {
            position: positions[i],
            normal: normals[i],
            uv: uvs[i],
            tangent: tangents.as_ref().map_or([0.0; 4], |t| t[i]),
        });
    }

    // Indices come from the file; one past the vertex list would index
    // out of bounds in tangent generation and on the GPU.
    if let Some(&bad) = indices.iter().find(|&&ix| ix as usize >= vertices.len()) {
        return Err(format!(
            "glTF index {} out of range for {} vertices",
            bad,
            vertices.len()
        )
        .into());
    }

    // Skinning attributes (optional).
    let joints = match attribute_accessor(attrs, "JOINTS_0") {
        Some(idx) => Some(read_accessor_joints(accessors, buffer_views, buffers, idx)?),
        None => None,
    };
    let weights = match attribute_accessor(attrs, "WEIGHTS_0") {
        Some(idx) => Some(read_accessor_vec4(accessors, buffer_views, buffers, idx)?),
        None => None,
    };

    if tangents.is_none() {
        compute_tangents(&mut vertices, &indices);
    }

    let material = prim
        .get("material")
        .and_then(|v| v.as_u32())
        .and_then(|idx| materials_json?.get(idx as usize))
        .and_then(extract_material);

    Ok(GltfMesh {
        vertices,
        indices,
        name: None,
        material,
        joints,
        weights,
    })
}

pub(super) fn get_buffer_data<'a>(
    accessor: &JsonValue,
    buffer_views: &[JsonValue],
    buffers: &'a [Vec<u8>],
) -> crate::Result<(&'a [u8], usize, Option<usize>)> {
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
    // Interleaved vertex data: elements sit `byteStride` apart instead of
    // packed. Only meaningful for vertex attributes (glTF 2.0 §3.6.2.2).
    let stride = bv
        .get("byteStride")
        .and_then(|v| v.as_u32())
        .map(|v| v as usize)
        .filter(|&v| v > 0); // 0 is invalid (spec min 4); read as tightly packed
    Ok((buffer, offset, stride))
}

fn read_accessor_vec3(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<[f32; 3]>> {
    read_accessor_vecn::<3>(accessors, buffer_views, buffers, idx)
}

fn read_accessor_vec2(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<[f32; 2]>> {
    read_accessor_vecn::<2>(accessors, buffer_views, buffers, idx)
}

fn read_accessor_vec4(
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
    idx: usize,
) -> crate::Result<Vec<[f32; 4]>> {
    read_accessor_vecn::<4>(accessors, buffer_views, buffers, idx)
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
    let (buffer, offset, _) = get_buffer_data(acc, buffer_views, buffers)?;

    let mut result = Vec::with_capacity(count);
    match comp_type {
        UNSIGNED_BYTE => {
            for i in 0..count {
                if offset + i >= buffer.len() {
                    return Err("Buffer overflow reading u8 indices".into());
                }
                result.push(buffer[offset + i] as u32);
            }
        }
        UNSIGNED_SHORT => {
            for i in 0..count {
                let o = offset + i * 2;
                if o + 2 > buffer.len() {
                    return Err("Buffer overflow reading u16 indices".into());
                }
                result.push(u16::from_le_bytes([buffer[o], buffer[o + 1]]) as u32);
            }
        }
        UNSIGNED_INT => {
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
        .unwrap_or(UNSIGNED_SHORT);
    let (buffer, offset, stride) = get_buffer_data(acc, buffer_views, buffers)?;
    let mut result = Vec::with_capacity(count);
    match comp_type {
        UNSIGNED_BYTE => {
            for i in 0..count {
                let o = offset + i * stride.unwrap_or(4);
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
        UNSIGNED_SHORT => {
            for i in 0..count {
                let o = offset + i * stride.unwrap_or(8);
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
    let (buffer, offset, _) = get_buffer_data(acc, buffer_views, buffers)?;
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

/// Test helper: `count` tightly packed f32 at `offset`.
#[cfg(test)]
pub(super) fn read_floats(buffer: &[u8], offset: usize, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            let o = offset + i * 4;
            f32::from_le_bytes([buffer[o], buffer[o + 1], buffer[o + 2], buffer[o + 3]])
        })
        .collect()
}

/// Test helper: `count` tightly packed u16 indices at `offset`.
#[cfg(test)]
pub(super) fn read_indices_u16(buffer: &[u8], offset: usize, count: usize) -> Vec<u32> {
    (0..count)
        .map(|i| {
            let o = offset + i * 2;
            u16::from_le_bytes([buffer[o], buffer[o + 1]]) as u32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gltf::component::read_accessor_floats;
    use crate::json_parser::parse_json;

    /// Two interleaved vertices: position (12 B) + uv (8 B) per vertex, so
    /// the shared bufferView has byteStride 20 and the uv accessor starts at
    /// byteOffset 12.
    fn interleaved() -> (Vec<JsonValue>, Vec<JsonValue>, Vec<Vec<u8>>) {
        let verts: [([f32; 3], [f32; 2]); 2] =
            [([1.0, 2.0, 3.0], [0.25, 0.5]), ([4.0, 5.0, 6.0], [0.75, 1.0])];
        let mut buf = Vec::new();
        for (p, uv) in verts {
            p.iter().chain(uv.iter()).for_each(|f| buf.extend(f.to_le_bytes()));
        }
        let accessors = parse_json(
            r#"[{"bufferView":0,"byteOffset":0,"count":2,"type":"VEC3","componentType":5126},
                {"bufferView":0,"byteOffset":12,"count":2,"type":"VEC2","componentType":5126}]"#,
        )
        .unwrap();
        let views = parse_json(r#"[{"buffer":0,"byteOffset":0,"byteLength":40,"byteStride":20}]"#)
            .unwrap();
        (
            accessors.as_array().unwrap().to_vec(),
            views.as_array().unwrap().to_vec(),
            vec![buf],
        )
    }

    #[test]
    fn vec3_reader_honours_byte_stride() {
        let (acc, views, bufs) = interleaved();
        let pos = read_accessor_vec3(&acc, &views, &bufs, 0).unwrap();
        assert_eq!(pos, vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    }

    #[test]
    fn zero_byte_stride_reads_as_tightly_packed() {
        let buf: Vec<u8> = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let acc = parse_json(r#"[{"bufferView":0,"count":2,"type":"VEC3","componentType":5126}]"#).unwrap();
        let views = parse_json(r#"[{"buffer":0,"byteLength":24,"byteStride":0}]"#).unwrap();
        let pos = read_accessor_vec3(acc.as_array().unwrap(), views.as_array().unwrap(), &[buf], 0).unwrap();
        assert_eq!(pos, vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    }

    fn single_view(buf: Vec<u8>, accessor_json: &str) -> (Vec<JsonValue>, Vec<JsonValue>, Vec<Vec<u8>>) {
        let views = format!(r#"[{{"buffer":0,"byteLength":{}}}]"#, buf.len());
        (
            parse_json(accessor_json).unwrap().as_array().unwrap().to_vec(),
            parse_json(&views).unwrap().as_array().unwrap().to_vec(),
            vec![buf],
        )
    }

    #[test]
    fn vec2_reads_normalized_unsigned_short_texcoords() {
        // glTF 2.0 allows TEXCOORD as normalized UNSIGNED_SHORT (65535 = 1.0).
        let buf: Vec<u8> = [0u16, 65535, 32768, 16384].iter().flat_map(|v| v.to_le_bytes()).collect();
        let (acc, views, bufs) = single_view(
            buf,
            r#"[{"bufferView":0,"count":2,"type":"VEC2","componentType":5123,"normalized":true}]"#,
        );
        let uv = read_accessor_vec2(&acc, &views, &bufs, 0).unwrap();
        assert_eq!(uv[0], [0.0, 1.0]);
        assert!((uv[1][0] - 32768.0 / 65535.0).abs() < 1e-6 && (uv[1][1] - 16384.0 / 65535.0).abs() < 1e-6);
    }

    #[test]
    fn vec4_reads_normalized_unsigned_byte_weights() {
        let (acc, views, bufs) = single_view(
            vec![255, 0, 0, 0, 128, 127, 0, 0],
            r#"[{"bufferView":0,"count":2,"type":"VEC4","componentType":5121,"normalized":true}]"#,
        );
        let w = read_accessor_vec4(&acc, &views, &bufs, 0).unwrap();
        assert_eq!(w[0], [1.0, 0.0, 0.0, 0.0]);
        assert!((w[1][0] - 128.0 / 255.0).abs() < 1e-6 && (w[1][1] - 127.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn signed_normalized_components_clamp_to_minus_one() {
        // SHORT normalized: -32768 and -32767 both map to -1.0 (spec: max(c / 32767, -1)).
        let buf: Vec<u8> = [-32768i16, -32767, 32767, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
        let (acc, views, bufs) = single_view(
            buf,
            r#"[{"bufferView":0,"count":2,"type":"VEC2","componentType":5122,"normalized":true}]"#,
        );
        let v = read_accessor_vec2(&acc, &views, &bufs, 0).unwrap();
        assert_eq!(v, vec![[-1.0, -1.0], [1.0, 0.0]]);
    }

    #[test]
    fn animation_output_reads_normalized_short_rotations() {
        // glTF 2.0 allows rotation sampler outputs as normalized SHORT.
        let buf: Vec<u8> = [0i16, 0, 0, 32767].iter().flat_map(|v| v.to_le_bytes()).collect();
        let (acc, views, bufs) = single_view(
            buf,
            r#"[{"bufferView":0,"count":1,"type":"VEC4","componentType":5122,"normalized":true}]"#,
        );
        let q = read_accessor_floats(&acc, &views, &bufs, 0).unwrap();
        assert_eq!(q, vec![0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn unsupported_component_type_is_an_error_not_garbage() {
        let (acc, views, bufs) = single_view(
            vec![0; 16],
            r#"[{"bufferView":0,"count":2,"type":"VEC2","componentType":5125}]"#,
        );
        assert!(read_accessor_vec2(&acc, &views, &bufs, 0).is_err());
    }

    #[test]
    fn vec2_reader_honours_byte_stride() {
        let (acc, views, bufs) = interleaved();
        let uv = read_accessor_vec2(&acc, &views, &bufs, 1).unwrap();
        assert_eq!(uv, vec![[0.25, 0.5], [0.75, 1.0]]);
    }
}
