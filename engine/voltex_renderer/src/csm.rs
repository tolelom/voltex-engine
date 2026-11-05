//! Cascaded shadow maps for the directional light: `CascadedShadowMap` (one depth
//! texture per cascade, 2 cascades), `CsmUniform`, and `compute_cascade_matrices`,
//! which fits each cascade's light matrix.
#![allow(missing_docs)]

use bytemuck::{Pod, Zeroable};
use voltex_math::{Mat4, Vec3, Vec4};

pub const CSM_CASCADE_COUNT: usize = 2;
pub const CSM_MAP_SIZE: u32 = 2048;
pub const CSM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Cascaded Shadow Map with 2 cascades.
pub struct CascadedShadowMap {
    pub textures: [wgpu::Texture; CSM_CASCADE_COUNT],
    pub views: [wgpu::TextureView; CSM_CASCADE_COUNT],
    pub sampler: wgpu::Sampler,
}

impl CascadedShadowMap {
    pub fn new(device: &wgpu::Device) -> Self {
        let create_cascade = |label: &str| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: CSM_MAP_SIZE,
                    height: CSM_MAP_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: CSM_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (texture, view)
        };

        let (t0, v0) = create_cascade("CSM Cascade 0");
        let (t1, v1) = create_cascade("CSM Cascade 1");

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("CSM Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Self {
            textures: [t0, t1],
            views: [v0, v1],
            sampler,
        }
    }
}

/// CSM uniform data: 2 light-view-proj matrices, cascade split distance, shadow params.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CsmUniform {
    pub light_view_proj: [[[f32; 4]; 4]; CSM_CASCADE_COUNT], // 128 bytes
    pub cascade_split: f32, // view-space depth where cascade 0 ends and cascade 1 begins
    pub shadow_map_size: f32,
    pub shadow_bias: f32,
    pub _padding: f32, // 16 bytes total for last row
}

/// Compute the 8 corners of a sub-frustum in world space, given the camera's
/// inverse view-projection and sub-frustum near/far in NDC z [0..1] range.
fn frustum_corners_world(inv_vp: &Mat4, z_near_ndc: f32, z_far_ndc: f32) -> [Vec3; 8] {
    let ndc_corners = [
        // Near plane corners
        Vec4::new(-1.0, -1.0, z_near_ndc, 1.0),
        Vec4::new(1.0, -1.0, z_near_ndc, 1.0),
        Vec4::new(1.0, 1.0, z_near_ndc, 1.0),
        Vec4::new(-1.0, 1.0, z_near_ndc, 1.0),
        // Far plane corners
        Vec4::new(-1.0, -1.0, z_far_ndc, 1.0),
        Vec4::new(1.0, -1.0, z_far_ndc, 1.0),
        Vec4::new(1.0, 1.0, z_far_ndc, 1.0),
        Vec4::new(-1.0, 1.0, z_far_ndc, 1.0),
    ];

    let mut world_corners = [Vec3::ZERO; 8];
    for (i, ndc) in ndc_corners.iter().enumerate() {
        let w = inv_vp.mul_vec4(*ndc);
        world_corners[i] = Vec3::new(w.x / w.w, w.y / w.w, w.z / w.w);
    }
    world_corners
}

/// Compute a tight orthographic light-view-projection matrix for a set of frustum corners.
fn light_matrix_for_corners(light_dir: Vec3, corners: &[Vec3; 8]) -> Mat4 {
    // Build a light-space view matrix looking in the light direction.
    let center = {
        let mut c = Vec3::ZERO;
        for corner in corners {
            c = c + *corner;
        }
        c * (1.0 / 8.0)
    };

    // Pick a stable up vector that isn't parallel to light_dir.
    let up = if light_dir.cross(Vec3::Y).length_squared() < 1e-6 {
        Vec3::Z
    } else {
        Vec3::Y
    };

    let light_view = Mat4::look_at(
        center - light_dir * 0.5, // eye slightly behind center along light direction
        center,
        up,
    );

    // Transform all corners into light view space and find AABB.
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;

    for corner in corners {
        let v = light_view.mul_vec4(Vec4::from_vec3(*corner, 1.0));
        let p = Vec3::new(v.x, v.y, v.z);
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
        min_z = min_z.min(p.z);
        max_z = max_z.max(p.z);
    }

    // Extend the z range to catch shadow casters behind the frustum.
    let z_margin = (max_z - min_z) * 2.0;
    min_z -= z_margin;

    let light_proj = Mat4::orthographic(min_x, max_x, min_y, max_y, -max_z, -min_z);

    light_proj.mul_mat4(&light_view)
}

/// Compute cascade light-view-projection matrices for 2 cascades.
///
/// - `light_dir`: normalized direction **toward** the light source (opposite of light travel).
///   Internally we negate it to get the light travel direction.
/// - `camera_view`, `camera_proj`: the camera's view and projection matrices.
/// - `near`, `far`: camera near/far planes.
/// - `split`: the view-space depth where cascade 0 ends and cascade 1 begins.
///
/// Returns two light-view-projection matrices, one for each cascade.
pub fn compute_cascade_matrices(
    light_dir: Vec3,
    camera_view: &Mat4,
    camera_proj: &Mat4,
    near: f32,
    far: f32,
    split: f32,
) -> [Mat4; CSM_CASCADE_COUNT] {
    let vp = camera_proj.mul_mat4(camera_view);
    let inv_vp = vp.inverse().expect("Camera VP matrix must be invertible");

    // Map view-space depth to NDC z. For wgpu perspective:
    //   ndc_z = (far * (z_view + near)) / (z_view * (far - near))
    // But since z_view is negative in RH, and we want the NDC value, we use
    // the projection matrix directly by projecting (0, 0, -depth, 1).
    let depth_to_ndc = |depth: f32| -> f32 {
        let clip = camera_proj.mul_vec4(Vec4::new(0.0, 0.0, -depth, 1.0));
        clip.z / clip.w
    };

    let ndc_near = depth_to_ndc(near);
    let ndc_split = depth_to_ndc(split);
    let ndc_far = depth_to_ndc(far);

    // Light direction is the direction light travels (away from the source). A
    // zero/degenerate `light_dir` (e.g. an uninitialised light) would normalize
    // to NaN and poison every cascade matrix; fall back to straight-down,
    // matching the runtime's directional_light_vp.
    let dir = {
        let d = (-light_dir).normalize_or_zero();
        if d == Vec3::ZERO {
            Vec3::new(0.0, -1.0, 0.0)
        } else {
            d
        }
    };

    let corners0 = frustum_corners_world(&inv_vp, ndc_near, ndc_split);
    let corners1 = frustum_corners_world(&inv_vp, ndc_split, ndc_far);

    [
        light_matrix_for_corners(dir, &corners0),
        light_matrix_for_corners(dir, &corners1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_csm_uniform_size() {
        // Must be multiple of 16 for WGSL uniform alignment
        assert_eq!(
            mem::size_of::<CsmUniform>() % 16,
            0,
            "CsmUniform must be 16-byte aligned, got {} bytes",
            mem::size_of::<CsmUniform>()
        );
    }

    #[test]
    fn test_compute_cascade_matrices_produces_valid_matrices() {
        let light_dir = Vec3::new(0.0, -1.0, -1.0).normalize();
        let view = Mat4::look_at(Vec3::new(0.0, 5.0, 10.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_4, 16.0 / 9.0, 0.1, 100.0);

        let matrices = compute_cascade_matrices(light_dir, &view, &proj, 0.1, 100.0, 20.0);

        // Both matrices should not be identity (they should be actual projections)
        assert_ne!(
            matrices[0].cols,
            Mat4::IDENTITY.cols,
            "Cascade 0 should not be identity"
        );
        assert_ne!(
            matrices[1].cols,
            Mat4::IDENTITY.cols,
            "Cascade 1 should not be identity"
        );
    }

    #[test]
    fn test_cascade_split_distance() {
        // The split distance should partition the frustum
        let light_dir = Vec3::new(0.0, -1.0, 0.0).normalize();
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_2, 1.0, 1.0, 50.0);

        let split = 15.0;
        let matrices = compute_cascade_matrices(light_dir, &view, &proj, 1.0, 50.0, split);

        // Both matrices should be different (covering different frustum regions)
        let differ = matrices[0]
            .cols
            .iter()
            .zip(matrices[1].cols.iter())
            .any(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| (x - y).abs() > 1e-3));
        assert!(
            differ,
            "Cascade matrices should differ for different frustum regions"
        );
    }

    #[test]
    fn test_frustum_corners_world_identity() {
        // With identity inverse VP, corners should be at NDC positions.
        let inv_vp = Mat4::IDENTITY;
        let corners = frustum_corners_world(&inv_vp, 0.0, 1.0);

        // Near plane at z=0
        assert!((corners[0].z - 0.0).abs() < 1e-5);
        // Far plane at z=1
        assert!((corners[4].z - 1.0).abs() < 1e-5);
    }

    #[test]
    fn compute_cascade_matrices_with_zero_light_dir_is_finite() {
        // A zero light direction (uninitialised light) must not poison the
        // cascade matrices with NaN — it falls back to straight-down.
        let view = Mat4::look_at(Vec3::new(0.0, 5.0, 10.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_4, 16.0 / 9.0, 0.1, 100.0);
        let matrices = compute_cascade_matrices(Vec3::ZERO, &view, &proj, 0.1, 100.0, 20.0);
        for m in &matrices {
            for col in &m.cols {
                for x in col {
                    assert!(x.is_finite(), "cascade matrix must be finite, got {x}");
                }
            }
        }
    }
}
