//! L2 Spherical Harmonics computation for procedural sky irradiance.
//!
//! Computes 9 SH coefficients (order 2) for 3 channels (RGB) from a procedural sky model.

use std::f32::consts::PI;

/// Real SH basis functions for L=0,1,2 evaluated at direction (x, y, z).
/// Returns array of 9 basis values.
fn sh_basis(x: f32, y: f32, z: f32) -> [f32; 9] {
    [
        0.282095,                           // Y00:  L=0 M=0
        0.488603 * y,                       // Y1-1: L=1 M=-1
        0.488603 * z,                       // Y10:  L=1 M=0
        0.488603 * x,                       // Y1+1: L=1 M=+1
        1.092548 * x * y,                   // Y2-2: L=2 M=-2
        1.092548 * y * z,                   // Y2-1: L=2 M=-1
        0.315392 * (3.0 * z * z - 1.0),    // Y20:  L=2 M=0
        1.092548 * x * z,                   // Y2+1: L=2 M=+1
        0.546274 * (x * x - y * y),         // Y2+2: L=2 M=+2
    ]
}

/// Hosek-Wilkie inspired procedural sky evaluation.
/// Returns RGB radiance for a given direction.
fn procedural_sky(dir: [f32; 3], sun_dir: [f32; 3], turbidity: f32) -> [f32; 3] {
    let turb = turbidity.clamp(1.5, 10.0);

    if dir[1] > 0.0 {
        // Rayleigh scattering: blue zenith, warm horizon
        let zenith_r = 0.15 / (turb * 0.15 + 0.5);
        let zenith_g = 0.3 / (turb * 0.15 + 0.5);
        let zenith_b = 0.8 / (turb * 0.15 + 0.5);

        let horizon_r = 0.7 * (1.0 + turb * 0.04);
        let horizon_g = 0.6 * (1.0 + turb * 0.04);
        let horizon_b = 0.5 * (1.0 + turb * 0.04);

        let elevation = dir[1];
        let t = elevation.powf(0.4);

        let sky_r = horizon_r + (zenith_r - horizon_r) * t;
        let sky_g = horizon_g + (zenith_g - horizon_g) * t;
        let sky_b = horizon_b + (zenith_b - horizon_b) * t;

        // Mie scattering near sun
        let cos_sun = (dir[0] * sun_dir[0] + dir[1] * sun_dir[1] + dir[2] * sun_dir[2]).max(0.0);
        let mie_strength = turb * 0.02;
        let mie_factor = mie_strength * cos_sun.powi(8);

        // Sun disk
        let sun_factor = cos_sun.powi(2048);

        [
            sky_r + mie_factor * 1.0 + sun_factor * 10.0,
            sky_g + mie_factor * 0.9 + sun_factor * 9.0,
            sky_b + mie_factor * 0.7 + sun_factor * 7.0,
        ]
    } else {
        // Ground
        let t = (-dir[1]).powf(0.4);
        let horizon = [0.6f32, 0.55, 0.45];
        let ground = [0.1f32, 0.08, 0.06];
        [
            horizon[0] + (ground[0] - horizon[0]) * t,
            horizon[1] + (ground[1] - horizon[1]) * t,
            horizon[2] + (ground[2] - horizon[2]) * t,
        ]
    }
}

fn normalize_vec3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-8 {
        return [0.0, 1.0, 0.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

/// Compute L2 (order 2) SH coefficients from the procedural sky model.
///
/// Samples the environment at `num_samples` directions distributed over the full sphere
/// and accumulates SH basis function values weighted by environment radiance.
///
/// Returns 9 RGB coefficient triplets.
///
/// # Arguments
/// * `sun_dir` - Normalized sun direction vector
/// * `sun_color` - Sun color (unused in current procedural model, kept for API compatibility)
/// * `turbidity` - Atmospheric turbidity (1.5 to 10.0)
pub fn compute_sh_coefficients(
    sun_dir: [f32; 3],
    _sun_color: [f32; 3],
    turbidity: f32,
) -> [[f32; 3]; 9] {
    compute_sh_coefficients_with_samples(sun_dir, _sun_color, turbidity, 128)
}

/// Same as `compute_sh_coefficients` but with configurable sample count per axis.
/// Total samples = `samples_per_axis * samples_per_axis`.
pub fn compute_sh_coefficients_with_samples(
    sun_dir: [f32; 3],
    _sun_color: [f32; 3],
    turbidity: f32,
    samples_per_axis: u32,
) -> [[f32; 3]; 9] {
    let sun_dir = normalize_vec3(sun_dir);
    let n = samples_per_axis;
    let total = n * n;

    let mut coeffs = [[0.0f32; 3]; 9];

    // Sample uniformly on the sphere using spherical coordinates
    for i in 0..n {
        let theta = PI * (i as f32 + 0.5) / n as f32; // [0, pi]
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for j in 0..n {
            let phi = 2.0 * PI * (j as f32 + 0.5) / n as f32; // [0, 2pi]

            let x = sin_theta * phi.cos();
            let y = cos_theta; // y-up convention
            let z = sin_theta * phi.sin();

            let radiance = procedural_sky([x, y, z], sun_dir, turbidity);
            let basis = sh_basis(x, y, z);

            // Monte Carlo weight: sphere area = 4*pi, uniform PDF = 1/(4*pi)
            // weight = radiance * basis * sin_theta * (pi/n) * (2*pi/n) / (1/(4*pi))
            // But for uniform sphere sampling with stratified grid:
            // weight = (4*pi / total) * radiance * basis
            // sin_theta is already accounted for by the area element
            let weight = 4.0 * PI * sin_theta / total as f32;
            // Actually the correct formula for stratified spherical integration:
            // dA = sin(theta) * dtheta * dphi
            // dtheta = pi/n, dphi = 2*pi/n
            // weight = sin(theta) * (pi/n) * (2*pi/n)
            let _correct_weight = sin_theta * (PI / n as f32) * (2.0 * PI / n as f32);

            for k in 0..9 {
                let w = _correct_weight * basis[k];
                coeffs[k][0] += w * radiance[0];
                coeffs[k][1] += w * radiance[1];
                coeffs[k][2] += w * radiance[2];
            }
        }
    }

    coeffs
}

/// Pack 9 RGB SH coefficients into 7 vec4s (28 floats) for GPU uniform buffer.
/// Layout: coefficients are stored sequentially as [c0.r, c0.g, c0.b, c1.r, c1.g, c1.b, ...]
/// packed into vec4s.
pub fn pack_sh_coefficients(coeffs: &[[f32; 3]; 9]) -> [[f32; 4]; 7] {
    // Flatten 9*3 = 27 floats, pad to 28
    let mut flat = [0.0f32; 28];
    for (i, c) in coeffs.iter().enumerate() {
        flat[i * 3] = c[0];
        flat[i * 3 + 1] = c[1];
        flat[i * 3 + 2] = c[2];
    }
    // flat[27] = 0.0 (padding)

    let mut packed = [[0.0f32; 4]; 7];
    for i in 0..7 {
        packed[i] = [flat[i * 4], flat[i * 4 + 1], flat[i * 4 + 2], flat[i * 4 + 3]];
    }
    packed
}

/// Evaluate SH at a given normal direction (CPU-side, for testing).
pub fn evaluate_sh_cpu(normal: [f32; 3], coeffs: &[[f32; 3]; 9]) -> [f32; 3] {
    let basis = sh_basis(normal[0], normal[1], normal[2]);
    let mut result = [0.0f32; 3];
    for k in 0..9 {
        result[0] += coeffs[k][0] * basis[k];
        result[1] += coeffs[k][1] * basis[k];
        result[2] += coeffs[k][2] * basis[k];
    }
    // Clamp to non-negative
    result[0] = result[0].max(0.0);
    result[1] = result[1].max(0.0);
    result[2] = result[2].max(0.0);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// For a uniform white environment (radiance = 1.0 everywhere),
    /// only L=0 coefficient should be non-zero, equal to sqrt(4*pi) * 1.0 / sqrt(4*pi) = sqrt(pi) / ...
    /// Actually, for uniform radiance L(d) = 1:
    /// c_00 = integral(1 * Y00 * dw) = Y00 * 4*pi = 0.282095 * 4*pi ≈ 3.5449
    /// All other coefficients should be ~0.
    #[test]
    fn test_sh_uniform_white_environment() {
        // We'll compute SH for a "uniform white" sky by using a custom function
        // Instead of procedural_sky, we test the basis directly
        let n = 128u32;
        let mut coeffs = [[0.0f32; 3]; 9];

        for i in 0..n {
            let theta = PI * (i as f32 + 0.5) / n as f32;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            for j in 0..n {
                let phi = 2.0 * PI * (j as f32 + 0.5) / n as f32;
                let x = sin_theta * phi.cos();
                let y = cos_theta;
                let z = sin_theta * phi.sin();

                let radiance = [1.0f32, 1.0, 1.0]; // uniform white
                let basis = sh_basis(x, y, z);
                let weight = sin_theta * (PI / n as f32) * (2.0 * PI / n as f32);

                for k in 0..9 {
                    let w = weight * basis[k];
                    coeffs[k][0] += w * radiance[0];
                    coeffs[k][1] += w * radiance[1];
                    coeffs[k][2] += w * radiance[2];
                }
            }
        }

        // L=0 coefficient should be approximately 0.282095 * 4*pi ≈ 3.5449
        let expected_c0 = 0.282095 * 4.0 * PI;
        assert!(
            (coeffs[0][0] - expected_c0).abs() < 0.05,
            "c0.r = {} expected ~{}", coeffs[0][0], expected_c0
        );
        assert!(
            (coeffs[0][1] - expected_c0).abs() < 0.05,
            "c0.g = {} expected ~{}", coeffs[0][1], expected_c0
        );
        assert!(
            (coeffs[0][2] - expected_c0).abs() < 0.05,
            "c0.b = {} expected ~{}", coeffs[0][2], expected_c0
        );

        // All higher-order coefficients should be ~0
        for k in 1..9 {
            for ch in 0..3 {
                assert!(
                    coeffs[k][ch].abs() < 0.05,
                    "c{}[{}] = {} should be ~0", k, ch, coeffs[k][ch]
                );
            }
        }
    }

    /// For a directional light along +Y, L1 coefficient for Y (index 1) should dominate.
    #[test]
    fn test_sh_directional_light_dominant_l1() {
        // Simulate a directional "light" by using a sky that is bright only near +Y
        let n = 128u32;
        let mut coeffs = [[0.0f32; 3]; 9];

        for i in 0..n {
            let theta = PI * (i as f32 + 0.5) / n as f32;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            for j in 0..n {
                let phi = 2.0 * PI * (j as f32 + 0.5) / n as f32;
                let x = sin_theta * phi.cos();
                let y = cos_theta;
                let z = sin_theta * phi.sin();

                // Concentrated light near +Y direction
                let intensity = y.max(0.0).powi(32);
                let radiance = [intensity; 3];
                let basis = sh_basis(x, y, z);
                let weight = sin_theta * (PI / n as f32) * (2.0 * PI / n as f32);

                for k in 0..9 {
                    let w = weight * basis[k];
                    coeffs[k][0] += w * radiance[0];
                    coeffs[k][1] += w * radiance[1];
                    coeffs[k][2] += w * radiance[2];
                }
            }
        }

        // L1 Y-component (index 1, which is 0.488603 * y) should be significant
        // L0 (index 0) should also be non-zero (DC component)
        assert!(
            coeffs[0][0] > 0.01,
            "L0 coefficient should be positive for directional light: {}", coeffs[0][0]
        );
        assert!(
            coeffs[1][0] > 0.01,
            "L1(-1) Y-direction coefficient should be positive: {}", coeffs[1][0]
        );

        // The L1 Y-component should be the largest L1 component (light is along +Y)
        assert!(
            coeffs[1][0].abs() > coeffs[2][0].abs(),
            "L1(-1) Y should dominate over L1(0) Z: {} vs {}", coeffs[1][0], coeffs[2][0]
        );
        assert!(
            coeffs[1][0].abs() > coeffs[3][0].abs(),
            "L1(-1) Y should dominate over L1(+1) X: {} vs {}", coeffs[1][0], coeffs[3][0]
        );
    }

    #[test]
    fn test_sh_procedural_sky_coefficients() {
        let coeffs = compute_sh_coefficients(
            [0.5, -0.7, 0.5],
            [1.0, 1.0, 1.0],
            3.0,
        );

        // L0 should be positive (sky has positive radiance)
        assert!(coeffs[0][0] > 0.0, "L0 R should be positive");
        assert!(coeffs[0][1] > 0.0, "L0 G should be positive");
        assert!(coeffs[0][2] > 0.0, "L0 B should be positive");

        // Verify coefficients are finite
        for k in 0..9 {
            for ch in 0..3 {
                assert!(
                    coeffs[k][ch].is_finite(),
                    "SH coefficient c{}[{}] = {} is not finite", k, ch, coeffs[k][ch]
                );
            }
        }
    }

    #[test]
    fn test_pack_sh_coefficients() {
        let mut coeffs = [[0.0f32; 3]; 9];
        for k in 0..9 {
            coeffs[k] = [(k * 3) as f32, (k * 3 + 1) as f32, (k * 3 + 2) as f32];
        }

        let packed = pack_sh_coefficients(&coeffs);

        // Verify flat layout: c0.r, c0.g, c0.b, c1.r, c1.g, c1.b, ...
        assert_eq!(packed[0], [0.0, 1.0, 2.0, 3.0]);   // c0.rgb, c1.r
        assert_eq!(packed[1], [4.0, 5.0, 6.0, 7.0]);   // c1.gb, c2.rg
        assert_eq!(packed[6][0], 24.0); // c8.r
        assert_eq!(packed[6][1], 25.0); // c8.g
        assert_eq!(packed[6][2], 26.0); // c8.b
        assert_eq!(packed[6][3], 0.0);  // padding
    }

    #[test]
    fn test_evaluate_sh_cpu_positive() {
        let coeffs = compute_sh_coefficients(
            [0.5, -0.7, 0.5],
            [1.0, 1.0, 1.0],
            3.0,
        );

        // Evaluate at several directions — should be non-negative
        let dirs = [
            [0.0, 1.0, 0.0],   // up
            [0.0, -1.0, 0.0],  // down
            [1.0, 0.0, 0.0],   // right
            [0.0, 0.0, 1.0],   // forward
        ];

        for dir in &dirs {
            let result = evaluate_sh_cpu(*dir, &coeffs);
            assert!(
                result[0] >= 0.0 && result[1] >= 0.0 && result[2] >= 0.0,
                "SH evaluation at {:?} should be non-negative: {:?}", dir, result
            );
        }
    }
}
