use voltex_math::Vec3;
use crate::light::{LightsUniform, LIGHT_DIRECTIONAL, LIGHT_POINT};

/// A plane in 3D space: normal.dot(point) + d = 0
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32,
}

impl Plane {
    /// Normalize the plane equation so that |normal| == 1.
    pub fn normalize(&self) -> Self {
        let len = self.normal.length();
        if len < 1e-10 {
            return *self;
        }
        Self {
            normal: Vec3::new(self.normal.x / len, self.normal.y / len, self.normal.z / len),
            d: self.d / len,
        }
    }

    /// Signed distance from a point to the plane (positive = inside / front).
    pub fn distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.d
    }
}

/// Six-plane frustum (left, right, bottom, top, near, far).
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    pub planes: [Plane; 6],
}

/// Extract 6 frustum planes from a view-projection matrix using the
/// Gribb-Hartmann method.
///
/// The planes point inward so that a point is inside if distance >= 0 for all planes.
/// Matrix is column-major `[[f32;4];4]` (same as `Mat4::cols`).
pub fn extract_frustum(view_proj: &voltex_math::Mat4) -> Frustum {
    // We work with rows of the VP matrix.
    // For column-major storage cols[c][r]:
    //   row[r] = (cols[0][r], cols[1][r], cols[2][r], cols[3][r])
    let m = &view_proj.cols;

    let row = |r: usize| -> [f32; 4] {
        [m[0][r], m[1][r], m[2][r], m[3][r]]
    };

    let r0 = row(0);
    let r1 = row(1);
    let r2 = row(2);
    let r3 = row(3);

    // Left:   row3 + row0
    let left = Plane {
        normal: Vec3::new(r3[0] + r0[0], r3[1] + r0[1], r3[2] + r0[2]),
        d: r3[3] + r0[3],
    }.normalize();

    // Right:  row3 - row0
    let right = Plane {
        normal: Vec3::new(r3[0] - r0[0], r3[1] - r0[1], r3[2] - r0[2]),
        d: r3[3] - r0[3],
    }.normalize();

    // Bottom: row3 + row1
    let bottom = Plane {
        normal: Vec3::new(r3[0] + r1[0], r3[1] + r1[1], r3[2] + r1[2]),
        d: r3[3] + r1[3],
    }.normalize();

    // Top:    row3 - row1
    let top = Plane {
        normal: Vec3::new(r3[0] - r1[0], r3[1] - r1[1], r3[2] - r1[2]),
        d: r3[3] - r1[3],
    }.normalize();

    // Near:   row2 (wgpu NDC z in [0,1], so near = row2 directly)
    let near = Plane {
        normal: Vec3::new(r2[0], r2[1], r2[2]),
        d: r2[3],
    }.normalize();

    // Far:    row3 - row2
    let far = Plane {
        normal: Vec3::new(r3[0] - r2[0], r3[1] - r2[1], r3[2] - r2[2]),
        d: r3[3] - r2[3],
    }.normalize();

    Frustum {
        planes: [left, right, bottom, top, near, far],
    }
}

/// Test whether a sphere (center, radius) is at least partially inside the frustum.
pub fn sphere_vs_frustum(center: Vec3, radius: f32, frustum: &Frustum) -> bool {
    for plane in &frustum.planes {
        if plane.distance(center) < -radius {
            return false;
        }
    }
    true
}

/// Return indices of lights from `lights` that are visible in the given frustum.
///
/// - Directional lights are always included.
/// - Point lights use a bounding sphere (position, range).
/// - Spot lights use a conservative bounding sphere centered at the light position
///   with radius equal to the light range.
pub fn cull_lights(frustum: &Frustum, lights: &LightsUniform) -> Vec<usize> {
    let count = lights.count as usize;
    let mut visible = Vec::with_capacity(count);

    for i in 0..count {
        let light = &lights.lights[i];
        if light.light_type == LIGHT_DIRECTIONAL {
            // Directional lights affect everything
            visible.push(i);
        } else if light.light_type == LIGHT_POINT {
            let center = Vec3::new(light.position[0], light.position[1], light.position[2]);
            if sphere_vs_frustum(center, light.range, frustum) {
                visible.push(i);
            }
        } else {
            // Spot light — use bounding sphere at position with radius = range
            let center = Vec3::new(light.position[0], light.position[1], light.position[2]);
            if sphere_vs_frustum(center, light.range, frustum) {
                visible.push(i);
            }
        }
    }

    visible
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_math::Mat4;
    use crate::light::{LightData, LightsUniform};

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn test_frustum_extraction_identity() {
        // Identity VP means clip space = NDC directly.
        // For wgpu: x,y in [-1,1], z in [0,1].
        let frustum = extract_frustum(&Mat4::IDENTITY);

        // All 6 planes should be normalized (length ~1)
        for (i, plane) in frustum.planes.iter().enumerate() {
            let len = plane.normal.length();
            assert!(approx_eq(len, 1.0, 1e-4), "Plane {} normal length = {}", i, len);
        }
    }

    #[test]
    fn test_frustum_extraction_perspective() {
        let proj = Mat4::perspective(
            std::f32::consts::FRAC_PI_2, // 90 deg
            1.0,
            0.1,
            100.0,
        );
        let view = Mat4::look_at(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::ZERO,
            Vec3::Y,
        );
        let vp = proj.mul_mat4(&view);
        let frustum = extract_frustum(&vp);

        // Origin (0,0,0) should be inside the frustum (it's 5 units in front of camera)
        assert!(sphere_vs_frustum(Vec3::ZERO, 0.0, &frustum),
            "Origin should be inside frustum");
    }

    #[test]
    fn test_sphere_inside_frustum() {
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let vp = proj.mul_mat4(&view);
        let frustum = extract_frustum(&vp);

        // Sphere at origin with radius 1 — well inside
        assert!(sphere_vs_frustum(Vec3::ZERO, 1.0, &frustum));
    }

    #[test]
    fn test_sphere_outside_frustum() {
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let vp = proj.mul_mat4(&view);
        let frustum = extract_frustum(&vp);

        // Sphere far behind the camera
        assert!(!sphere_vs_frustum(Vec3::new(0.0, 0.0, 200.0), 1.0, &frustum),
            "Sphere far behind camera should be outside");

        // Sphere far to the side
        assert!(!sphere_vs_frustum(Vec3::new(500.0, 0.0, 0.0), 1.0, &frustum),
            "Sphere far to the side should be outside");
    }

    #[test]
    fn test_sphere_partially_inside() {
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let vp = proj.mul_mat4(&view);
        let frustum = extract_frustum(&vp);

        // Sphere at far plane boundary but with large radius should be inside
        assert!(sphere_vs_frustum(Vec3::new(0.0, 0.0, -96.0), 5.0, &frustum));
    }

    #[test]
    fn test_cull_lights_directional_always_included() {
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 50.0);
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let vp = proj.mul_mat4(&view);
        let frustum = extract_frustum(&vp);

        let mut lights = LightsUniform::new();
        lights.add_light(LightData::directional([0.0, -1.0, 0.0], [1.0, 1.0, 1.0], 1.0));
        lights.add_light(LightData::point([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 2.0, 5.0));
        // Point light far away — should be culled
        lights.add_light(LightData::point([500.0, 500.0, 500.0], [0.0, 1.0, 0.0], 2.0, 1.0));

        let visible = cull_lights(&frustum, &lights);
        // Directional (0) always included, point at origin (1) inside, far point (2) culled
        assert!(visible.contains(&0), "Directional light must always be included");
        assert!(visible.contains(&1), "Point light at origin should be visible");
        assert!(!visible.contains(&2), "Far point light should be culled");
    }

    #[test]
    fn test_cull_lights_spot() {
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 50.0);
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let vp = proj.mul_mat4(&view);
        let frustum = extract_frustum(&vp);

        let mut lights = LightsUniform::new();
        // Spot light inside frustum
        lights.add_light(LightData::spot(
            [0.0, 2.0, 0.0], [0.0, -1.0, 0.0], [1.0, 1.0, 1.0], 3.0, 10.0, 15.0, 30.0,
        ));
        // Spot light far away
        lights.add_light(LightData::spot(
            [300.0, 300.0, 300.0], [0.0, -1.0, 0.0], [1.0, 1.0, 1.0], 3.0, 5.0, 15.0, 30.0,
        ));

        let visible = cull_lights(&frustum, &lights);
        assert!(visible.contains(&0), "Near spot should be visible");
        assert!(!visible.contains(&1), "Far spot should be culled");
    }
}
