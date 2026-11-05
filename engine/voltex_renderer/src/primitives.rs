//! Built-in primitive mesh generators — the shapes every game reaches for.
//! Each returns a CPU-side [`MeshData`] (like
//! [`generate_sphere_mesh`](crate::generate_sphere_mesh)), ready for
//! [`MeshData::upload`]. Vertices carry correct per-face normals, UVs, and
//! tangents so they work with both the colored and PBR (normal-mapped) paths.

use voltex_image::MeshVertex;

/// CPU-side triangle mesh: vertices plus a triangle-list index buffer.
#[derive(Debug, Clone)]
pub struct MeshData {
    /// Interleaved vertex data.
    pub vertices: Vec<MeshVertex>,
    /// Triangle-list indices into `vertices` (3 per triangle).
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Upload to GPU buffers — shorthand for
    /// `Mesh::new(device, &self.vertices, &self.indices)`.
    pub fn upload(&self, device: &wgpu::Device) -> crate::Mesh {
        crate::Mesh::new(device, &self.vertices, &self.indices)
    }
}

/// An axis-aligned cube of edge length `size`, centered at the origin (24
/// vertices, 36 indices; one quad per face with outward normals and CCW winding
/// from outside). Tangents follow each face's U texture direction.
pub fn generate_cube_mesh(size: f32) -> MeshData {
    let s = size * 0.5; // half edge
    // (normal, [4 corners CCW from outside]) — corner[0]→[1] is +U, [0]→[3] is +V.
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[s, -s, s], [s, -s, -s], [s, s, -s], [s, s, s]]),
        ([-1.0, 0.0, 0.0], [[-s, -s, -s], [-s, -s, s], [-s, s, s], [-s, s, -s]]),
        ([0.0, 1.0, 0.0], [[-s, s, s], [s, s, s], [s, s, -s], [-s, s, -s]]),
        ([0.0, -1.0, 0.0], [[-s, -s, -s], [s, -s, -s], [s, -s, s], [-s, -s, s]]),
        ([0.0, 0.0, 1.0], [[-s, -s, s], [s, -s, s], [s, s, s], [-s, s, s]]),
        ([0.0, 0.0, -1.0], [[s, -s, -s], [-s, -s, -s], [-s, s, -s], [s, s, -s]]),
    ];
    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    let mut verts = Vec::with_capacity(24);
    for (normal, corners) in faces {
        let tangent = face_tangent(normal, &corners);
        for (corner, uv) in corners.iter().zip(uvs.iter()) {
            verts.push(MeshVertex {
                position: *corner,
                normal,
                uv: *uv,
                tangent,
            });
        }
    }
    let mut indices = Vec::with_capacity(36);
    for face in 0..6u32 {
        let base = face * 4;
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    MeshData { vertices: verts, indices }
}

/// A flat ground quad on the XZ plane (`+Y` normal, CCW from above), spanning
/// `size`×`size` centered at the origin. UVs span 0..1.
pub fn generate_plane_mesh(size: f32) -> MeshData {
    let half = size * 0.5;
    let vertex_at = |x: f32, z: f32, u: f32, v: f32| MeshVertex {
        position: [x, 0.0, z],
        normal: [0.0, 1.0, 0.0],
        uv: [u, v],
        tangent: [1.0, 0.0, 0.0, 1.0],
    };
    let verts = vec![
        vertex_at(-half, -half, 0.0, 0.0),
        vertex_at(half, -half, 1.0, 0.0),
        vertex_at(half, half, 1.0, 1.0),
        vertex_at(-half, half, 0.0, 1.0),
    ];
    let indices = vec![0u32, 2, 1, 0, 3, 2]; // CCW from above (+Y)
    MeshData { vertices: verts, indices }
}

/// A `width`×`height` quad on the XY plane facing `+Z` (CCW from the front),
/// centered at the origin — for walls, sprites-as-geometry, billboards drawn as
/// meshes, etc. UVs span 0..1.
pub fn generate_quad_mesh(width: f32, height: f32) -> MeshData {
    let (half_w, half_h) = (width * 0.5, height * 0.5);
    let vertex_at = |x: f32, y: f32, u: f32, v: f32| MeshVertex {
        position: [x, y, 0.0],
        normal: [0.0, 0.0, 1.0],
        uv: [u, v],
        tangent: [1.0, 0.0, 0.0, 1.0],
    };
    let verts = vec![
        vertex_at(-half_w, -half_h, 0.0, 1.0),
        vertex_at(half_w, -half_h, 1.0, 1.0),
        vertex_at(half_w, half_h, 1.0, 0.0),
        vertex_at(-half_w, half_h, 0.0, 0.0),
    ];
    let indices = vec![0u32, 1, 2, 0, 2, 3];
    MeshData { vertices: verts, indices }
}

/// Old form of [`generate_cube_mesh`] that returns an unnamed
/// `(vertices, indices)` tuple.
#[deprecated(note = "use `generate_cube_mesh`, which returns a named `MeshData`")]
pub fn generate_cube(size: f32) -> (Vec<MeshVertex>, Vec<u32>) {
    let m = generate_cube_mesh(size);
    (m.vertices, m.indices)
}

/// Old form of [`generate_plane_mesh`] that returns an unnamed
/// `(vertices, indices)` tuple.
#[deprecated(note = "use `generate_plane_mesh`, which returns a named `MeshData`")]
pub fn generate_plane(size: f32) -> (Vec<MeshVertex>, Vec<u32>) {
    let m = generate_plane_mesh(size);
    (m.vertices, m.indices)
}

/// Old form of [`generate_quad_mesh`] that returns an unnamed
/// `(vertices, indices)` tuple.
#[deprecated(note = "use `generate_quad_mesh`, which returns a named `MeshData`")]
pub fn generate_quad(width: f32, height: f32) -> (Vec<MeshVertex>, Vec<u32>) {
    let m = generate_quad_mesh(width, height);
    (m.vertices, m.indices)
}

/// Tangent (xyz = +U direction, w = handedness) for a quad face whose corners
/// run `[0]→[1]` along +U and `[0]→[3]` along +V.
fn face_tangent(normal: [f32; 3], corners: &[[f32; 3]; 4]) -> [f32; 4] {
    let u = sub(corners[1], corners[0]);
    let v = sub(corners[3], corners[0]);
    let t = normalize(u);
    // Handedness: w = sign(dot(cross(N, T), V_uv)).
    let nt = cross(normal, t);
    let w = if dot(nt, v) < 0.0 { -1.0 } else { 1.0 };
    [t[0], t[1], t[2], w]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(a: [f32; 3]) -> [f32; 3] {
    let len = dot(a, a).sqrt();
    if len > 1e-6 {
        [a[0] / len, a[1] / len, a[2] / len]
    } else {
        [1.0, 0.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_has_24_verts_36_indices() {
        let cube = generate_cube_mesh(2.0);
        assert_eq!(cube.vertices.len(), 24);
        assert_eq!(cube.indices.len(), 36);
        // Spans -1..1 for size 2.
        assert!(cube.vertices.iter().all(|x| x.position.iter().all(|&c| (c.abs() - 1.0).abs() < 1e-6)));
    }

    #[test]
    fn cube_face_normals_are_unit_and_axis_aligned() {
        let cube = generate_cube_mesh(1.0);
        for vert in &cube.vertices {
            let n = vert.normal;
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-6, "normal not unit: {n:?}");
            // tangent perpendicular to normal.
            let d = n[0] * vert.tangent[0] + n[1] * vert.tangent[1] + n[2] * vert.tangent[2];
            assert!(d.abs() < 1e-5, "tangent not perpendicular to normal");
        }
    }

    #[test]
    fn plane_is_up_facing() {
        let plane = generate_plane_mesh(10.0);
        assert_eq!(plane.vertices.len(), 4);
        assert_eq!(plane.indices.len(), 6);
        assert!(plane.vertices.iter().all(|x| x.normal == [0.0, 1.0, 0.0]));
        assert!(plane.vertices.iter().all(|x| x.position[1] == 0.0));
    }

    #[test]
    fn quad_faces_forward() {
        let quad = generate_quad_mesh(2.0, 4.0);
        assert_eq!(quad.vertices.len(), 4);
        assert_eq!(quad.indices.len(), 6);
        assert!(quad.vertices.iter().all(|x| x.normal == [0.0, 0.0, 1.0]));
    }

    #[test]
    #[allow(deprecated)]
    fn deprecated_tuple_generators_match_mesh_data() {
        fn same(old: (Vec<MeshVertex>, Vec<u32>), new: MeshData) {
            let bytes = |v: &[MeshVertex]| bytemuck::cast_slice::<_, u8>(v).to_vec();
            assert_eq!(bytes(&old.0), bytes(&new.vertices));
            assert_eq!(old.1, new.indices);
        }
        same(generate_cube(1.5), generate_cube_mesh(1.5));
        same(generate_plane(3.0), generate_plane_mesh(3.0));
        same(generate_quad(2.0, 1.0), generate_quad_mesh(2.0, 1.0));
        let sphere = crate::sphere::generate_sphere(0.7, 12, 6);
        same(sphere, crate::generate_sphere_mesh(0.7, 12, 6));
    }
}
