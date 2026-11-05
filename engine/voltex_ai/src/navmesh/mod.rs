//! Navigation mesh data structure used by [`find_path`](crate::find_path).

use voltex_math::Vec3;

mod format;
pub use format::{deserialize_navmesh_headerless, serialize_navmesh_headerless};
#[allow(deprecated)]
pub use format::{deserialize_navmesh, serialize_navmesh};

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

/// Barycentric weights `(u, v, w)` of `(px, pz)` for triangle `(a, b, c)` in
/// the XZ plane (`u` on `a`, `v` on `b`, `w` on `c`; all ≥ 0 inside), or
/// `None` for a degenerate triangle.
pub(crate) fn barycentric_xz(px: f32, pz: f32, a: Vec3, b: Vec3, c: Vec3) -> Option<(f32, f32, f32)> {
    let denom = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
    if denom.abs() < f32::EPSILON {
        return None;
    }
    let u = ((b.z - c.z) * (px - c.x) + (c.x - b.x) * (pz - c.z)) / denom;
    let v = ((c.z - a.z) * (px - c.x) + (a.x - c.x) * (pz - c.z)) / denom;
    Some((u, v, 1.0 - u - v))
}

/// Test whether `point` lies inside or on the triangle (a, b, c) using XZ barycentric coordinates.
pub(crate) fn point_in_triangle_xz(point: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
    match barycentric_xz(point.x, point.z, a, b, c) {
        Some((u, v, w)) => u >= 0.0 && v >= 0.0 && w >= 0.0,
        None => false, // degenerate triangle
    }
}

#[cfg(test)]
mod tests;
