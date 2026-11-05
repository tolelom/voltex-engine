//! NavMesh binary formats.
//!
//! Both formats share one body, all little-endian:
//! - `vertex_count: u32` + `vertex_count × [f32; 3]`
//! - `triangle_count: u32` + per triangle `indices: [u32; 3]` +
//!   `neighbors: [i32; 3]` (-1 = no neighbour on that edge)
//!
//! The VNAV file format ([`NavMesh::to_vnav_bytes`]) puts an 8-byte header
//! (`b"VNAV"` + `version: u32`) in front of the body; the headerless format
//! ([`serialize_navmesh_headerless`]) is the body alone.

use super::{NavMesh, NavTriangle};
use voltex_math::Vec3;

const VNAV_MAGIC: &[u8; 4] = b"VNAV";
const VNAV_VERSION: u32 = 1;
/// Magic (4 bytes) + version (u32).
const VNAV_HEADER_LEN: usize = 8;

/// Maximum vertex / triangle count we accept in a single navmesh file. Real
/// navmeshes have at most a few hundred thousand polygons; this limit
/// catches adversarial headers without restricting realistic use.
const MAX_NAVMESH_VERTICES: usize = 1_000_000;
const MAX_NAVMESH_TRIANGLES: usize = 1_000_000;

impl NavMesh {
    /// Serialize to the VNAV file format: `b"VNAV"` + version `1` (u32) + the
    /// shared body (see the [module docs](self)).
    pub fn to_vnav_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(VNAV_MAGIC);
        data.extend_from_slice(&VNAV_VERSION.to_le_bytes());
        write_body(self, &mut data);
        data
    }

    /// Parse the VNAV file format written by [`NavMesh::to_vnav_bytes`],
    /// checking magic and version first. Body validation is the same as
    /// [`deserialize_navmesh_headerless`].
    pub fn from_vnav_bytes(data: &[u8]) -> crate::Result<Self> {
        if data.len() < VNAV_HEADER_LEN {
            return Err("data too short for header".into());
        }

        if &data[0..4] != VNAV_MAGIC {
            return Err(format!(
                "invalid magic: expected VNAV, got {:?}",
                &data[0..4]
            ).into());
        }

        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != VNAV_VERSION {
            return Err(format!("unsupported version: {}", version).into());
        }

        read_body(&data[VNAV_HEADER_LEN..])
    }

    /// Old name of [`NavMesh::to_vnav_bytes`].
    #[deprecated(note = "use `to_vnav_bytes`")]
    pub fn serialize(&self) -> Vec<u8> {
        self.to_vnav_bytes()
    }

    /// Old name of [`NavMesh::from_vnav_bytes`].
    #[deprecated(note = "use `from_vnav_bytes`")]
    pub fn deserialize(data: &[u8]) -> crate::Result<Self> {
        Self::from_vnav_bytes(data)
    }
}

/// Serialize a NavMesh to the headerless format (the VNAV body without
/// magic/version — see the [module docs](self)).
pub fn serialize_navmesh_headerless(navmesh: &NavMesh) -> Vec<u8> {
    let mut data = Vec::new();
    write_body(navmesh, &mut data);
    data
}

/// Parse the headerless format written by [`serialize_navmesh_headerless`].
///
/// Validation: vertex/triangle counts are capped at 1M each. Triangle
/// vertex indices are validated to be inside the vertex array, and
/// neighbour indices (when present) are validated to be inside the
/// triangle array. This means downstream consumers
/// (`NavMesh::shared_edge`, `point_in_triangle_xz`) can use these
/// indices without re-checking.
pub fn deserialize_navmesh_headerless(data: &[u8]) -> crate::Result<NavMesh> {
    read_body(data)
}

/// Old name of [`serialize_navmesh_headerless`].
#[deprecated(note = "use `serialize_navmesh_headerless`")]
pub fn serialize_navmesh(navmesh: &NavMesh) -> Vec<u8> {
    serialize_navmesh_headerless(navmesh)
}

/// Old name of [`deserialize_navmesh_headerless`].
#[deprecated(note = "use `deserialize_navmesh_headerless`")]
pub fn deserialize_navmesh(data: &[u8]) -> crate::Result<NavMesh> {
    deserialize_navmesh_headerless(data)
}

fn write_body(navmesh: &NavMesh, data: &mut Vec<u8>) {
    data.extend_from_slice(&(navmesh.vertices.len() as u32).to_le_bytes());
    for v in &navmesh.vertices {
        data.extend_from_slice(&v.x.to_le_bytes());
        data.extend_from_slice(&v.y.to_le_bytes());
        data.extend_from_slice(&v.z.to_le_bytes());
    }

    data.extend_from_slice(&(navmesh.triangles.len() as u32).to_le_bytes());
    for tri in &navmesh.triangles {
        for &idx in &tri.indices {
            data.extend_from_slice(&(idx as u32).to_le_bytes());
        }
        for &neighbor in &tri.neighbors {
            let encoded: i32 = match neighbor {
                Some(i) => i as i32,
                None => -1,
            };
            data.extend_from_slice(&encoded.to_le_bytes());
        }
    }
}

fn read_body(data: &[u8]) -> crate::Result<NavMesh> {
    let mut reader = ByteReader { data, offset: 0 };

    let vertex_count = reader.read_u32()? as usize;
    if vertex_count > MAX_NAVMESH_VERTICES {
        return Err(format!(
            "navmesh vertex count {} exceeds limit {}",
            vertex_count, MAX_NAVMESH_VERTICES
        ).into());
    }
    let mut vertices = Vec::with_capacity(vertex_count);
    for _ in 0..vertex_count {
        let x = reader.read_f32()?;
        let y = reader.read_f32()?;
        let z = reader.read_f32()?;
        vertices.push(Vec3::new(x, y, z));
    }

    let triangle_count = reader.read_u32()? as usize;
    if triangle_count > MAX_NAVMESH_TRIANGLES {
        return Err(format!(
            "navmesh triangle count {} exceeds limit {}",
            triangle_count, MAX_NAVMESH_TRIANGLES
        ).into());
    }
    let mut triangles = Vec::with_capacity(triangle_count);
    for _ in 0..triangle_count {
        let i0 = reader.read_u32()? as usize;
        let i1 = reader.read_u32()? as usize;
        let i2 = reader.read_u32()? as usize;
        if i0 >= vertex_count || i1 >= vertex_count || i2 >= vertex_count {
            return Err(format!(
                "navmesh triangle vertex index out of range (vertices={})",
                vertex_count
            ).into());
        }
        let n0 = reader.read_i32()?;
        let n1 = reader.read_i32()?;
        let n2 = reader.read_i32()?;
        let decode_neighbor = |encoded: i32| -> crate::Result<Option<usize>> {
            if encoded < 0 {
                return Ok(None);
            }
            let idx = encoded as usize;
            if idx >= triangle_count {
                Err(format!(
                    "navmesh neighbour triangle index {} out of range (triangles={})",
                    idx, triangle_count
                ).into())
            } else {
                Ok(Some(idx))
            }
        };
        triangles.push(NavTriangle {
            indices: [i0, i1, i2],
            neighbors: [decode_neighbor(n0)?, decode_neighbor(n1)?, decode_neighbor(n2)?],
        });
    }

    Ok(NavMesh::new(vertices, triangles))
}

/// Little-endian cursor over a byte slice; every read fails cleanly at the end.
struct ByteReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl ByteReader<'_> {
    fn read_4(&mut self) -> crate::Result<[u8; 4]> {
        if self.offset + 4 > self.data.len() {
            return Err("unexpected end of data".into());
        }
        let d = &self.data[self.offset..self.offset + 4];
        self.offset += 4;
        Ok([d[0], d[1], d[2], d[3]])
    }

    fn read_u32(&mut self) -> crate::Result<u32> {
        Ok(u32::from_le_bytes(self.read_4()?))
    }

    fn read_i32(&mut self) -> crate::Result<i32> {
        Ok(i32::from_le_bytes(self.read_4()?))
    }

    fn read_f32(&mut self) -> crate::Result<f32> {
        Ok(f32::from_le_bytes(self.read_4()?))
    }
}
