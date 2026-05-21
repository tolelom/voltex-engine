//! Triangle mesh collider — list of vertices + index triples. Supports
//! ray-vs-mesh queries (closest hit and all hits).

use voltex_math::{Vec3, Ray};
use crate::ray::ray_vs_triangle;

/// A triangle mesh collider defined by vertices and triangle indices.
#[derive(Debug, Clone)]
pub struct MeshCollider {
    /// Vertex positions in local space.
    pub vertices: Vec<Vec3>,
    /// Triangle index triples into [`vertices`](Self::vertices).
    pub indices: Vec<[u32; 3]>,
}

/// Result of a ray-mesh intersection test.
#[derive(Debug, Clone, Copy)]
pub struct MeshHit {
    /// Distance along the ray to the hit (in ray units).
    pub distance: f32,
    /// Hit point in world space.
    pub point: Vec3,
    /// Triangle normal at the hit point.
    pub normal: Vec3,
    /// Index of the hit triangle in `MeshCollider::indices`.
    pub triangle_index: usize,
}

/// Cast a ray against a triangle mesh. Returns the closest hit, if any.
pub fn ray_vs_mesh(ray: &Ray, mesh: &MeshCollider) -> Option<MeshHit> {
    let mut closest: Option<MeshHit> = None;

    for (i, tri) in mesh.indices.iter().enumerate() {
        let v0 = mesh.vertices[tri[0] as usize];
        let v1 = mesh.vertices[tri[1] as usize];
        let v2 = mesh.vertices[tri[2] as usize];

        if let Some((t, normal)) = ray_vs_triangle(ray, v0, v1, v2) {
            let is_closer = closest.as_ref().is_none_or(|c| t < c.distance);
            if is_closer {
                let point = ray.at(t);
                closest = Some(MeshHit {
                    distance: t,
                    point,
                    normal,
                    triangle_index: i,
                });
            }
        }
    }

    closest
}

/// Cast a ray against a triangle mesh. Returns all hits sorted by distance.
pub fn ray_vs_mesh_all(ray: &Ray, mesh: &MeshCollider) -> Vec<MeshHit> {
    let mut hits = Vec::new();

    for (i, tri) in mesh.indices.iter().enumerate() {
        let v0 = mesh.vertices[tri[0] as usize];
        let v1 = mesh.vertices[tri[1] as usize];
        let v2 = mesh.vertices[tri[2] as usize];

        if let Some((t, normal)) = ray_vs_triangle(ray, v0, v1, v2) {
            let point = ray.at(t);
            hits.push(MeshHit {
                distance: t,
                point,
                normal,
                triangle_index: i,
            });
        }
    }

    hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ray_vs_mesh_hit() {
        // Simple quad (2 triangles)
        let mesh = MeshCollider {
            vertices: vec![
                Vec3::new(-1.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(-1.0, 0.0, 1.0),
            ],
            indices: vec![[0, 1, 2], [0, 2, 3]],
        };
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let hit = ray_vs_mesh(&ray, &mesh);
        assert!(hit.is_some());
        assert!((hit.unwrap().distance - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_ray_vs_mesh_miss() {
        let mesh = MeshCollider {
            vertices: vec![
                Vec3::new(-1.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            indices: vec![[0, 1, 2]],
        };
        let ray = Ray::new(Vec3::new(5.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        assert!(ray_vs_mesh(&ray, &mesh).is_none());
    }

    #[test]
    fn test_ray_vs_mesh_closest() {
        // Two triangles at different heights
        let mesh = MeshCollider {
            vertices: vec![
                Vec3::new(-1.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(-1.0, 2.0, -1.0),
                Vec3::new(1.0, 2.0, -1.0),
                Vec3::new(0.0, 2.0, 1.0),
            ],
            indices: vec![[0, 1, 2], [3, 4, 5]],
        };
        let ray = Ray::new(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let hit = ray_vs_mesh(&ray, &mesh).unwrap();
        assert!((hit.distance - 3.0).abs() < 0.01); // hits y=2 first
    }

    #[test]
    fn test_ray_vs_mesh_all() {
        let mesh = MeshCollider {
            vertices: vec![
                Vec3::new(-1.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(-1.0, 2.0, -1.0),
                Vec3::new(1.0, 2.0, -1.0),
                Vec3::new(0.0, 2.0, 1.0),
            ],
            indices: vec![[0, 1, 2], [3, 4, 5]],
        };
        let ray = Ray::new(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let hits = ray_vs_mesh_all(&ray, &mesh);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].distance < hits[1].distance); // sorted
    }
}
