//! 전투: 조준 unprojection, 투사체, 피격/사망.
use voltex::prelude::*;

// ── Task 8: 마우스 조준 unprojection ──────────────────────────────────────

/// 화면 픽셀 (mx, my) → NDC (x, y).
/// NDC x: [-1, 1] (좌 → 우), y: [-1, 1] (아래 → 위).
#[allow(dead_code)] // 호출자: Task 11 게임 루프
pub fn ndc_from_mouse(mx: f64, my: f64, w: f32, h: f32) -> (f32, f32) {
    ((2.0 * mx as f32 / w) - 1.0, 1.0 - (2.0 * my as f32 / h))
}

/// NDC 좌표 → 월드 레이 → y=plane_y 평면 교점.
/// 레이가 평면과 평행하거나 교점이 카메라 뒤면 None.
#[allow(dead_code)] // 호출자: Task 11 게임 루프
pub fn unproject_to_plane(vp_inv: &Mat4, ndc: (f32, f32), plane_y: f32) -> Option<Vec3> {
    let h = |v: Vec4| -> Option<Vec3> {
        if v.w.abs() < 1e-9 {
            return None;
        }
        Some(Vec3::new(v.x / v.w, v.y / v.w, v.z / v.w))
    };
    // wgpu NDC z: near=0.0, far=1.0
    let near = h(vp_inv.mul_vec4(Vec4::new(ndc.0, ndc.1, 0.0, 1.0)))?;
    let far = h(vp_inv.mul_vec4(Vec4::new(ndc.0, ndc.1, 1.0, 1.0)))?;
    let dir = far - near;
    if dir.y.abs() < 1e-6 {
        return None;
    }
    let t = (plane_y - near.y) / dir.y;
    if t < 0.0 {
        return None;
    }
    Some(near + dir * t)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use voltex::prelude::*;

    // ── Task 8 ──

    #[test]
    fn ndc_maps_screen_corners() {
        assert_eq!(ndc_from_mouse(0.0, 0.0, 1280.0, 720.0), (-1.0, 1.0));
        let (cx, cy) = ndc_from_mouse(640.0, 360.0, 1280.0, 720.0);
        assert!(cx.abs() < 1e-6 && cy.abs() < 1e-6);
    }

    #[test]
    fn unproject_center_hits_look_target() {
        let eye = Vec3::new(0.0, 10.0, 5.0);
        let view = Mat4::look_at(eye, Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let proj = Mat4::perspective(45f32.to_radians(), 16.0 / 9.0, 0.1, 100.0);
        // view_projection() = proj * view (camera.rs:101)
        let vp = proj * view;
        let inv = vp.inverse().unwrap();
        let hit = unproject_to_plane(&inv, (0.0, 0.0), 0.5).unwrap();
        assert!(
            hit.x.abs() < 0.1 && hit.z.abs() < 0.1,
            "center must hit look target: {hit:?}"
        );
        let right = unproject_to_plane(&inv, (0.5, 0.0), 0.5).unwrap();
        assert!(right.x > hit.x);
    }
}
