use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SoftShadowParams {
    pub light_dir: [f32; 3],
    pub num_rays: u32,
    pub light_radius: f32,
    pub max_distance: f32,
    pub _pad: [f32; 2],
}

impl Default for SoftShadowParams {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftShadowParams {
    pub fn new() -> Self {
        SoftShadowParams { light_dir: [0.0, -1.0, 0.0], num_rays: 16, light_radius: 0.02, max_distance: 50.0, _pad: [0.0; 2] }
    }
}

/// Penumbra estimation: wider penumbra for objects farther from occluder.
pub fn penumbra_size(light_radius: f32, blocker_dist: f32, receiver_dist: f32) -> f32 {
    if blocker_dist < 1e-6 { return 0.0; }
    light_radius * (receiver_dist - blocker_dist) / blocker_dist
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_params_default() { let p = SoftShadowParams::new(); assert_eq!(p.num_rays, 16); }
    #[test]
    fn test_penumbra_close() { assert!((penumbra_size(0.02, 1.0, 1.1) - 0.002).abs() < 0.001); }
    #[test]
    fn test_penumbra_far() { assert!(penumbra_size(0.02, 1.0, 5.0) > penumbra_size(0.02, 1.0, 2.0)); }
}
