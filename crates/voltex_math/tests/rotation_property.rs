//! Randomized algebraic properties for `Quat` and `Mat4`: quaternion rotation
//! agrees with Rodrigues' formula and with `to_mat4`, products compose, Euler
//! YXZ round-trips away from gimbal lock, slerp stays on the unit sphere at
//! constant angular speed, and `Mat4::inverse` inverts TRS matrices across
//! scales from 1e-4 to 1e3.

use voltex_math::{Mat4, Quat, Vec3, Vec4};

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() % 100_000) as f32 / 100_000.0 * (hi - lo)
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vec3 {
        Vec3::new(self.range(lo, hi), self.range(lo, hi), self.range(lo, hi))
    }
    fn unit(&mut self) -> Vec3 {
        loop {
            let v = self.vec(-1.0, 1.0);
            let l = v.length();
            if l > 0.1 && l <= 1.0 {
                return v * (1.0 / l);
            }
        }
    }
    fn quat(&mut self) -> Quat {
        let axis = self.unit();
        Quat::from_axis_angle(axis, self.range(-std::f32::consts::PI, std::f32::consts::PI))
    }
}

fn rotate(q: Quat, v: Vec3) -> Vec3 {
    let p = q * Quat::new(v.x, v.y, v.z, 0.0) * q.conjugate();
    Vec3::new(p.x, p.y, p.z)
}

fn transform(m: Mat4, v: Vec3) -> Vec3 {
    let r = m * Vec4::new(v.x, v.y, v.z, 1.0);
    Vec3::new(r.x, r.y, r.z)
}

fn close(a: Vec3, b: Vec3, tol: f32) -> bool {
    (a - b).length() <= tol * (1.0 + b.length())
}

#[test]
fn quaternion_rotation_matches_rodrigues_and_to_mat4() {
    let mut rng = XorShift(0x9017_A7E5);
    for case in 0..5000 {
        let axis = rng.unit();
        let angle = rng.range(-6.0, 6.0);
        let v = rng.vec(-10.0, 10.0);
        let q = Quat::from_axis_angle(axis, angle);
        let (s, c) = angle.sin_cos();
        let rodrigues = v * c + axis.cross(v) * s + axis * (axis.dot(v) * (1.0 - c));
        assert!((q.length() - 1.0).abs() < 1e-5, "case {case}: |q| = {}", q.length());
        assert!(close(rotate(q, v), rodrigues, 1e-4), "case {case}: q·v {:?} vs {rodrigues:?}", rotate(q, v));
        assert!(close(transform(q.to_mat4(), v), rodrigues, 1e-4), "case {case}: to_mat4");
        assert!(close(rotate(q.inverse(), rodrigues), v, 1e-4), "case {case}: inverse");
    }
}

#[test]
fn quaternion_and_matrix_products_compose_the_same_way() {
    let mut rng = XorShift(0xC0DE_0001);
    for case in 0..3000 {
        let (a, b) = (rng.quat(), rng.quat());
        let v = rng.vec(-5.0, 5.0);
        let by_quat = rotate(a * b, v);
        assert!(close(by_quat, rotate(a, rotate(b, v)), 1e-4), "case {case}: (ab)v != a(bv)");
        assert!(close(transform(a.to_mat4() * b.to_mat4(), v), by_quat, 1e-4), "case {case}: mat product");
    }
}

#[test]
fn euler_yxz_round_trips_and_matches_matrix_order() {
    let mut rng = XorShift(0xE7E7_0042);
    for case in 0..5000 {
        let e = Vec3::new(rng.range(-1.5, 1.5), rng.range(-3.1, 3.1), rng.range(-3.1, 3.1));
        let q = Quat::from_euler_yxz(e);
        let m = Mat4::rotation_y(e.y) * Mat4::rotation_x(e.x) * Mat4::rotation_z(e.z);
        let v = rng.vec(-3.0, 3.0);
        assert!(close(transform(q.to_mat4(), v), transform(m, v), 1e-4), "case {case}: euler {e:?} order");
        let back = Quat::from_euler_yxz(q.to_euler_yxz());
        // Same rotation (q and -q are equivalent).
        assert!(back.dot(q).abs() > 1.0 - 1e-4, "case {case}: euler {e:?} -> {:?}", q.to_euler_yxz());
    }
}

#[test]
fn slerp_is_unit_constant_speed_and_hits_endpoints() {
    let mut rng = XorShift(0x51E2_0007);
    for case in 0..2000 {
        let (a, b) = (rng.quat(), rng.quat());
        let start = Quat::slerp(a, b, 0.0);
        let end = Quat::slerp(a, b, 1.0);
        assert!(start.dot(a).abs() > 1.0 - 1e-5, "case {case}: slerp(0) != a");
        assert!(end.dot(b).abs() > 1.0 - 1e-5, "case {case}: slerp(1) != b");
        let total = 2.0 * a.dot(b).abs().clamp(0.0, 1.0).acos();
        let mut prev = a;
        for i in 1..=8 {
            let q = Quat::slerp(a, b, i as f32 / 8.0);
            assert!((q.length() - 1.0).abs() < 1e-4, "case {case}: |slerp| = {}", q.length());
            let step = 2.0 * q.dot(prev).abs().clamp(0.0, 1.0).acos();
            assert!((step - total / 8.0).abs() < 2e-3, "case {case}: step {step} vs {}", total / 8.0);
            prev = q;
        }
    }
}

#[test]
fn mat4_inverse_inverts_trs_across_scales() {
    let mut rng = XorShift(0x1A7E_0013);
    for case in 0..5000 {
        // Translation proportional to the scale keeps the round trip well
        // conditioned in f32; what's under test is the inverse itself.
        let scale = 10f32.powf(rng.range(-4.0, 3.0));
        let s = rng.vec(0.5, 2.0) * scale;
        let t = rng.vec(-100.0, 100.0) * scale;
        let m = Mat4::translation(t.x, t.y, t.z) * rng.quat().to_mat4() * Mat4::scale(s.x, s.y, s.z);
        let inv = m.inverse().unwrap_or_else(|| panic!("case {case}: invertible TRS (scale {scale}) reported singular"));
        let v = rng.vec(-10.0, 10.0);
        assert!(close(transform(inv, transform(m, v)), v, 1e-3), "case {case}: M⁻¹Mv != v at scale {scale}");
    }
}

#[test]
fn mat4_inverse_handles_large_world_orthographic_projection() {
    // A 100k-unit 2D map: det ≈ (2/w)(2/h)(1/depth) ≈ 4e-13, below the old
    // absolute 1e-12 cut-off, so picking/unprojection got None (and csm
    // panicked on its expect) for a perfectly well-conditioned matrix.
    let ortho = Mat4::orthographic(-50_000.0, 50_000.0, -50_000.0, 50_000.0, 0.0, 1000.0);
    let inv = ortho.inverse().expect("large orthographic projection is invertible");
    let p = Vec3::new(12_345.0, -6_789.0, 500.0);
    assert!(close(transform(inv, transform(ortho, p)), p, 1e-4));
}

#[test]
fn mat4_inverse_rejects_singular_matrices_at_any_scale() {
    for scale in [1e-3f32, 1.0, 1e3] {
        let flat = Mat4::scale(scale, scale, 0.0);
        assert!(flat.inverse().is_none(), "zero-thickness scale at {scale}");
        let c = [scale, 2.0 * scale, 3.0 * scale, 0.0];
        let repeated = Mat4::from_cols(c, c, [0.0, 0.0, scale, 0.0], [0.0, 0.0, 0.0, 1.0]);
        assert!(repeated.inverse().is_none(), "two equal columns at {scale}");
    }
}

#[test]
fn mat4_inverse_accepts_view_projection_far_from_the_origin() {
    for distance in [1e3f32, 1e5, 1e6] {
        let eye = Vec3::new(distance, 10.0, distance);
        let view = Mat4::look_at(eye, eye + Vec3::new(1.0, -0.2, 0.3), Vec3::new(0.0, 1.0, 0.0));
        let vp = Mat4::perspective(1.0, 16.0 / 9.0, 0.1, 1000.0) * view;
        assert!(vp.inverse().is_some(), "camera {distance} units out");
    }
}
