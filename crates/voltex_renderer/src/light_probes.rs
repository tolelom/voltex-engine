#![allow(missing_docs)]

pub struct LightProbe {
    pub position: [f32; 3],
    pub sh_coefficients: [[f32; 3]; 9], // L2 SH, 9 RGB coefficients
}

impl LightProbe {
    pub fn new(position: [f32; 3]) -> Self {
        LightProbe { position, sh_coefficients: [[0.0; 3]; 9] }
    }
    pub fn evaluate_irradiance(&self, normal: [f32; 3]) -> [f32; 3] {
        // L0
        let mut result = [self.sh_coefficients[0][0], self.sh_coefficients[0][1], self.sh_coefficients[0][2]];
        // L1
        let (nx, ny, nz) = (normal[0], normal[1], normal[2]);
        for (c, slot) in result.iter_mut().enumerate() {
            *slot += self.sh_coefficients[1][c] * ny;
            *slot += self.sh_coefficients[2][c] * nz;
            *slot += self.sh_coefficients[3][c] * nx;
        }
        result
    }
}

pub struct LightProbeGrid {
    probes: Vec<LightProbe>,
}

impl Default for LightProbeGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl LightProbeGrid {
    pub fn new() -> Self { LightProbeGrid { probes: Vec::new() } }
    pub fn add(&mut self, probe: LightProbe) { self.probes.push(probe); }
    pub fn nearest(&self, pos: [f32; 3]) -> Option<&LightProbe> {
        self.probes.iter().min_by(|a, b| {
            let da = dist_sq(a.position, pos);
            let db = dist_sq(b.position, pos);
            da.partial_cmp(&db).unwrap()
        })
    }
    pub fn len(&self) -> usize { self.probes.len() }
    pub fn is_empty(&self) -> bool { self.probes.is_empty() }
}

fn dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0]-b[0]; let dy = a[1]-b[1]; let dz = a[2]-b[2]; dx*dx+dy*dy+dz*dz
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_probe_evaluate() {
        let mut p = LightProbe::new([0.0; 3]);
        p.sh_coefficients[0] = [0.5, 0.5, 0.5]; // ambient
        let irr = p.evaluate_irradiance([0.0, 1.0, 0.0]);
        assert!(irr[0] > 0.0);
    }
    #[test]
    fn test_grid_nearest() {
        let mut grid = LightProbeGrid::new();
        grid.add(LightProbe::new([0.0, 0.0, 0.0]));
        grid.add(LightProbe::new([10.0, 0.0, 0.0]));
        let nearest = grid.nearest([1.0, 0.0, 0.0]).unwrap();
        assert!((nearest.position[0] - 0.0).abs() < 1e-6);
    }
}
