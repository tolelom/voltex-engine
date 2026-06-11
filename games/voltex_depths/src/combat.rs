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

// ── Task 9: 투사체 스폰/이동/벽 소멸 ─────────────────────────────────────

use voltex::physics::Collider;
use voltex_runtime::Lifetime;

use crate::components::{Projectile, Team, Velocity};
use crate::level::LevelMap;

#[allow(dead_code)] // 호출자: Task 11 게임 루프
pub fn spawn_projectile(
    world: &mut World,
    from: Vec3,
    dir: Vec3,
    team: Team,
    damage: i32,
    speed: f32,
) {
    let d = dir.normalize_or_zero();
    if d.x == 0.0 && d.z == 0.0 {
        return;
    }
    let color = match team {
        Team::Player => [1.0, 0.95, 0.35, 1.0],
        Team::Enemy => [0.95, 0.30, 0.60, 1.0],
    };
    world.spawn_bundle((
        Transform::from_position(from),
        PrimitiveMesh::sphere_default(0.18),
        MaterialDesc::color(color),
        Projectile { team, damage },
        Velocity(d * speed),
        Collider::Sphere { radius: 0.25 },
        Lifetime::new(3.0),
    ));
}

/// 투사체 이동 + 벽 셀 진입 시 즉시 소멸 (Lifetime은 안전망).
#[allow(dead_code)] // 호출자: Task 11 게임 루프
pub fn projectile_system(world: &mut World, map: &LevelMap, dt: f32) {
    // 스냅샷 패턴: query 중 world.get_mut 빌림 충돌 방지
    let moves: Vec<(Entity, Vec3)> = world
        .query::<Projectile>()
        .filter_map(|(e, _)| {
            let v = world.get::<Velocity>(e)?.0;
            Some((e, v * dt))
        })
        .collect();

    let mut to_kill: Vec<Entity> = Vec::new();
    for (e, dp) in moves {
        if let Some(t) = world.get_mut::<Transform>(e) {
            t.position = t.position + dp;
            if map.blocked_at(t.position) {
                to_kill.push(e);
            }
        }
    }
    for e in to_kill {
        voltex::ecs::despawn_recursive(world, e);
    }
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

    // ── Task 9 ──

    #[test]
    fn projectile_moves_and_dies_on_wall() {
        let map =
            crate::level::LevelMap::parse(&["#####", "#P..#", "#####"]).unwrap();
        let mut w = World::new();
        let from = map.world_pos(1, 1, 0.6);
        spawn_projectile(
            &mut w,
            from,
            Vec3::new(1.0, 0.0, 0.0),
            crate::components::Team::Player,
            1,
            14.0,
        );
        let e = w
            .query::<crate::components::Projectile>()
            .map(|(e, _)| e)
            .next()
            .unwrap();
        projectile_system(&mut w, &map, 0.1);
        let after = w.get::<Transform>(e).unwrap().position;
        assert!(after.x > from.x, "projectile must advance +x");
        for _ in 0..40 {
            projectile_system(&mut w, &map, 0.1);
        }
        assert!(!w.is_alive(e), "projectile must despawn on wall");
    }
}
