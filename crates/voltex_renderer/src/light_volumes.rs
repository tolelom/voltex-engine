#![allow(missing_docs)]

/// Light volume shapes for deferred lighting optimization.
#[derive(Debug, Clone)]
pub enum LightVolume {
    Sphere { center: [f32; 3], radius: f32 },
    Cone { apex: [f32; 3], direction: [f32; 3], angle: f32, range: f32 },
    Fullscreen,
}

impl LightVolume {
    pub fn point_light(center: [f32; 3], radius: f32) -> Self { LightVolume::Sphere { center, radius } }
    pub fn spot_light(apex: [f32; 3], dir: [f32; 3], angle: f32, range: f32) -> Self {
        LightVolume::Cone { apex, direction: dir, angle, range }
    }
    pub fn directional() -> Self { LightVolume::Fullscreen }

    pub fn contains_point(&self, point: [f32; 3]) -> bool {
        match self {
            LightVolume::Sphere { center, radius } => {
                let dx = point[0]-center[0]; let dy = point[1]-center[1]; let dz = point[2]-center[2];
                dx*dx + dy*dy + dz*dz <= radius * radius
            }
            LightVolume::Fullscreen => true,
            LightVolume::Cone { apex, direction, angle, range } => {
                let dx = point[0]-apex[0]; let dy = point[1]-apex[1]; let dz = point[2]-apex[2];
                let dist = (dx*dx+dy*dy+dz*dz).sqrt();
                if dist > *range { return false; }
                if dist < 1e-6 { return true; }
                let dot = (dx*direction[0]+dy*direction[1]+dz*direction[2]) / dist;
                dot >= angle.cos()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_sphere_contains() {
        let v = LightVolume::point_light([0.0; 3], 5.0);
        assert!(v.contains_point([3.0, 0.0, 0.0]));
        assert!(!v.contains_point([6.0, 0.0, 0.0]));
    }
    #[test]
    fn test_fullscreen() {
        assert!(LightVolume::directional().contains_point([999.0, 999.0, 999.0]));
    }
    #[test]
    fn test_cone_contains() {
        let v = LightVolume::spot_light([0.0;3], [0.0,0.0,-1.0], 0.5, 10.0);
        assert!(v.contains_point([0.0, 0.0, -5.0])); // on axis
        assert!(!v.contains_point([0.0, 0.0, 5.0])); // behind
    }
}
