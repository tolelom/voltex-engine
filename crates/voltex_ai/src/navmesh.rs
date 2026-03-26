use voltex_math::Vec3;

/// A triangle in the navigation mesh, defined by vertex indices and neighbor triangle indices.
#[derive(Debug, Clone)]
pub struct NavTriangle {
    /// Indices into NavMesh::vertices
    pub indices: [usize; 3],
    /// Neighboring triangle indices (None if no neighbor on that edge)
    pub neighbors: [Option<usize>; 3],
}

/// Navigation mesh composed of triangles for pathfinding.
pub struct NavMesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<NavTriangle>,
}

impl NavMesh {
    pub fn new(vertices: Vec<Vec3>, triangles: Vec<NavTriangle>) -> Self {
        Self { vertices, triangles }
    }

    /// Find the index of the triangle that contains `point` (XZ plane test).
    /// Returns None if the point is outside all triangles.
    pub fn find_triangle(&self, point: Vec3) -> Option<usize> {
        for (i, tri) in self.triangles.iter().enumerate() {
            let a = self.vertices[tri.indices[0]];
            let b = self.vertices[tri.indices[1]];
            let c = self.vertices[tri.indices[2]];
            if point_in_triangle_xz(point, a, b, c) {
                return Some(i);
            }
        }
        None
    }

    /// Return the centroid of triangle `tri_idx` in 3D space.
    pub fn triangle_center(&self, tri_idx: usize) -> Vec3 {
        let tri = &self.triangles[tri_idx];
        let a = self.vertices[tri.indices[0]];
        let b = self.vertices[tri.indices[1]];
        let c = self.vertices[tri.indices[2]];
        Vec3::new(
            (a.x + b.x + c.x) / 3.0,
            (a.y + b.y + c.y) / 3.0,
            (a.z + b.z + c.z) / 3.0,
        )
    }

    /// Return the midpoint of the shared edge between `tri_idx` and neighbor slot `edge_idx` (0, 1, or 2).
    /// Edge 0 is between vertex indices[0] and indices[1],
    /// Edge 1 is between indices[1] and indices[2],
    /// Edge 2 is between indices[2] and indices[0].
    pub fn edge_midpoint(&self, tri_idx: usize, edge_idx: usize) -> Vec3 {
        let tri = &self.triangles[tri_idx];
        let (i0, i1) = match edge_idx {
            0 => (tri.indices[0], tri.indices[1]),
            1 => (tri.indices[1], tri.indices[2]),
            2 => (tri.indices[2], tri.indices[0]),
            _ => panic!("edge_idx must be 0, 1, or 2"),
        };
        let a = self.vertices[i0];
        let b = self.vertices[i1];
        Vec3::new(
            (a.x + b.x) / 2.0,
            (a.y + b.y) / 2.0,
            (a.z + b.z) / 2.0,
        )
    }

    /// Serialize the NavMesh to a binary format with header.
    ///
    /// Format:
    /// - Magic: "VNAV" (4 bytes)
    /// - Version: u32 (little-endian)
    /// - num_vertices: u32
    /// - vertices: [f32; 3] * num_vertices
    /// - num_triangles: u32
    /// - triangles: for each triangle: indices [u32; 3] + neighbors [i32; 3] (-1 for None)
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        // Magic
        data.extend_from_slice(b"VNAV");
        // Version
        data.extend_from_slice(&1u32.to_le_bytes());

        // Vertices
        data.extend_from_slice(&(self.vertices.len() as u32).to_le_bytes());
        for v in &self.vertices {
            data.extend_from_slice(&v.x.to_le_bytes());
            data.extend_from_slice(&v.y.to_le_bytes());
            data.extend_from_slice(&v.z.to_le_bytes());
        }

        // Triangles
        data.extend_from_slice(&(self.triangles.len() as u32).to_le_bytes());
        for tri in &self.triangles {
            for &idx in &tri.indices {
                data.extend_from_slice(&(idx as u32).to_le_bytes());
            }
            for &nb in &tri.neighbors {
                let val: i32 = match nb {
                    Some(i) => i as i32,
                    None => -1,
                };
                data.extend_from_slice(&val.to_le_bytes());
            }
        }

        data
    }

    /// Deserialize a NavMesh from binary data with header validation.
    pub fn deserialize(data: &[u8]) -> Result<Self, String> {
        if data.len() < 8 {
            return Err("data too short for header".to_string());
        }

        // Validate magic
        if &data[0..4] != b"VNAV" {
            return Err(format!(
                "invalid magic: expected VNAV, got {:?}",
                &data[0..4]
            ));
        }

        // Read version
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != 1 {
            return Err(format!("unsupported version: {}", version));
        }

        // Delegate rest to offset-based reading
        let mut offset = 8usize;

        let read_u32 = |off: &mut usize| -> Result<u32, String> {
            if *off + 4 > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let val = u32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
            *off += 4;
            Ok(val)
        };

        let read_i32 = |off: &mut usize| -> Result<i32, String> {
            if *off + 4 > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let val = i32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
            *off += 4;
            Ok(val)
        };

        let read_f32 = |off: &mut usize| -> Result<f32, String> {
            if *off + 4 > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let val = f32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
            *off += 4;
            Ok(val)
        };

        let vc = read_u32(&mut offset)? as usize;
        let mut vertices = Vec::with_capacity(vc);
        for _ in 0..vc {
            let x = read_f32(&mut offset)?;
            let y = read_f32(&mut offset)?;
            let z = read_f32(&mut offset)?;
            vertices.push(Vec3::new(x, y, z));
        }

        let tc = read_u32(&mut offset)? as usize;
        let mut triangles = Vec::with_capacity(tc);
        for _ in 0..tc {
            let i0 = read_u32(&mut offset)? as usize;
            let i1 = read_u32(&mut offset)? as usize;
            let i2 = read_u32(&mut offset)? as usize;
            let n0 = read_i32(&mut offset)?;
            let n1 = read_i32(&mut offset)?;
            let n2 = read_i32(&mut offset)?;
            let to_opt = |v: i32| -> Option<usize> {
                if v < 0 { None } else { Some(v as usize) }
            };
            triangles.push(NavTriangle {
                indices: [i0, i1, i2],
                neighbors: [to_opt(n0), to_opt(n1), to_opt(n2)],
            });
        }

        Ok(NavMesh::new(vertices, triangles))
    }

    /// Return the shared edge (portal) vertices between two adjacent triangles.
    /// Returns (left, right) vertices of the portal edge as seen from `from_tri` looking toward `to_tri`.
    /// Returns None if the triangles are not adjacent.
    pub fn shared_edge(&self, from_tri: usize, to_tri: usize) -> Option<(Vec3, Vec3)> {
        let tri = &self.triangles[from_tri];
        for edge_idx in 0..3 {
            if tri.neighbors[edge_idx] == Some(to_tri) {
                let (i0, i1) = match edge_idx {
                    0 => (tri.indices[0], tri.indices[1]),
                    1 => (tri.indices[1], tri.indices[2]),
                    2 => (tri.indices[2], tri.indices[0]),
                    _ => unreachable!(),
                };
                return Some((self.vertices[i0], self.vertices[i1]));
            }
        }
        None
    }
}

/// Test whether `point` lies inside or on the triangle (a, b, c) using XZ barycentric coordinates.
pub fn point_in_triangle_xz(point: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
    // Compute barycentric coordinates using XZ plane
    let px = point.x;
    let pz = point.z;
    let ax = a.x; let az = a.z;
    let bx = b.x; let bz = b.z;
    let cx = c.x; let cz = c.z;

    let denom = (bz - cz) * (ax - cx) + (cx - bx) * (az - cz);
    if denom.abs() < f32::EPSILON {
        return false; // degenerate triangle
    }

    let u = ((bz - cz) * (px - cx) + (cx - bx) * (pz - cz)) / denom;
    let v = ((cz - az) * (px - cx) + (ax - cx) * (pz - cz)) / denom;
    let w = 1.0 - u - v;

    u >= 0.0 && v >= 0.0 && w >= 0.0
}

/// Serialize a NavMesh to binary format.
/// Format: vertex_count(u32) + vertices(f32*3 each) + triangle_count(u32) + triangles(indices u32*3 + neighbors i32*3 each)
pub fn serialize_navmesh(navmesh: &NavMesh) -> Vec<u8> {
    let mut data = Vec::new();

    // Vertex count
    let vc = navmesh.vertices.len() as u32;
    data.extend_from_slice(&vc.to_le_bytes());

    // Vertices
    for v in &navmesh.vertices {
        data.extend_from_slice(&v.x.to_le_bytes());
        data.extend_from_slice(&v.y.to_le_bytes());
        data.extend_from_slice(&v.z.to_le_bytes());
    }

    // Triangle count
    let tc = navmesh.triangles.len() as u32;
    data.extend_from_slice(&tc.to_le_bytes());

    // Triangles: indices(u32*3) + neighbors(i32*3, -1 for None)
    for tri in &navmesh.triangles {
        for &idx in &tri.indices {
            data.extend_from_slice(&(idx as u32).to_le_bytes());
        }
        for &nb in &tri.neighbors {
            let val: i32 = match nb {
                Some(i) => i as i32,
                None => -1,
            };
            data.extend_from_slice(&val.to_le_bytes());
        }
    }

    data
}

/// Deserialize a NavMesh from binary data.
pub fn deserialize_navmesh(data: &[u8]) -> Result<NavMesh, String> {
    let mut offset = 0;

    let read_u32 = |off: &mut usize| -> Result<u32, String> {
        if *off + 4 > data.len() {
            return Err("unexpected end of data".to_string());
        }
        let val = u32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
        *off += 4;
        Ok(val)
    };

    let read_i32 = |off: &mut usize| -> Result<i32, String> {
        if *off + 4 > data.len() {
            return Err("unexpected end of data".to_string());
        }
        let val = i32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
        *off += 4;
        Ok(val)
    };

    let read_f32 = |off: &mut usize| -> Result<f32, String> {
        if *off + 4 > data.len() {
            return Err("unexpected end of data".to_string());
        }
        let val = f32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
        *off += 4;
        Ok(val)
    };

    let vc = read_u32(&mut offset)? as usize;
    let mut vertices = Vec::with_capacity(vc);
    for _ in 0..vc {
        let x = read_f32(&mut offset)?;
        let y = read_f32(&mut offset)?;
        let z = read_f32(&mut offset)?;
        vertices.push(Vec3::new(x, y, z));
    }

    let tc = read_u32(&mut offset)? as usize;
    let mut triangles = Vec::with_capacity(tc);
    for _ in 0..tc {
        let i0 = read_u32(&mut offset)? as usize;
        let i1 = read_u32(&mut offset)? as usize;
        let i2 = read_u32(&mut offset)? as usize;
        let n0 = read_i32(&mut offset)?;
        let n1 = read_i32(&mut offset)?;
        let n2 = read_i32(&mut offset)?;
        let to_opt = |v: i32| -> Option<usize> {
            if v < 0 { None } else { Some(v as usize) }
        };
        triangles.push(NavTriangle {
            indices: [i0, i1, i2],
            neighbors: [to_opt(n0), to_opt(n1), to_opt(n2)],
        });
    }

    Ok(NavMesh::new(vertices, triangles))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_navmesh() -> NavMesh {
        // Two triangles sharing an edge:
        //   v0=(0,0,0), v1=(2,0,0), v2=(1,0,2), v3=(3,0,2)
        // Tri 0: v0, v1, v2  (left triangle)
        // Tri 1: v1, v3, v2  (right triangle)
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 2.0),
            Vec3::new(3.0, 0.0, 2.0),
        ];
        let triangles = vec![
            NavTriangle {
                indices: [0, 1, 2],
                neighbors: [None, Some(1), None],
            },
            NavTriangle {
                indices: [1, 3, 2],
                neighbors: [None, None, Some(0)],
            },
        ];
        NavMesh::new(vertices, triangles)
    }

    #[test]
    fn test_find_triangle_inside() {
        let nm = make_simple_navmesh();
        // Center of tri 0 is roughly (1, 0, 0.67)
        let p = Vec3::new(1.0, 0.0, 0.5);
        let result = nm.find_triangle(p);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn test_find_triangle_outside() {
        let nm = make_simple_navmesh();
        // Point far outside the mesh
        let p = Vec3::new(10.0, 0.0, 10.0);
        let result = nm.find_triangle(p);
        assert_eq!(result, None);
    }

    #[test]
    fn test_triangle_center() {
        let nm = make_simple_navmesh();
        let center = nm.triangle_center(0);
        // (0+2+1)/3 = 1.0, (0+0+0)/3 = 0.0, (0+0+2)/3 = 0.667
        assert!((center.x - 1.0).abs() < 1e-5);
        assert!((center.y - 0.0).abs() < 1e-5);
        assert!((center.z - (2.0 / 3.0)).abs() < 1e-5);
    }

    #[test]
    fn test_edge_midpoint() {
        let nm = make_simple_navmesh();
        // Edge 1 of tri 0 is between indices[1]=v1=(2,0,0) and indices[2]=v2=(1,0,2)
        let mid = nm.edge_midpoint(0, 1);
        assert!((mid.x - 1.5).abs() < 1e-5);
        assert!((mid.y - 0.0).abs() < 1e-5);
        assert!((mid.z - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_shared_edge() {
        let nm = make_simple_navmesh();
        // Tri 0 edge 1 connects to Tri 1: shared edge is v1=(2,0,0) and v2=(1,0,2)
        let (left, right) = nm.shared_edge(0, 1).expect("should find shared edge");
        assert_eq!(left, Vec3::new(2.0, 0.0, 0.0));
        assert_eq!(right, Vec3::new(1.0, 0.0, 2.0));
    }

    #[test]
    fn test_shared_edge_not_adjacent() {
        let nm = make_simple_navmesh();
        // Tri 0 is not adjacent to itself via neighbors
        assert!(nm.shared_edge(0, 0).is_none());
    }

    #[test]
    fn test_serialize_roundtrip() {
        let nm = make_simple_navmesh();
        let data = serialize_navmesh(&nm);
        let nm2 = deserialize_navmesh(&data).expect("should deserialize");
        assert_eq!(nm2.vertices.len(), nm.vertices.len());
        assert_eq!(nm2.triangles.len(), nm.triangles.len());
        for (a, b) in nm.vertices.iter().zip(nm2.vertices.iter()) {
            assert!((a.x - b.x).abs() < 1e-6);
            assert!((a.y - b.y).abs() < 1e-6);
            assert!((a.z - b.z).abs() < 1e-6);
        }
        for (a, b) in nm.triangles.iter().zip(nm2.triangles.iter()) {
            assert_eq!(a.indices, b.indices);
            assert_eq!(a.neighbors, b.neighbors);
        }
    }

    #[test]
    fn test_deserialize_empty_data() {
        let result = deserialize_navmesh(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_method_serialize_deserialize_roundtrip() {
        let nm = make_simple_navmesh();
        let data = nm.serialize();
        let restored = NavMesh::deserialize(&data).expect("should deserialize");
        assert_eq!(nm.vertices.len(), restored.vertices.len());
        assert_eq!(nm.triangles.len(), restored.triangles.len());
        for (a, b) in nm.vertices.iter().zip(restored.vertices.iter()) {
            assert!((a.x - b.x).abs() < 1e-6);
            assert!((a.y - b.y).abs() < 1e-6);
            assert!((a.z - b.z).abs() < 1e-6);
        }
        for (a, b) in nm.triangles.iter().zip(restored.triangles.iter()) {
            assert_eq!(a.indices, b.indices);
            assert_eq!(a.neighbors, b.neighbors);
        }
    }

    #[test]
    fn test_deserialize_invalid_magic() {
        let data = b"XXXX\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert!(NavMesh::deserialize(data).is_err());
    }

    #[test]
    fn test_deserialize_too_short() {
        let data = b"VNA";
        assert!(NavMesh::deserialize(data).is_err());
    }

    #[test]
    fn test_serialize_empty_navmesh() {
        let nm = NavMesh::new(vec![], vec![]);
        let data = nm.serialize();
        let restored = NavMesh::deserialize(&data).expect("should deserialize empty");
        assert!(restored.vertices.is_empty());
        assert!(restored.triangles.is_empty());
    }

    #[test]
    fn test_serialize_header_format() {
        let nm = NavMesh::new(vec![], vec![]);
        let data = nm.serialize();
        // Check magic
        assert_eq!(&data[0..4], b"VNAV");
        // Check version = 1
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        assert_eq!(version, 1);
    }

    #[test]
    fn test_deserialize_unsupported_version() {
        let mut data = Vec::new();
        data.extend_from_slice(b"VNAV");
        data.extend_from_slice(&99u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        assert!(NavMesh::deserialize(&data).is_err());
    }
}
