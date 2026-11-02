//! Navigation mesh data structure used by [`find_path`](crate::find_path).

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
    /// All triangle vertex positions.
    pub vertices: Vec<Vec3>,
    /// Triangles indexing into [`vertices`](Self::vertices), with adjacency.
    pub triangles: Vec<NavTriangle>,
}

impl NavMesh {
    /// Builds a navmesh from pre-computed vertex + triangle arrays. The caller
    /// is responsible for the `neighbors` adjacency being consistent.
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

    /// Return the midpoint of the shared edge between `tri_idx` and neighbor
    /// slot `edge_idx` (0, 1, or 2).
    ///
    /// - Edge 0 is between vertex `indices\[0\]` and `indices\[1\]`.
    /// - Edge 1 is between `indices\[1\]` and `indices\[2\]`.
    /// - Edge 2 is between `indices\[2\]` and `indices\[0\]`.
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
    pub fn deserialize(data: &[u8]) -> crate::Result<Self> {
        if data.len() < 8 {
            return Err("data too short for header".into());
        }

        // Validate magic
        if &data[0..4] != b"VNAV" {
            return Err(format!(
                "invalid magic: expected VNAV, got {:?}",
                &data[0..4]
            ).into());
        }

        // Read version
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != 1 {
            return Err(format!("unsupported version: {}", version).into());
        }

        // Delegate rest to offset-based reading
        let mut offset = 8usize;

        let read_u32 = |off: &mut usize| -> crate::Result<u32> {
            if *off + 4 > data.len() {
                return Err("unexpected end of data".into());
            }
            let val = u32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
            *off += 4;
            Ok(val)
        };

        let read_i32 = |off: &mut usize| -> crate::Result<i32> {
            if *off + 4 > data.len() {
                return Err("unexpected end of data".into());
            }
            let val = i32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
            *off += 4;
            Ok(val)
        };

        let read_f32 = |off: &mut usize| -> crate::Result<f32> {
            if *off + 4 > data.len() {
                return Err("unexpected end of data".into());
            }
            let val = f32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
            *off += 4;
            Ok(val)
        };

        let vc = read_u32(&mut offset)? as usize;
        if vc > MAX_NAVMESH_VERTICES {
            return Err(format!(
                "navmesh vertex count {} exceeds limit {}",
                vc, MAX_NAVMESH_VERTICES
            ).into());
        }
        let mut vertices = Vec::with_capacity(vc);
        for _ in 0..vc {
            let x = read_f32(&mut offset)?;
            let y = read_f32(&mut offset)?;
            let z = read_f32(&mut offset)?;
            vertices.push(Vec3::new(x, y, z));
        }

        let tc = read_u32(&mut offset)? as usize;
        if tc > MAX_NAVMESH_TRIANGLES {
            return Err(format!(
                "navmesh triangle count {} exceeds limit {}",
                tc, MAX_NAVMESH_TRIANGLES
            ).into());
        }
        let mut triangles = Vec::with_capacity(tc);
        for _ in 0..tc {
            let i0 = read_u32(&mut offset)? as usize;
            let i1 = read_u32(&mut offset)? as usize;
            let i2 = read_u32(&mut offset)? as usize;
            if i0 >= vc || i1 >= vc || i2 >= vc {
                return Err(format!(
                    "navmesh triangle vertex index out of range (vertices={})",
                    vc
                ).into());
            }
            let n0 = read_i32(&mut offset)?;
            let n1 = read_i32(&mut offset)?;
            let n2 = read_i32(&mut offset)?;
            let to_opt = |v: i32, tc: usize| -> crate::Result<Option<usize>> {
                if v < 0 {
                    Ok(None)
                } else {
                    let idx = v as usize;
                    if idx >= tc {
                        Err(format!(
                            "navmesh neighbour triangle index {} out of range (triangles={})",
                            idx, tc
                        ).into())
                    } else {
                        Ok(Some(idx))
                    }
                }
            };
            triangles.push(NavTriangle {
                indices: [i0, i1, i2],
                neighbors: [to_opt(n0, tc)?, to_opt(n1, tc)?, to_opt(n2, tc)?],
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
pub(crate) fn point_in_triangle_xz(point: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
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

/// Maximum vertex / triangle count we accept in a single VNAV file. Real
/// navmeshes have at most a few hundred thousand polygons; this limit
/// catches adversarial headers without restricting realistic use.
const MAX_NAVMESH_VERTICES: usize = 1_000_000;
const MAX_NAVMESH_TRIANGLES: usize = 1_000_000;

/// Deserialize a NavMesh from binary data.
///
/// Validation: vertex/triangle counts are capped at 1M each. Triangle
/// vertex indices are validated to be inside the vertex array, and
/// neighbour indices (when present) are validated to be inside the
/// triangle array. This means downstream consumers
/// (`NavMesh::shared_edge`, `point_in_triangle_xz`) can use these
/// indices without re-checking.
pub fn deserialize_navmesh(data: &[u8]) -> crate::Result<NavMesh> {
    let mut offset = 0;

    let read_u32 = |off: &mut usize| -> crate::Result<u32> {
        if *off + 4 > data.len() {
            return Err("unexpected end of data".into());
        }
        let val = u32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
        *off += 4;
        Ok(val)
    };

    let read_i32 = |off: &mut usize| -> crate::Result<i32> {
        if *off + 4 > data.len() {
            return Err("unexpected end of data".into());
        }
        let val = i32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
        *off += 4;
        Ok(val)
    };

    let read_f32 = |off: &mut usize| -> crate::Result<f32> {
        if *off + 4 > data.len() {
            return Err("unexpected end of data".into());
        }
        let val = f32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
        *off += 4;
        Ok(val)
    };

    let vc = read_u32(&mut offset)? as usize;
    if vc > MAX_NAVMESH_VERTICES {
        return Err(format!(
            "navmesh vertex count {} exceeds limit {}",
            vc, MAX_NAVMESH_VERTICES
        ).into());
    }
    let mut vertices = Vec::with_capacity(vc);
    for _ in 0..vc {
        let x = read_f32(&mut offset)?;
        let y = read_f32(&mut offset)?;
        let z = read_f32(&mut offset)?;
        vertices.push(Vec3::new(x, y, z));
    }

    let tc = read_u32(&mut offset)? as usize;
    if tc > MAX_NAVMESH_TRIANGLES {
        return Err(format!(
            "navmesh triangle count {} exceeds limit {}",
            tc, MAX_NAVMESH_TRIANGLES
        ).into());
    }
    let mut triangles = Vec::with_capacity(tc);
    for _ in 0..tc {
        let i0 = read_u32(&mut offset)? as usize;
        let i1 = read_u32(&mut offset)? as usize;
        let i2 = read_u32(&mut offset)? as usize;
        if i0 >= vc || i1 >= vc || i2 >= vc {
            return Err(format!(
                "navmesh triangle vertex index out of range (vertices={})",
                vc
            ).into());
        }
        let n0 = read_i32(&mut offset)?;
        let n1 = read_i32(&mut offset)?;
        let n2 = read_i32(&mut offset)?;
        let to_opt = |v: i32, tc: usize| -> crate::Result<Option<usize>> {
            if v < 0 {
                Ok(None)
            } else {
                let idx = v as usize;
                if idx >= tc {
                    Err(format!(
                        "navmesh neighbour triangle index {} out of range (triangles={})",
                        idx, tc
                    ).into())
                } else {
                    Ok(Some(idx))
                }
            }
        };
        triangles.push(NavTriangle {
            indices: [i0, i1, i2],
            neighbors: [to_opt(n0, tc)?, to_opt(n1, tc)?, to_opt(n2, tc)?],
        });
    }

    Ok(NavMesh::new(vertices, triangles))
}

#[cfg(test)]
mod tests;
