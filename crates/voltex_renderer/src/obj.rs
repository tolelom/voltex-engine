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
                let face: Vec<(u32, u32, u32)> = rest
                    .split_whitespace()
                    .map(|token| parse_face_vertex(token))
                    .collect();
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
                    });
                    vertex_map.insert(key, new_idx);
                    new_idx
                };
                indices.push(final_idx);
            }
        }
    }

    ObjData { vertices, indices }
}

/// Parse a face vertex token of the form "v", "v/vt", "v//vn", or "v/vt/vn".
/// Returns (v_idx, vt_idx, vn_idx) where 0 means absent.
fn parse_face_vertex(token: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = token.split('/').collect();
    let v = parts.get(0).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    let vt = parts.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    let vn = parts.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
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
}
