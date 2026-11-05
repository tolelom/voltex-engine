//! Built-in primitive mesh generators — the shapes every game reaches for.
//! Each returns `(vertices, indices)` as `(Vec<MeshVertex>, Vec<u32>)` (like
//! [`generate_sphere`](crate::generate_sphere)), ready for
//! [`Mesh::new`](crate::Mesh::new). Vertices carry correct per-face
//! normals, UVs, and tangents so they work with both the colored and PBR
//! (normal-mapped) paths.

use voltex_image::MeshVertex;

/// An axis-aligned cube of edge length `size`, centered at the origin (24
/// vertices, 36 indices; one quad per face with outward normals and CCW winding
/// from outside). Tangents follow each face's U texture direction.
pub fn generate_cube(size: f32) -> (Vec<MeshVertex>, Vec<u32>) {
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
    (verts, indices)
}

/// A flat ground quad on the XZ plane (`+Y` normal, CCW from above), spanning
/// `size`×`size` centered at the origin. UVs span 0..1.
pub fn generate_plane(size: f32) -> (Vec<MeshVertex>, Vec<u32>) {
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
    (verts, indices)
}

/// A `width`×`height` quad on the XY plane facing `+Z` (CCW from the front),
/// centered at the origin — for walls, sprites-as-geometry, billboards drawn as
/// meshes, etc. UVs span 0..1.
pub fn generate_quad(width: f32, height: f32) -> (Vec<MeshVertex>, Vec<u32>) {
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
    (verts, indices)
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
        let (v, i) = generate_cube(2.0);
        assert_eq!(v.len(), 24);
        assert_eq!(i.len(), 36);
        // Spans -1..1 for size 2.
        assert!(v.iter().all(|x| x.position.iter().all(|&c| (c.abs() - 1.0).abs() < 1e-6)));
    }

    #[test]
    fn cube_face_normals_are_unit_and_axis_aligned() {
        let (v, _) = generate_cube(1.0);
        for vert in &v {
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
        let (v, i) = generate_plane(10.0);
        assert_eq!(v.len(), 4);
        assert_eq!(i.len(), 6);
        assert!(v.iter().all(|x| x.normal == [0.0, 1.0, 0.0]));
        assert!(v.iter().all(|x| x.position[1] == 0.0));
    }

    #[test]
    fn quad_faces_forward() {
        let (v, i) = generate_quad(2.0, 4.0);
        assert_eq!(v.len(), 4);
        assert_eq!(i.len(), 6);
        assert!(v.iter().all(|x| x.normal == [0.0, 0.0, 1.0]));
    }
}
