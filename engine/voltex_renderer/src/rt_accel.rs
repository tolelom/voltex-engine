//! Ray-tracing acceleration structures: builds one BLAS per mesh and a TLAS
//! over [`RtInstance`] transforms for the hardware ray-query passes.

#![allow(missing_docs)]

use crate::vertex::MeshVertex;

/// Data needed to build a BLAS for one mesh.
pub struct BlasMeshData<'a> {
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub vertex_count: u32,
    pub index_count: u32,
}

/// One instance transform fed to the TLAS: a world transform and a BLAS index.
pub struct RtInstance {
    /// Column-major 4x4 transform matrix.
    pub transform: [f32; 16],
    /// Index into `RtAccel::blas_list`.
    pub blas_index: usize,
}

/// Bottom + Top Level Acceleration Structures for a scene.
pub struct RtAccel {
    pub blas_list: Vec<wgpu::Blas>,
    pub tlas: wgpu::Tlas,
}

impl RtAccel {
    /// Create BLAS for each mesh and build a TLAS with the given instances.
    ///
    /// The encoder must be submitted after this call so the GPU builds fire.
    pub fn new(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        meshes: &[BlasMeshData<'_>],
        instances: &[RtInstance],
    ) -> Self {
        let vertex_stride = std::mem::size_of::<MeshVertex>() as u64;

        // ── Build one BLAS per mesh ───────────────────────────────────────────
        let size_descs: Vec<wgpu::BlasTriangleGeometrySizeDescriptor> = meshes
            .iter()
            .map(|m| wgpu::BlasTriangleGeometrySizeDescriptor {
                vertex_format: wgpu::VertexFormat::Float32x3,
                vertex_count: m.vertex_count,
                index_format: Some(wgpu::IndexFormat::Uint32),
                index_count: Some(m.index_count),
                flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
            })
            .collect();

        let blas_list: Vec<wgpu::Blas> = meshes
            .iter()
            .zip(size_descs.iter())
            .map(|(_mesh, size_desc)| {
                device.create_blas(
                    &wgpu::CreateBlasDescriptor {
                        label: Some("Mesh BLAS"),
                        flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                        update_mode: wgpu::AccelerationStructureUpdateMode::Build,
                    },
                    wgpu::BlasGeometrySizeDescriptors::Triangles {
                        descriptors: vec![size_desc.clone()],
                    },
                )
            })
            .collect();

        // ── Create TLAS ───────────────────────────────────────────────────────
        let max_instances = instances.len().max(1) as u32;
        let mut tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("Scene TLAS"),
            max_instances,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });

        // ── Populate TLAS instances ───────────────────────────────────────────
        for (i, inst) in instances.iter().enumerate() {
            let blas = &blas_list[inst.blas_index];
            tlas[i] = Some(wgpu::TlasInstance::new(
                blas,
                mat4_to_tlas_transform(&inst.transform),
                0,
                0xFF,
            ));
        }

        // ── Build entries ─────────────────────────────────────────────────────
        let blas_entries: Vec<wgpu::BlasBuildEntry<'_>> = meshes
            .iter()
            .zip(blas_list.iter())
            .zip(size_descs.iter())
            .map(|((mesh, blas), size_desc)| wgpu::BlasBuildEntry {
                blas,
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![
                    wgpu::BlasTriangleGeometry {
                        size: size_desc,
                        vertex_buffer: mesh.vertex_buffer,
                        first_vertex: 0,
                        vertex_stride,
                        index_buffer: Some(mesh.index_buffer),
                        first_index: Some(0),
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    },
                ]),
            })
            .collect();

        encoder.build_acceleration_structures(blas_entries.iter(), std::iter::once(&tlas));

        Self { blas_list, tlas }
    }

    /// Update TLAS instance transforms and rebuild.
    pub fn update_instances(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        instances: &[RtInstance],
    ) {
        for (i, inst) in instances.iter().enumerate() {
            if i >= self.tlas.get().len() {
                break;
            }
            let blas = &self.blas_list[inst.blas_index];
            self.tlas[i] = Some(wgpu::TlasInstance::new(
                blas,
                mat4_to_tlas_transform(&inst.transform),
                0,
                0xFF,
            ));
        }
        encoder.build_acceleration_structures(std::iter::empty(), std::iter::once(&self.tlas));
    }
}

/// Convert a column-major 4×4 matrix to a row-major 3×4 affine transform.
///
/// The TLAS expects `[f32; 12]` in row-major order (3 rows × 4 columns).
/// A standard column-major mat4 stores: `col0 = (m0, m1, m2, m3)`,
/// `col1 = (m4, m5, m6, m7)`, ...
///
/// - Row 0: `m\[0\], m\[4\], m\[8\],  m\[12\]`
/// - Row 1: `m\[1\], m\[5\], m\[9\],  m\[13\]`
/// - Row 2: `m\[2\], m\[6\], m\[10\], m\[14\]`
pub fn mat4_to_tlas_transform(m: &[f32; 16]) -> [f32; 12] {
    [
        m[0], m[4], m[8], m[12], m[1], m[5], m[9], m[13], m[2], m[6], m[10], m[14],
    ]
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mat4_to_tlas_transform_identity() {
        #[rustfmt::skip]
        let identity: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        let result = mat4_to_tlas_transform(&identity);
        // Row 0: [1,0,0,0], Row 1: [0,1,0,0], Row 2: [0,0,1,0]
        let expected: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_mat4_to_tlas_transform_translation() {
        // Column-major: identity rotation + translation (tx=1, ty=2, tz=3)
        // Column 3 = [tx, ty, tz, 1] stored at indices [12,13,14,15]
        #[rustfmt::skip]
        let mat: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,   // col 0
            0.0, 1.0, 0.0, 0.0,   // col 1
            0.0, 0.0, 1.0, 0.0,   // col 2
            1.0, 2.0, 3.0, 1.0,   // col 3 (translation)
        ];
        let result = mat4_to_tlas_transform(&mat);
        // Row-major 3x4: rotation part identity, translation in last column
        let expected: [f32; 12] = [1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 1.0, 3.0];
        assert_eq!(result, expected);
    }
}
