#![allow(missing_docs)]

use crate::shadow::{SHADOW_FORMAT, SHADOW_MAP_SIZE};
use voltex_math::{Mat4, Vec3};

/// Shadow map for a single spot light. Reuses the same texture format and
/// resolution as the directional `ShadowMap`.
pub struct SpotShadowMap {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl SpotShadowMap {
    pub fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Spot Shadow Map Texture"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Spot Shadow Sampler"),
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
            texture,
            view,
            sampler,
        }
    }
}

/// Compute the view-projection matrix for a spot light shadow pass.
///
/// - `position`: world-space position of the spot light
/// - `direction`: normalized direction the spot light points toward
/// - `outer_angle`: outer cone half-angle in **radians**
/// - `range`: maximum distance the spot light reaches
///
/// The projection uses a perspective matrix with FOV = 2 * outer_angle,
/// 1:1 aspect ratio, near = 0.1, far = range.
pub fn spot_shadow_matrix(position: Vec3, direction: Vec3, outer_angle: f32, range: f32) -> Mat4 {
    // A zero/degenerate direction would normalize to NaN and poison the view
    // matrix; fall back to straight-down so the shadow stays finite.
    let dir = {
        let d = direction.normalize_or_zero();
        if d == Vec3::ZERO {
            Vec3::new(0.0, -1.0, 0.0)
        } else {
            d
        }
    };

    // Pick a stable up vector that isn't parallel to the light direction.
    let up = if dir.cross(Vec3::Y).length_squared() < 1e-6 {
        Vec3::Z
    } else {
        Vec3::Y
    };

    let target = position + dir;
    let view = Mat4::look_at(position, target, up);

    // FOV = 2 * outer_angle; clamped to avoid degenerate projections.
    let fov = (2.0 * outer_angle).min(std::f32::consts::PI - 0.01);
    let near = 0.1_f32;
    let far = range.max(near + 0.1);
    let proj = Mat4::perspective(fov, 1.0, near, far);

    proj.mul_mat4(&view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_math::Vec4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn test_spot_shadow_matrix_center() {
        let pos = Vec3::new(0.0, 10.0, 0.0);
        let dir = Vec3::new(0.0, -1.0, 0.0);
        let outer_angle = 30.0_f32.to_radians();
        let range = 20.0;

        let vp = spot_shadow_matrix(pos, dir, outer_angle, range);

        // A point directly below the light (along the direction) should map near center of NDC.
        let test_point = Vec4::new(0.0, 5.0, 0.0, 1.0); // 5 units below the light
        let clip = vp.mul_vec4(test_point);
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;

        assert!(
            approx_eq(ndc_x, 0.0),
            "Center point should have NDC x~0, got {}",
            ndc_x
        );
        // y may not be exactly 0 due to the look_at up vector choice, but should be close
        assert!(
            ndc_y.abs() < 0.5,
            "Center point should be near NDC center, got y={}",
            ndc_y
        );
    }

    #[test]
    fn test_spot_shadow_matrix_depth_range() {
        let pos = Vec3::new(0.0, 0.0, 0.0);
        let dir = Vec3::new(0.0, 0.0, -1.0);
        let outer_angle = 45.0_f32.to_radians();
        let range = 50.0;

        let vp = spot_shadow_matrix(pos, dir, outer_angle, range);

        // A point at near distance should have NDC z ~ 0
        let near_point = Vec4::new(0.0, 0.0, -0.1, 1.0);
        let clip_near = vp.mul_vec4(near_point);
        let ndc_z_near = clip_near.z / clip_near.w;
        assert!(
            (-0.1..=0.2).contains(&ndc_z_near),
            "Near point NDC z should be ~0, got {}",
            ndc_z_near
        );

        // A point at far distance should have NDC z ~ 1
        let far_point = Vec4::new(0.0, 0.0, -50.0, 1.0);
        let clip_far = vp.mul_vec4(far_point);
        let ndc_z_far = clip_far.z / clip_far.w;
        assert!(
            ndc_z_far > 0.9 && ndc_z_far <= 1.01,
            "Far point NDC z should be ~1, got {}",
            ndc_z_far
        );
    }

    #[test]
    fn test_spot_shadow_matrix_not_identity() {
        let pos = Vec3::new(5.0, 5.0, 5.0);
        let dir = Vec3::new(-1.0, -1.0, -1.0).normalize();
        let vp = spot_shadow_matrix(pos, dir, 25.0_f32.to_radians(), 30.0);
        assert_ne!(vp.cols, Mat4::IDENTITY.cols);
    }

    #[test]
    fn test_spot_shadow_matrix_direction_down() {
        // Light pointing straight down should work (uses Z as up instead of Y)
        let pos = Vec3::new(0.0, 10.0, 0.0);
        let dir = Vec3::new(0.0, -1.0, 0.0);
        let vp = spot_shadow_matrix(pos, dir, 30.0_f32.to_radians(), 15.0);
        // Should not panic; the matrix should be valid
        assert_ne!(vp.cols, Mat4::IDENTITY.cols);
    }

    #[test]
    fn spot_shadow_matrix_with_zero_direction_is_finite() {
        // A zero direction must fall back to straight-down rather than NaN.
        let vp = spot_shadow_matrix(Vec3::new(0.0, 10.0, 0.0), Vec3::ZERO, 30.0_f32.to_radians(), 15.0);
        for col in &vp.cols {
            for x in col {
                assert!(x.is_finite(), "spot shadow matrix must be finite, got {x}");
            }
        }
    }
}
