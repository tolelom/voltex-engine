use voltex_math::{Mat4, Vec3};

pub const POINT_SHADOW_SIZE: u32 = 512;
pub const POINT_SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Depth cube map for omnidirectional point light shadows.
///
/// Uses a single cube texture with 6 faces (512x512 each).
pub struct PointShadowMap {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    /// Per-face views for rendering into each cube face.
    pub face_views: [wgpu::TextureView; 6],
    pub sampler: wgpu::Sampler,
}

impl PointShadowMap {
    pub fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Point Shadow Cube Texture"),
            size: wgpu::Extent3d {
                width: POINT_SHADOW_SIZE,
                height: POINT_SHADOW_SIZE,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: POINT_SHADOW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // Full cube view for sampling in shader
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Point Shadow Cube View"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        // Per-face views for rendering
        let face_labels = ["+X", "-X", "+Y", "-Y", "+Z", "-Z"];
        let face_views = std::array::from_fn(|i| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("Point Shadow Face {}", face_labels[i])),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Point Shadow Sampler"),
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
            face_views,
            sampler,
        }
    }
}

/// Compute the 6 view matrices for rendering into a point light shadow cube map.
///
/// Order: +X, -X, +Y, -Y, +Z, -Z (matching wgpu cube face order).
///
/// Each matrix is a look-at view matrix from `light_pos` toward the
/// corresponding axis direction, with an appropriate up vector.
pub fn point_shadow_view_matrices(light_pos: Vec3) -> [Mat4; 6] {
    [
        // +X: look right
        Mat4::look_at(light_pos, light_pos + Vec3::X, -Vec3::Y),
        // -X: look left
        Mat4::look_at(light_pos, light_pos - Vec3::X, -Vec3::Y),
        // +Y: look up
        Mat4::look_at(light_pos, light_pos + Vec3::Y, Vec3::Z),
        // -Y: look down
        Mat4::look_at(light_pos, light_pos - Vec3::Y, -Vec3::Z),
        // +Z: look forward
        Mat4::look_at(light_pos, light_pos + Vec3::Z, -Vec3::Y),
        // -Z: look backward
        Mat4::look_at(light_pos, light_pos - Vec3::Z, -Vec3::Y),
    ]
}

/// Compute the perspective projection for point light shadow rendering.
///
/// 90 degree FOV, 1:1 aspect ratio.
/// `near` and `far` control the shadow range (typically 0.1 and light.range).
pub fn point_shadow_projection(near: f32, far: f32) -> Mat4 {
    Mat4::perspective(std::f32::consts::FRAC_PI_2, 1.0, near, far)
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_math::Vec4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn test_point_shadow_view_matrices_count() {
        let views = point_shadow_view_matrices(Vec3::ZERO);
        assert_eq!(views.len(), 6);
    }

    #[test]
    fn test_point_shadow_view_directions() {
        let pos = Vec3::new(0.0, 0.0, 0.0);
        let views = point_shadow_view_matrices(pos);

        // For the +X face, a point at (1, 0, 0) should map to the center of the view
        // (i.e., to (0, 0, -1) in view space, roughly).
        let test_point = Vec4::new(1.0, 0.0, 0.0, 1.0);
        let in_view = views[0].mul_vec4(test_point);
        // z should be negative (in front of camera)
        assert!(in_view.z < 0.0, "+X face: point at +X should be in front, got z={}", in_view.z);
        // x and y should be near 0 (centered)
        assert!(approx_eq(in_view.x, 0.0), "+X face: expected x~0, got {}", in_view.x);
        assert!(approx_eq(in_view.y, 0.0), "+X face: expected y~0, got {}", in_view.y);

        // For the -X face, a point at (-1, 0, 0) should be in front
        let test_neg_x = Vec4::new(-1.0, 0.0, 0.0, 1.0);
        let in_view_neg = views[1].mul_vec4(test_neg_x);
        assert!(in_view_neg.z < 0.0, "-X face: point at -X should be in front, got z={}", in_view_neg.z);
    }

    #[test]
    fn test_point_shadow_view_offset_position() {
        let pos = Vec3::new(5.0, 10.0, -3.0);
        let views = point_shadow_view_matrices(pos);

        // Origin of the light should map to (0,0,0) in view space
        let origin = Vec4::from_vec3(pos, 1.0);
        for (i, view) in views.iter().enumerate() {
            let v = view.mul_vec4(origin);
            assert!(approx_eq(v.x, 0.0), "Face {}: origin x should be 0, got {}", i, v.x);
            assert!(approx_eq(v.y, 0.0), "Face {}: origin y should be 0, got {}", i, v.y);
            assert!(approx_eq(v.z, 0.0), "Face {}: origin z should be 0, got {}", i, v.z);
        }
    }

    #[test]
    fn test_point_shadow_projection_90fov() {
        let proj = point_shadow_projection(0.1, 100.0);

        // At 90 degree FOV with aspect 1:1, a point at (-near, 0, -near) in view space
        // should project to the edge of the viewport.
        let near = 0.1f32;
        let edge = Vec4::new(near, 0.0, -near, 1.0);
        let clip = proj.mul_vec4(edge);
        let ndc_x = clip.x / clip.w;
        // Should be at x=1.0 (right edge)
        assert!(approx_eq(ndc_x, 1.0), "Expected NDC x=1.0, got {}", ndc_x);
    }

    #[test]
    fn test_point_shadow_projection_near_plane() {
        let proj = point_shadow_projection(0.1, 100.0);
        let p = Vec4::new(0.0, 0.0, -0.1, 1.0);
        let clip = proj.mul_vec4(p);
        let ndc_z = clip.z / clip.w;
        assert!(approx_eq(ndc_z, 0.0), "Near plane should map to NDC z=0, got {}", ndc_z);
    }
}
