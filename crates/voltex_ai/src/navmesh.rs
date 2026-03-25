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
}
