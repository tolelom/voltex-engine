use std::ops::Mul;
use crate::{Vec3, Vec4};

/// 4x4 matrix in column-major order (matches wgpu/WGSL convention).
///
/// `cols[i]` is the i-th column, stored as `[f32; 4]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

impl Mat4 {
    /// The identity matrix.
    pub const IDENTITY: Self = Self {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// Construct from four column vectors.
    pub fn from_cols(c0: [f32; 4], c1: [f32; 4], c2: [f32; 4], c3: [f32; 4]) -> Self {
        Self { cols: [c0, c1, c2, c3] }
    }

    /// Return a flat 16-element slice suitable for GPU upload.
    ///
    /// # Safety
    /// `[[f32; 4]; 4]` and `[f32; 16]` have identical layout (both are 64 bytes,
    /// 4-byte aligned), so the transmute is well-defined.
    pub fn as_slice(&self) -> &[f32; 16] {
        // SAFETY: [[f32;4];4] is layout-identical to [f32;16].
        unsafe { &*(self.cols.as_ptr() as *const [f32; 16]) }
    }

    /// Matrix × matrix multiplication.
    pub fn mul_mat4(&self, rhs: &Mat4) -> Mat4 {
        let mut result = [[0.0f32; 4]; 4];
        for (col, col_data) in result.iter_mut().enumerate() {
            for (row, slot) in col_data.iter_mut().enumerate() {
                let mut sum = 0.0f32;
                for k in 0..4 {
                    sum += self.cols[k][row] * rhs.cols[col][k];
                }
                *slot = sum;
            }
        }
        Mat4 { cols: result }
    }

    /// Matrix × Vec4 multiplication.
    pub fn mul_vec4(&self, v: Vec4) -> Vec4 {
        let x = self.cols[0][0] * v.x + self.cols[1][0] * v.y + self.cols[2][0] * v.z + self.cols[3][0] * v.w;
        let y = self.cols[0][1] * v.x + self.cols[1][1] * v.y + self.cols[2][1] * v.z + self.cols[3][1] * v.w;
        let z = self.cols[0][2] * v.x + self.cols[1][2] * v.y + self.cols[2][2] * v.z + self.cols[3][2] * v.w;
        let w = self.cols[0][3] * v.x + self.cols[1][3] * v.y + self.cols[2][3] * v.z + self.cols[3][3] * v.w;
        Vec4 { x, y, z, w }
    }

    /// Translation matrix for (x, y, z).
    pub fn translation(x: f32, y: f32, z: f32) -> Self {
        Self {
            cols: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [x,   y,   z,   1.0],
            ],
        }
    }

    /// Uniform/non-uniform scale matrix.
    pub fn scale(sx: f32, sy: f32, sz: f32) -> Self {
        Self {
            cols: [
                [sx,  0.0, 0.0, 0.0],
                [0.0, sy,  0.0, 0.0],
                [0.0, 0.0, sz,  0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Rotation around the X axis by `angle` radians (right-handed).
    pub fn rotation_x(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            cols: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0,  c,   s,  0.0],
                [0.0, -s,   c,  0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Rotation around the Y axis by `angle` radians (right-handed).
    pub fn rotation_y(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            cols: [
                [ c,  0.0, -s,  0.0],
                [0.0, 1.0, 0.0, 0.0],
                [ s,  0.0,  c,  0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Rotation around the Z axis by `angle` radians (right-handed).
    pub fn rotation_z(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            cols: [
                [ c,   s,  0.0, 0.0],
                [-s,   c,  0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Right-handed look-at view matrix.
    ///
    /// - `eye`    — camera position
    /// - `target` — point the camera is looking at
    /// - `up`     — world up vector (usually `Vec3::Y`)
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = (target - eye).normalize(); // forward
        let r = f.cross(up).normalize();    // right
        let u = r.cross(f);                 // true up

        Self {
            cols: [
                [r.x,            u.x,            -f.x,           0.0],
                [r.y,            u.y,            -f.y,           0.0],
                [r.z,            u.z,            -f.z,           0.0],
                [-r.dot(eye),   -u.dot(eye),      f.dot(eye),    1.0],
            ],
        }
    }

    /// Perspective projection for wgpu NDC (z in [0, 1]).
    ///
    /// - `fov_y`  — vertical field of view in radians
    /// - `aspect` — width / height
    /// - `near`   — near clip distance (positive)
    /// - `far`    — far  clip distance (positive)
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y / 2.0).tan();
        let range_inv = 1.0 / (near - far);
        Self {
            cols: [
                [f / aspect, 0.0, 0.0,                   0.0],
                [0.0,        f,   0.0,                   0.0],
                [0.0,        0.0, far * range_inv,       -1.0],
                [0.0,        0.0, near * far * range_inv, 0.0],
            ],
        }
    }

    /// Orthographic projection (wgpu NDC: z in `[0, 1]`).
    pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let rml = right - left;
        let tmb = top - bottom;
        let fmn = far - near;
        Self::from_cols(
            [2.0 / rml, 0.0, 0.0, 0.0],
            [0.0, 2.0 / tmb, 0.0, 0.0],
            [0.0, 0.0, -1.0 / fmn, 0.0],
            [-(right + left) / rml, -(top + bottom) / tmb, -near / fmn, 1.0],
        )
    }

    /// Compute the inverse of this matrix. Returns `None` if the matrix is singular.
    ///
    /// Uses direct cofactor expansion via 3x3 minors. The previous shortcut that
    /// expressed every cofactor in terms of just six 2x2 minors of rows {0,1}
    /// and six of rows {2,3} produced wrong results for entries whose 3x3
    /// minor spans top and bottom halves (e.g. row 0 + row 2 + row 3), which
    /// silently broke unprojection / viewport-picking.
    pub fn inverse(&self) -> Option<Self> {
        let m = &self.cols;
        let e = |r: usize, c: usize| -> f32 { m[c][r] };

        // 3x3 determinant of a row-major 3-by-3 block.
        let det3 = |a: f32, b: f32, c: f32,
                    d: f32, ee: f32, f: f32,
                    g: f32, h: f32, i: f32| -> f32 {
            a * (ee * i - f * h) - b * (d * i - f * g) + c * (d * h - ee * g)
        };

        // Cofactor at (r, c): signed 3x3 minor with row r and column c removed.
        let cof = |r: usize, c: usize| -> f32 {
            let mut rows = [0usize; 3];
            let mut cols = [0usize; 3];
            let mut ri = 0;
            for x in 0..4 {
                if x != r {
                    rows[ri] = x;
                    ri += 1;
                }
            }
            let mut ci = 0;
            for x in 0..4 {
                if x != c {
                    cols[ci] = x;
                    ci += 1;
                }
            }
            let sign = if (r + c).is_multiple_of(2) { 1.0 } else { -1.0 };
            sign * det3(
                e(rows[0], cols[0]), e(rows[0], cols[1]), e(rows[0], cols[2]),
                e(rows[1], cols[0]), e(rows[1], cols[1]), e(rows[1], cols[2]),
                e(rows[2], cols[0]), e(rows[2], cols[1]), e(rows[2], cols[2]),
            )
        };

        // det(M) = Laplace expansion along row 0.
        let det = e(0,0) * cof(0,0) + e(0,1) * cof(0,1)
                + e(0,2) * cof(0,2) + e(0,3) * cof(0,3);
        if det.abs() < 1e-12 {
            return None;
        }
        let inv_det = 1.0 / det;

        // inv(r, c) = cofactor(c, r) / det  (adjugate is transposed cofactor).
        // The (r, c) indices are needed on both sides — enumerate() would
        // hide the intent without removing the lookup.
        let mut out = [[0.0f32; 4]; 4];
        #[allow(clippy::needless_range_loop)]
        for r in 0..4 {
            for c in 0..4 {
                out[c][r] = cof(c, r) * inv_det;
            }
        }
        Some(Mat4 { cols: out })
    }

    /// Return the transpose of this matrix.
    pub fn transpose(&self) -> Self {
        let c = &self.cols;
        Self {
            cols: [
                [c[0][0], c[1][0], c[2][0], c[3][0]],
                [c[0][1], c[1][1], c[2][1], c[3][1]],
                [c[0][2], c[1][2], c[2][2], c[3][2]],
                [c[0][3], c[1][3], c[2][3], c[3][3]],
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Operator overloads
// ---------------------------------------------------------------------------

impl Mul<Mat4> for Mat4 {
    type Output = Mat4;
    fn mul(self, rhs: Mat4) -> Mat4 {
        self.mul_mat4(&rhs)
    }
}

impl Mul<Vec4> for Mat4 {
    type Output = Vec4;
    fn mul(self, rhs: Vec4) -> Vec4 {
        self.mul_vec4(rhs)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn mat4_approx_eq(a: &Mat4, b: &Mat4) -> bool {
        for col in 0..4 {
            for row in 0..4 {
                if !approx_eq(a.cols[col][row], b.cols[col][row]) {
                    return false;
                }
            }
        }
        true
    }

    fn vec4_approx_eq(a: Vec4, b: Vec4) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y) && approx_eq(a.z, b.z) && approx_eq(a.w, b.w)
    }

    // 1. IDENTITY * translation == translation
    #[test]
    fn test_identity_mul() {
        let t = Mat4::translation(1.0, 2.0, 3.0);
        let result = Mat4::IDENTITY * t;
        assert!(mat4_approx_eq(&result, &t));
    }

    // 2. translate(10,20,30) * point(1,2,3,1) == (11,22,33,1)
    #[test]
    fn test_translation_mul_vec4() {
        let t = Mat4::translation(10.0, 20.0, 30.0);
        let v = Vec4 { x: 1.0, y: 2.0, z: 3.0, w: 1.0 };
        let result = t * v;
        assert!(vec4_approx_eq(result, Vec4 { x: 11.0, y: 22.0, z: 33.0, w: 1.0 }));
    }

    // 3. scale(2,3,4) * (1,1,1,1) == (2,3,4,1)
    #[test]
    fn test_scale() {
        let s = Mat4::scale(2.0, 3.0, 4.0);
        let v = Vec4 { x: 1.0, y: 1.0, z: 1.0, w: 1.0 };
        let result = s * v;
        assert!(vec4_approx_eq(result, Vec4 { x: 2.0, y: 3.0, z: 4.0, w: 1.0 }));
    }

    // 4. rotation_y(90°) * (1,0,0,1) -> approximately (0,0,-1,1)
    #[test]
    fn test_rotation_y_90() {
        let r = Mat4::rotation_y(FRAC_PI_2);
        let v = Vec4 { x: 1.0, y: 0.0, z: 0.0, w: 1.0 };
        let result = r * v;
        assert!(approx_eq(result.x, 0.0));
        assert!(approx_eq(result.y, 0.0));
        assert!(approx_eq(result.z, -1.0));
        assert!(approx_eq(result.w, 1.0));
    }

    // 5. look_at(eye=(0,0,5), target=origin, up=Y) — origin maps to (0,0,-5)
    #[test]
    fn test_look_at_origin() {
        let eye    = Vec3::new(0.0, 0.0, 5.0);
        let target = Vec3::ZERO;
        let up     = Vec3::Y;
        let view   = Mat4::look_at(eye, target, up);

        // The world-space origin in homogeneous coords:
        let origin = Vec4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };
        let result = view * origin;

        assert!(approx_eq(result.x, 0.0));
        assert!(approx_eq(result.y, 0.0));
        assert!(approx_eq(result.z, -5.0));
        assert!(approx_eq(result.w, 1.0));
    }

    // 6. Near plane point maps to NDC z = 0
    #[test]
    fn test_perspective_near_plane() {
        let fov_y  = std::f32::consts::FRAC_PI_2; // 90°
        let aspect = 1.0f32;
        let near   = 1.0f32;
        let far    = 100.0f32;
        let proj   = Mat4::perspective(fov_y, aspect, near, far);

        // A point exactly at the near plane in view space (z = -near in RH).
        let p = Vec4 { x: 0.0, y: 0.0, z: -near, w: 1.0 };
        let clip = proj * p;
        // NDC z = clip.z / clip.w should equal 0 for the near plane.
        let ndc_z = clip.z / clip.w;
        assert!(approx_eq(ndc_z, 0.0), "near-plane NDC z = {ndc_z}, expected 0");
    }

    // 7. Transpose swaps rows and columns
    #[test]
    fn test_transpose() {
        let m = Mat4::from_cols(
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        );
        let t = m.transpose();
        // After transpose, col[i][j] == original col[j][i]
        for col in 0..4 {
            for row in 0..4 {
                assert!(approx_eq(t.cols[col][row], m.cols[row][col]),
                    "t.cols[{col}][{row}] = {} != m.cols[{row}][{col}] = {}",
                    t.cols[col][row], m.cols[row][col]);
            }
        }
    }

    // 8. Orthographic projection
    #[test]
    fn test_orthographic() {
        let proj = Mat4::orthographic(-10.0, 10.0, -10.0, 10.0, 0.1, 100.0);
        // Center point should map to (0, 0, ~0)
        let p = proj * Vec4::new(0.0, 0.0, -0.1, 1.0);
        let ndc = Vec3::new(p.x / p.w, p.y / p.w, p.z / p.w);
        assert!(approx_eq(ndc.x, 0.0));
        assert!(approx_eq(ndc.y, 0.0));
    }

    // M * inv(M) == I  for a perspective+lookAt composition. Regression for a
    // historical bug where inverse() built cofactors from only 2x2 minors of
    // rows {0,1}+{2,3}, which is insufficient for entries whose 3x3 minor spans
    // top and bottom halves of the matrix.
    #[test]
    fn test_inverse_composes_to_identity() {
        let view = Mat4::look_at(
            Vec3::new(0.0, 1.5, 5.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 100.0);
        let vp = proj.mul_mat4(&view);
        let inv = vp.inverse().expect("invertible");
        let product = vp.mul_mat4(&inv);
        for col in 0..4 {
            for row in 0..4 {
                let expected = if col == row { 1.0 } else { 0.0 };
                assert!(
                    (product.cols[col][row] - expected).abs() < 1e-3,
                    "[col {} row {}] = {}, expected {}",
                    col, row, product.cols[col][row], expected,
                );
            }
        }
    }

    #[test]
    fn test_inverse_translate_roundtrip() {
        let t = Mat4::translation(3.0, -4.0, 7.0);
        let inv = t.inverse().expect("translation invertible");
        let v = Vec4::new(1.0, 2.0, 3.0, 1.0);
        let mapped = t * v;
        let back = inv * mapped;
        assert!(vec4_approx_eq(back, v));
    }

    // 9. as_slice — identity diagonal
    #[test]
    fn test_as_slice() {
        let slice = Mat4::IDENTITY.as_slice();
        assert_eq!(slice.len(), 16);
        // Diagonal indices in column-major flat layout: 0, 5, 10, 15
        assert!(approx_eq(slice[0],  1.0));
        assert!(approx_eq(slice[5],  1.0));
        assert!(approx_eq(slice[10], 1.0));
        assert!(approx_eq(slice[15], 1.0));
        // Off-diagonal should be zero (spot check)
        assert!(approx_eq(slice[1],  0.0));
        assert!(approx_eq(slice[4],  0.0));
    }
}
