//! Procedural UV sphere generator — primary use is PBR demo + IBL preview.

use crate::vertex::MeshVertex;
use std::f32::consts::PI;

/// Generate a UV sphere with Y-up coordinate system.
/// Returns (vertices, indices).
pub fn generate_sphere(radius: f32, sectors: u32, stacks: u32) -> (Vec<MeshVertex>, Vec<u32>) {
    let mut vertices: Vec<MeshVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let sector_step = 2.0 * PI / sectors as f32;
    let stack_step = PI / stacks as f32;

    for i in 0..=stacks {
        // Stack angle from PI/2 (top) to -PI/2 (bottom)
        let stack_angle = PI / 2.0 - (i as f32) * stack_step;
        let xz = radius * stack_angle.cos();
        let y = radius * stack_angle.sin();

        for j in 0..=sectors {
            let sector_angle = (j as f32) * sector_step;

            let x = xz * sector_angle.cos();
            let z = xz * sector_angle.sin();

            let position = [x, y, z];
            let normal = [x / radius, y / radius, z / radius];
            let uv = [
                j as f32 / sectors as f32,
                i as f32 / stacks as f32,
            ];

            // Tangent follows the longitude direction (increasing sector angle) in XZ plane
            let tangent_x = -sector_angle.sin();
            let tangent_z = sector_angle.cos();

            vertices.push(MeshVertex {
                position,
                normal,
                uv,
                tangent: [tangent_x, 0.0, tangent_z, 1.0],
            });
        }
    }

    // Indices: two triangles per quad
    for i in 0..stacks {
        for j in 0..sectors {
            let k1 = i * (sectors + 1) + j;
            let k2 = k1 + sectors + 1;

            // First triangle
            indices.push(k1);
            indices.push(k2);
            indices.push(k1 + 1);

            // Second triangle
            indices.push(k1 + 1);
            indices.push(k2);
            indices.push(k2 + 1);
        }
    }

    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_vertex_count() {
        let sectors = 36u32;
        let stacks = 18u32;
        let (vertices, _) = generate_sphere(1.0, sectors, stacks);
        assert_eq!(vertices.len(), ((stacks + 1) * (sectors + 1)) as usize);
    }

    #[test]
    fn test_sphere_index_count() {
        let sectors = 36u32;
        let stacks = 18u32;
        let (_, indices) = generate_sphere(1.0, sectors, stacks);
        assert_eq!(indices.len(), (stacks * sectors * 6) as usize);
    }

    #[test]
    fn test_sphere_normals_unit_length() {
        let (vertices, _) = generate_sphere(1.0, 12, 8);
        for v in &vertices {
            let n = v.normal;
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!(
                (len - 1.0).abs() < 1e-5,
                "Normal length {} is not unit length",
                len
            );
        }
    }
}
