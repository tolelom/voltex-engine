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

#[test]
fn deserialize_navmesh_rejects_huge_vertex_count() {
    // Adversarial vertex count u32::MAX. Without the cap this drove a
    // multi-GB `Vec::with_capacity` allocation.
    let mut data = Vec::new();
    data.extend_from_slice(&u32::MAX.to_le_bytes());
    let err = deserialize_navmesh(&data).err().expect("expected error").to_string();
    assert!(err.contains("vertex count"), "unexpected error: {err}");
}

#[test]
fn deserialize_navmesh_rejects_triangle_vertex_index_oob() {
    // One vertex declared, triangle references vertex 5 → out of range.
    // Without the check this allowed building a NavMesh whose
    // `shared_edge` / `point_in_triangle_xz` would panic on
    // `self.vertices[5]`.
    let mut data = Vec::new();
    // vertex_count = 1, one vertex at (0,0,0)
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&0.0f32.to_le_bytes());
    data.extend_from_slice(&0.0f32.to_le_bytes());
    data.extend_from_slice(&0.0f32.to_le_bytes());
    // triangle_count = 1, triangle (5,5,5) with no neighbours
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&5u32.to_le_bytes());
    data.extend_from_slice(&5u32.to_le_bytes());
    data.extend_from_slice(&5u32.to_le_bytes());
    data.extend_from_slice(&(-1i32).to_le_bytes());
    data.extend_from_slice(&(-1i32).to_le_bytes());
    data.extend_from_slice(&(-1i32).to_le_bytes());
    let err = deserialize_navmesh(&data).err().expect("expected error").to_string();
    assert!(err.contains("vertex index out of range"), "unexpected error: {err}");
}

#[test]
fn deserialize_navmesh_rejects_neighbour_index_oob() {
    // One valid triangle but neighbour index = 5 (only 1 triangle exists).
    let mut data = Vec::new();
    // vertex_count = 3, simple triangle vertices
    data.extend_from_slice(&3u32.to_le_bytes());
    for _ in 0..9 {
        data.extend_from_slice(&0.0f32.to_le_bytes());
    }
    // triangle_count = 1, valid indices but neighbour = 5
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&5i32.to_le_bytes());
    data.extend_from_slice(&(-1i32).to_le_bytes());
    data.extend_from_slice(&(-1i32).to_le_bytes());
    let err = deserialize_navmesh(&data).err().expect("expected error").to_string();
    assert!(err.contains("neighbour triangle index"), "unexpected error: {err}");
}
