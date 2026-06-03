//! GPU light + camera uniforms. Includes the multi-light array
//! ([`LightsUniform`] / [`LightData`]) used by the PBR pipeline.
#![allow(missing_docs)]

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _padding: f32,
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self::new()
    }
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            camera_pos: [0.0; 3],
            _padding: 0.0,
        }
    }
}

/// Per-entity uniform for the [`BasicRenderer`](../../voltex_renderer_ecs) path:
/// [`CameraUniform`] plus a `base_color` tint so meshes can be colored without
/// a texture (the Blinn-Phong `mesh_shader` samples only a white default
/// texture, leaving every object the same gray). Layout matches
/// `mesh_colored.wgsl`'s `CameraUniform` (std140: `base_color` at offset 144).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ColoredCameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _padding: f32,
    pub base_color: [f32; 4],
}

impl Default for ColoredCameraUniform {
    fn default() -> Self {
        Self::new()
    }
}

impl ColoredCameraUniform {
    pub fn new() -> Self {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self {
            view_proj: id,
            model: id,
            camera_pos: [0.0; 3],
            _padding: 0.0,
            base_color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightUniform {
    pub direction: [f32; 3],
    pub _padding1: f32,
    pub color: [f32; 3],
    pub ambient_strength: f32,
}

impl Default for LightUniform {
    fn default() -> Self {
        Self::new()
    }
}

impl LightUniform {
    pub fn new() -> Self {
        Self {
            direction: [0.0, -1.0, -1.0],
            _padding1: 0.0,
            color: [1.0, 1.0, 1.0],
            ambient_strength: 0.1,
        }
    }
}

/// Max additive point lights in the `BasicRenderer` Blinn-Phong path.
pub const MAX_POINT_LIGHTS: usize = 8;

/// One point light for the `BasicRenderer` path (`mesh_colored.wgsl` group 2).
/// 32 bytes / std140-friendly (vec3+f32, vec3+f32).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct PointLight {
    /// World-space position.
    pub position: [f32; 3],
    /// Falloff radius (attenuation reaches 0 at `range`).
    pub range: f32,
    /// RGB color.
    pub color: [f32; 3],
    /// Brightness multiplier.
    pub intensity: f32,
}

impl PointLight {
    /// A point light at `position` with `color` and `intensity`, reaching to `range`.
    pub fn new(position: [f32; 3], color: [f32; 3], range: f32, intensity: f32) -> Self {
        Self { position, range, color, intensity }
    }
}

/// GPU uniform: a fixed array of [`PointLight`] + active `count`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct PointLightsUniform {
    /// Light slots; only the first `count` are active.
    pub lights: [PointLight; MAX_POINT_LIGHTS],
    /// Number of active lights (`0..=MAX_POINT_LIGHTS`).
    pub count: u32,
    pub _pad: [u32; 3],
}

impl Default for PointLightsUniform {
    fn default() -> Self {
        Self::new()
    }
}

impl PointLightsUniform {
    /// All-zero, `count = 0` (no point lights).
    pub fn new() -> Self {
        Self {
            lights: [PointLight { position: [0.0; 3], range: 0.0, color: [0.0; 3], intensity: 0.0 };
                MAX_POINT_LIGHTS],
            count: 0,
            _pad: [0; 3],
        }
    }

    /// Build from a slice, clamping to [`MAX_POINT_LIGHTS`].
    pub fn from_slice(src: &[PointLight]) -> Self {
        let mut u = Self::new();
        let n = src.len().min(MAX_POINT_LIGHTS);
        u.lights[..n].copy_from_slice(&src[..n]);
        u.count = n as u32;
        u
    }
}

// Multi-light support
pub const MAX_LIGHTS: usize = 16;
pub const LIGHT_DIRECTIONAL: u32 = 0;
pub const LIGHT_POINT: u32 = 1;
pub const LIGHT_SPOT: u32 = 2;

/// Per-light data. Must be exactly 64 bytes (4 × vec4) for WGSL array alignment.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightData {
    pub position: [f32; 3],
    pub light_type: u32, // 16 bytes
    pub direction: [f32; 3],
    pub range: f32, // 32 bytes
    pub color: [f32; 3],
    pub intensity: f32, // 48 bytes
    pub inner_cone: f32,
    pub outer_cone: f32,
    pub _padding: [f32; 2], // 64 bytes
}

impl LightData {
    pub fn directional(direction: [f32; 3], color: [f32; 3], intensity: f32) -> Self {
        Self {
            position: [0.0; 3],
            light_type: LIGHT_DIRECTIONAL,
            direction,
            range: 0.0,
            color,
            intensity,
            inner_cone: 0.0,
            outer_cone: 0.0,
            _padding: [0.0; 2],
        }
    }

    pub fn point(position: [f32; 3], color: [f32; 3], intensity: f32, range: f32) -> Self {
        Self {
            position,
            light_type: LIGHT_POINT,
            direction: [0.0; 3],
            range,
            color,
            intensity,
            inner_cone: 0.0,
            outer_cone: 0.0,
            _padding: [0.0; 2],
        }
    }

    pub fn spot(
        position: [f32; 3],
        direction: [f32; 3],
        color: [f32; 3],
        intensity: f32,
        range: f32,
        inner_angle_deg: f32,
        outer_angle_deg: f32,
    ) -> Self {
        Self {
            position,
            light_type: LIGHT_SPOT,
            direction,
            range,
            color,
            intensity,
            inner_cone: inner_angle_deg.to_radians().cos(),
            outer_cone: outer_angle_deg.to_radians().cos(),
            _padding: [0.0; 2],
        }
    }
}

/// Uniform buffer holding up to MAX_LIGHTS lights plus ambient color.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightsUniform {
    pub lights: [LightData; MAX_LIGHTS], // 1024 bytes
    pub count: u32,                      // 4 bytes
    pub _pad_count: [f32; 3],            // 12 bytes (align ambient_color to 16-byte for WGSL vec3)
    pub ambient_color: [f32; 3],         // 12 bytes at offset 1040
    pub _pad_end: f32,                   // 4 bytes → total 1056 (matches WGSL)
}

impl Default for LightsUniform {
    fn default() -> Self {
        Self::new()
    }
}

impl LightsUniform {
    pub fn new() -> Self {
        Self {
            lights: [LightData::zeroed(); MAX_LIGHTS],
            count: 0,
            _pad_count: [0.0; 3],
            ambient_color: [0.03, 0.03, 0.03],
            _pad_end: 0.0,
        }
    }

    pub fn add_light(&mut self, light: LightData) {
        if (self.count as usize) < MAX_LIGHTS {
            self.lights[self.count as usize] = light;
            self.count += 1;
        }
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_light_data_size() {
        assert_eq!(
            mem::size_of::<LightData>() % 16,
            0,
            "LightData must be a multiple of 16 bytes for WGSL array alignment"
        );
        assert_eq!(
            mem::size_of::<LightData>(),
            64,
            "LightData must be exactly 64 bytes"
        );
    }

    #[test]
    fn test_lights_uniform_add() {
        let mut u = LightsUniform::new();
        u.add_light(LightData::directional(
            [0.0, -1.0, 0.0],
            [1.0, 1.0, 1.0],
            1.0,
        ));
        u.add_light(LightData::point(
            [0.0, 5.0, 0.0],
            [1.0, 0.0, 0.0],
            2.0,
            10.0,
        ));
        assert_eq!(u.count, 2);
    }

    #[test]
    fn test_lights_uniform_max() {
        let mut u = LightsUniform::new();
        for _ in 0..20 {
            u.add_light(LightData::directional(
                [0.0, -1.0, 0.0],
                [1.0, 1.0, 1.0],
                1.0,
            ));
        }
        assert_eq!(
            u.count, MAX_LIGHTS as u32,
            "count must be capped at MAX_LIGHTS (16)"
        );
    }

    #[test]
    fn test_spot_light_cone() {
        let light = LightData::spot(
            [0.0, 10.0, 0.0],
            [0.0, -1.0, 0.0],
            [1.0, 1.0, 1.0],
            3.0,
            20.0,
            15.0,
            30.0,
        );
        let expected_inner = 15.0_f32.to_radians().cos();
        let expected_outer = 30.0_f32.to_radians().cos();
        assert!(
            (light.inner_cone - expected_inner).abs() < 1e-6,
            "inner_cone should be cos(15°)"
        );
        assert!(
            (light.outer_cone - expected_outer).abs() < 1e-6,
            "outer_cone should be cos(30°)"
        );
    }
}
