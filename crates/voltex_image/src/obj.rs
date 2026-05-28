//! Wavefront OBJ parser — vertices + indices + texture coords + normals.
#![allow(missing_docs)]

use std::collections::HashMap;

use crate::vertex::MeshVertex;

pub struct ObjData {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

pub fn parse_obj(source: &str) -> ObjData {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();

    // Intermediate face data: list of (v_idx, vt_idx, vn_idx) per face
    let mut faces: Vec<Vec<(u32, u32, u32)>> = Vec::new();

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let keyword = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("").trim();

        match keyword {
            "v" => {
                let coords: Vec<f32> = rest
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if coords.len() >= 3 {
                    positions.push([coords[0], coords[1], coords[2]]);
                }
            }
            "vn" => {
                let coords: Vec<f32> = rest
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if coords.len() >= 3 {
                    normals.push([coords[0], coords[1], coords[2]]);
                }
            }
            "vt" => {
                let coords: Vec<f32> = rest
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if coords.len() >= 2 {
                    uvs.push([coords[0], coords[1]]);
                } else if coords.len() == 1 {
                    uvs.push([coords[0], 0.0]);
                }
            }
            "f" => {
                let face: Vec<(u32, u32, u32)> =
                    rest.split_whitespace().map(parse_face_vertex).collect();
                if face.len() >= 3 {
                    faces.push(face);
                }
            }
            _ => {}
        }
    }

    // Deduplicate vertices using a HashMap keyed by (v_idx, vt_idx, vn_idx)
    let mut vertex_map: HashMap<(u32, u32, u32), u32> = HashMap::new();
    let mut vertices: Vec<MeshVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let default_normal = [0.0_f32, 1.0, 0.0];
    let default_uv = [0.0_f32, 0.0];

    for face in &faces {
        // Triangulate using fan method: (0,1,2), (0,2,3), (0,3,4), ...
        let fan_anchor = &face[0];
        for i in 1..(face.len() - 1) {
            let tri = [fan_anchor, &face[i], &face[i + 1]];
            for &&(v_idx, vt_idx, vn_idx) in &tri {
                let key = (v_idx, vt_idx, vn_idx);
                let final_idx = if let Some(&existing) = vertex_map.get(&key) {
                    existing
                } else {
                    // OBJ indices are 1-based; 0 means missing
                    let position = if v_idx > 0 {
                        positions
                            .get((v_idx - 1) as usize)
                            .copied()
                            .unwrap_or([0.0, 0.0, 0.0])
                    } else {
                        [0.0, 0.0, 0.0]
                    };

                    let normal = if vn_idx > 0 {
                        normals
                            .get((vn_idx - 1) as usize)
                            .copied()
                            .unwrap_or(default_normal)
                    } else {
                        default_normal
                    };

                    let uv = if vt_idx > 0 {
                        uvs.get((vt_idx - 1) as usize)
                            .copied()
                            .unwrap_or(default_uv)
                    } else {
                        default_uv
                    };

                    let new_idx = vertices.len() as u32;
                    vertices.push(MeshVertex {
                        position,
                        normal,
                        uv,
                        tangent: [0.0; 4],
                    });
                    vertex_map.insert(key, new_idx);
                    new_idx
                };
                indices.push(final_idx);
            }
        }
    }

    compute_tangents(&mut vertices, &indices);

    ObjData { vertices, indices }
}

pub fn compute_tangents(vertices: &mut [MeshVertex], indices: &[u32]) {
    // Accumulate tangent per vertex from triangles
    let mut tangents = vec![[0.0f32; 3]; vertices.len()];
    let mut bitangents = vec![[0.0f32; 3]; vertices.len()];

    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;

        let v0 = vertices[i0];
        let v1 = vertices[i1];
        let v2 = vertices[i2];

        let edge1 = [
            v1.position[0] - v0.position[0],
            v1.position[1] - v0.position[1],
            v1.position[2] - v0.position[2],
        ];
        let edge2 = [
            v2.position[0] - v0.position[0],
            v2.position[1] - v0.position[1],
            v2.position[2] - v0.position[2],
        ];
        let duv1 = [v1.uv[0] - v0.uv[0], v1.uv[1] - v0.uv[1]];
        let duv2 = [v2.uv[0] - v0.uv[0], v2.uv[1] - v0.uv[1]];

        let det = duv1[0] * duv2[1] - duv2[0] * duv1[1];
        if det.abs() < 1e-8 {
            continue;
        }
        let f = 1.0 / det;

        let t = [
            f * (duv2[1] * edge1[0] - duv1[1] * edge2[0]),
            f * (duv2[1] * edge1[1] - duv1[1] * edge2[1]),
            f * (duv2[1] * edge1[2] - duv1[1] * edge2[2]),
        ];
        let b = [
            f * (-duv2[0] * edge1[0] + duv1[0] * edge2[0]),
            f * (-duv2[0] * edge1[1] + duv1[0] * edge2[1]),
            f * (-duv2[0] * edge1[2] + duv1[0] * edge2[2]),
        ];

        for &idx in &[i0, i1, i2] {
            tangents[idx] = [
                tangents[idx][0] + t[0],
                tangents[idx][1] + t[1],
                tangents[idx][2] + t[2],
            ];
            bitangents[idx] = [
                bitangents[idx][0] + b[0],
                bitangents[idx][1] + b[1],
                bitangents[idx][2] + b[2],
            ];
        }
    }

    // Orthogonalize and compute handedness
    for (i, v) in vertices.iter_mut().enumerate() {
        let n = v.normal;
        let t = tangents[i];
        // Gram-Schmidt orthogonalize: T' = normalize(T - N * dot(N, T))
        let n_dot_t = n[0] * t[0] + n[1] * t[1] + n[2] * t[2];
        let ortho = [
            t[0] - n[0] * n_dot_t,
            t[1] - n[1] * n_dot_t,
            t[2] - n[2] * n_dot_t,
        ];
        let len = (ortho[0] * ortho[0] + ortho[1] * ortho[1] + ortho[2] * ortho[2]).sqrt();
        if len > 1e-8 {
            let normalized = [ortho[0] / len, ortho[1] / len, ortho[2] / len];
            // Handedness: sign of dot(cross(N, T'), B)
            let cross = [
                n[1] * normalized[2] - n[2] * normalized[1],
                n[2] * normalized[0] - n[0] * normalized[2],
                n[0] * normalized[1] - n[1] * normalized[0],
            ];
            let b = bitangents[i];
            let dot_b = cross[0] * b[0] + cross[1] * b[1] + cross[2] * b[2];
            let w = if dot_b < 0.0 { -1.0 } else { 1.0 };
            v.tangent = [normalized[0], normalized[1], normalized[2], w];
        } else {
            v.tangent = [1.0, 0.0, 0.0, 1.0]; // fallback
        }
    }
}

/// Parse a face vertex token of the form "v", "v/vt", "v//vn", or "v/vt/vn".
/// Returns (v_idx, vt_idx, vn_idx) where 0 means absent.
fn parse_face_vertex(token: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = token.split('/').collect();
    let v = parts
        .first()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let vt = parts
        .get(1)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let vn = parts
        .get(2)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    (v, vt, vn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_triangle() {
        let src = "\
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0
vn 0.0 0.0 1.0
f 1//1 2//1 3//1
";
        let data = parse_obj(src);
        assert_eq!(data.vertices.len(), 3);
        assert_eq!(data.indices.len(), 3);

        // Verify positions
        assert_eq!(data.vertices[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(data.vertices[1].position, [1.0, 0.0, 0.0]);
        assert_eq!(data.vertices[2].position, [0.0, 1.0, 0.0]);

        // Verify normals
        for v in &data.vertices {
            assert_eq!(v.normal, [0.0, 0.0, 1.0]);
        }
    }

    #[test]
    fn test_parse_quad_triangulated() {
        let src = "\
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 1.0 1.0 0.0
v 0.0 1.0 0.0
vn 0.0 0.0 1.0
f 1//1 2//1 3//1 4//1
";
        let data = parse_obj(src);
        // 4-vertex quad → 2 triangles → 6 indices
        assert_eq!(data.indices.len(), 6);
        // 4 unique vertices
        assert_eq!(data.vertices.len(), 4);
    }

    #[test]
    fn test_parse_with_uv() {
        let src = "\
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0
vt 0.0 0.0
vt 1.0 0.0
vt 0.0 1.0
vn 0.0 0.0 1.0
f 1/1/1 2/2/1 3/3/1
";
        let data = parse_obj(src);
        assert_eq!(data.vertices.len(), 3);
        assert_eq!(data.indices.len(), 3);

        // Verify UV coordinates
        assert_eq!(data.vertices[0].uv, [0.0, 0.0]);
        assert_eq!(data.vertices[1].uv, [1.0, 0.0]);
        assert_eq!(data.vertices[2].uv, [0.0, 1.0]);
    }

    #[test]
    fn test_vertex_dedup() {
        let src = "\
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0
vn 0.0 0.0 1.0
f 1//1 2//1 3//1
f 1//1 3//1 2//1
";
        let data = parse_obj(src);
        // Both triangles share the same 3 vertices → only 3 unique vertices
        assert_eq!(data.vertices.len(), 3);
        // 2 triangles → 6 indices
        assert_eq!(data.indices.len(), 6);
    }

    // ── Panic-resistance fuzz tests ────────────────────────────────
    //
    // parse_obj returns ObjData directly (no Result) — so the
    // strongest guarantee we can verify is "never panic". Empty
    // input, random UTF-8 bytes, malformed face indices, and very
    // long lines must all return some ObjData (possibly empty).

    fn fuzz_str(seed: u32, len: usize) -> String {
        // Produce printable ASCII so we hit the OBJ parser's actual
        // tokenizer rather than UTF-8 decoding paths.
        let mut s: u32 = if seed == 0 { 0xDEAD_BEEF } else { seed };
        (0..len)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                // Printable ASCII 32..=126 plus newlines.
                let r = (s % 96) as u8;
                if r < 2 {
                    '\n'
                } else {
                    (32 + (r % 95)) as char
                }
            })
            .collect()
    }

    #[test]
    fn test_compute_tangents_xy_triangle() {
        // Triangle in the XY plane, normal +Z, with axis-aligned UVs.
        // U increases along +X, so the tangent must point along +X.
        let mut verts = vec![
            MeshVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                tangent: [0.0; 4],
            },
            MeshVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
                tangent: [0.0; 4],
            },
            MeshVertex {
                position: [0.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
                tangent: [0.0; 4],
            },
        ];
        compute_tangents(&mut verts, &[0, 1, 2]);
        for v in &verts {
            assert!(
                (v.tangent[0] - 1.0).abs() < 1e-5,
                "tangent.x={}",
                v.tangent[0]
            );
            assert!(v.tangent[1].abs() < 1e-5, "tangent.y={}", v.tangent[1]);
            assert!(v.tangent[2].abs() < 1e-5, "tangent.z={}", v.tangent[2]);
            assert!(
                (v.tangent[3].abs() - 1.0).abs() < 1e-5,
                "handedness w={}",
                v.tangent[3]
            );
            // Tangent must be orthogonal to the normal.
            let dot = v.tangent[0] * v.normal[0]
                + v.tangent[1] * v.normal[1]
                + v.tangent[2] * v.normal[2];
            assert!(dot.abs() < 1e-5, "dot(T,N)={}", dot);
        }
    }

    #[test]
    fn test_compute_tangents_degenerate_uv_fallback() {
        // All UVs identical -> duv det ≈ 0 -> triangle skipped -> tangent stays
        // zero -> Gram-Schmidt length ≈ 0 -> documented fallback [1,0,0,1].
        let mut verts = vec![
            MeshVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.5, 0.5],
                tangent: [0.0; 4],
            },
            MeshVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.5, 0.5],
                tangent: [0.0; 4],
            },
            MeshVertex {
                position: [0.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.5, 0.5],
                tangent: [0.0; 4],
            },
        ];
        compute_tangents(&mut verts, &[0, 1, 2]);
        for v in &verts {
            assert_eq!(v.tangent, [1.0, 0.0, 0.0, 1.0], "expected fallback tangent");
        }
    }

    #[test]
    fn fuzz_parse_obj_no_panic_on_empty() {
        let _ = parse_obj("");
    }

    #[test]
    fn fuzz_parse_obj_no_panic_on_random_text() {
        for seed in 1..=128u32 {
            for &len in &[0usize, 1, 16, 128, 1024, 4096] {
                let _ = parse_obj(&fuzz_str(seed, len));
            }
        }
    }

    #[test]
    fn fuzz_parse_obj_no_panic_on_malformed_directives() {
        // Each line starts with a known OBJ directive but has
        // garbage operands — the parser must not panic on missing or
        // non-numeric fields.
        let cases = [
            "v",
            "v 1.0",
            "v 1.0 2.0",
            "v abc def ghi",
            "vn",
            "vn 1",
            "vt",
            "f",
            "f 1",
            "f 1/2",
            "f 1/2/3 4/5/6",
            "f 999999 999999 999999",    // indices way out of range
            "f -1 -2 -3",                // negative (relative) indices
            "f 1/2/3/4 1/2/3/4 1/2/3/4", // too many slashes
            "g name",
            "o name",
            "s 1",
            "# comment",
            "",
        ];
        for c in &cases {
            let _ = parse_obj(c);
            // Also wrap with the previous-directive context to catch
            // ordering-dependent panics.
            let _ = parse_obj(&format!("v 1.0 2.0 3.0\n{}", c));
        }
    }

    #[test]
    fn fuzz_parse_obj_no_panic_on_very_long_lines() {
        // 100 KB single line of "v 1 2 3 " repeated — exercises the
        // line buffer / splitter, not a normal OBJ shape.
        let long = "v 1 2 3 ".repeat(12500);
        let _ = parse_obj(&long);
    }
}
