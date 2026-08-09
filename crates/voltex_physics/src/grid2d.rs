//! XY 평면 AABB vs 타일 그리드 축 분리(X→Y) 이동·해소.
//!
//! 타일 기반 2D 게임(플랫포머/탑다운) 공통 캐릭터 이동 코어.
//! `is_solid(gx, gy)` 클로저가 그리드를 정의하므로 맵 타입에 비의존이고,
//! **경계 정책(맵 밖 solid/open)은 호출자 소유**다.
//!
//! 전제: 한 프레임 이동량이 `tile` 미만 (스윕 없음 — 속도 × dt ≪ tile).
//! 관통 시 접촉면에 스냅 + 해당 축 속도 0. 중력/점프 같은 튜닝 상수는
//! 게임 영역 (voltex_leap `physics2d.rs`에서 승격 — dogfood-friction-2 #4).

use voltex_math::Vec3;

/// 셀 경계 flush 접촉을 겹침으로 오인하지 않기 위한 여유 (tile 비례).
const EPS_FRAC: f32 = 1e-4;
/// 발밑 접지 판정 프로브 깊이 (tile 비례).
const GROUND_PROBE_FRAC: f32 = 0.02;

/// [`move_and_collide_2d`] 결과.
pub struct MoveResult2D {
    /// 해소된 위치 (`z`는 입력 그대로).
    pub pos: Vec3,
    /// 충돌한 축의 성분이 0으로 소거된 속도.
    pub vel: Vec3,
    /// 하향 접촉으로 정지했거나 발밑 프로브가 솔리드에 닿음.
    pub grounded: bool,
    /// X 축 충돌 발생.
    pub hit_wall: bool,
}

/// AABB(`half` = 반폭/반높이)를 `is_solid` 그리드에 대해 이동·해소한다.
pub fn move_and_collide_2d(
    is_solid: impl Fn(i32, i32) -> bool,
    tile: f32,
    pos: Vec3,
    vel: Vec3,
    half: [f32; 2],
    dt: f32,
) -> MoveResult2D {
    let eps = tile * EPS_FRAC;
    let mut p = pos;
    let mut v = vel;
    let mut hit_wall = false;
    let mut grounded = false;

    // ---- X 축
    if v.x != 0.0 {
        p.x += v.x * dt;
        if let Some((gx, _)) = first_solid_overlap(&is_solid, tile, eps, p, half) {
            if v.x > 0.0 {
                p.x = gx as f32 * tile - half[0];
            } else {
                p.x = (gx + 1) as f32 * tile + half[0];
            }
            v.x = 0.0;
            hit_wall = true;
        }
    }

    // ---- Y 축
    if v.y != 0.0 {
        p.y += v.y * dt;
        if let Some((_, gy)) = first_solid_overlap(&is_solid, tile, eps, p, half) {
            if v.y > 0.0 {
                p.y = gy as f32 * tile - half[1];
            } else {
                p.y = (gy + 1) as f32 * tile + half[1];
                grounded = true;
            }
            v.y = 0.0;
        }
    }

    // ---- 발밑 프로브 (수직 이동 없이도 접지 유지)
    if !grounded && v.y == 0.0 {
        let probe_y = p.y - half[1] - tile * GROUND_PROBE_FRAC;
        let gy = (probe_y / tile).floor() as i32;
        let gx0 = ((p.x - half[0] + eps) / tile).floor() as i32;
        let gx1 = ((p.x + half[0] - eps) / tile).floor() as i32;
        grounded = (gx0..=gx1).any(|gx| is_solid(gx, gy));
    }

    MoveResult2D { pos: p, vel: v, grounded, hit_wall }
}

/// AABB가 겹치는 셀 중 첫 솔리드 셀. 프레임 이동량 < tile 전제에서 겹칠 수
/// 있는 솔리드는 진행 방향 경계 셀뿐이라 순회 순서는 결과에 무관하다.
fn first_solid_overlap(
    is_solid: &impl Fn(i32, i32) -> bool,
    tile: f32,
    eps: f32,
    p: Vec3,
    half: [f32; 2],
) -> Option<(i32, i32)> {
    let gx0 = ((p.x - half[0] + eps) / tile).floor() as i32;
    let gx1 = ((p.x + half[0] - eps) / tile).floor() as i32;
    let gy0 = ((p.y - half[1] + eps) / tile).floor() as i32;
    let gy1 = ((p.y + half[1] - eps) / tile).floor() as i32;
    for gy in gy0..=gy1 {
        for gx in gx0..=gx1 {
            if is_solid(gx, gy) {
                return Some((gx, gy));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4×4 방: 바닥 gy0, 천장 gy3, 좌우 벽 gx0/gx5. 내부 1..=4 × 1..=2.
    fn room(gx: i32, gy: i32) -> bool {
        !(1..=4).contains(&gx) || !(1..=2).contains(&gy)
    }

    const HALF: [f32; 2] = [0.35, 0.45];
    const G: f32 = -30.0;

    #[test]
    fn falls_and_lands_on_floor() {
        let mut pos = Vec3::new(2.5, 2.4, 0.0);
        let mut vel = Vec3::ZERO;
        let mut grounded = false;
        for _ in 0..120 {
            vel.y += G * (1.0 / 60.0);
            let r = move_and_collide_2d(room, 1.0, pos, vel, HALF, 1.0 / 60.0);
            pos = r.pos;
            vel = r.vel;
            grounded = r.grounded;
        }
        assert!((pos.y - (1.0 + HALF[1])).abs() < 1e-3, "pos.y={}", pos.y);
        assert_eq!(vel.y, 0.0);
        assert!(grounded);
    }

    #[test]
    fn wall_blocks_horizontal_motion() {
        let r = move_and_collide_2d(
            room,
            1.0,
            Vec3::new(4.4, 1.5, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            HALF,
            0.1,
        );
        assert!((r.pos.x - (5.0 - HALF[0])).abs() < 1e-3);
        assert_eq!(r.vel.x, 0.0);
        assert!(r.hit_wall);
    }

    #[test]
    fn ceiling_stops_upward_velocity() {
        let r = move_and_collide_2d(
            room,
            1.0,
            Vec3::new(2.5, 2.4, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
            HALF,
            0.1,
        );
        assert!((r.pos.y - (3.0 - HALF[1])).abs() < 1e-3);
        assert_eq!(r.vel.y, 0.0);
        assert!(!r.grounded);
    }

    #[test]
    fn grounded_without_vertical_motion() {
        let standing = Vec3::new(2.5, 1.0 + HALF[1], 0.0);
        let r = move_and_collide_2d(room, 1.0, standing, Vec3::new(1.0, 0.0, 0.0), HALF, 0.016);
        assert!(r.grounded);
        assert!(!r.hit_wall);
    }

    #[test]
    fn boundary_policy_belongs_to_the_closure() {
        // open 경계: 그리드 밖 전부 빈칸 → 자유 낙하 계속.
        let open = |_: i32, _: i32| false;
        let r = move_and_collide_2d(open, 1.0, Vec3::ZERO, Vec3::new(0.0, -5.0, 0.0), HALF, 0.1);
        assert!(r.pos.y < 0.0);
        assert!(!r.grounded);
        // solid 경계: 전부 솔리드 취급 → 어느 방향도 못 움직임 (스냅 정지).
        let solid = |_: i32, _: i32| true;
        let r = move_and_collide_2d(
            solid,
            1.0,
            Vec3::new(0.5, 0.5, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
            [0.4, 0.4],
            0.01,
        );
        assert_eq!(r.vel.x, 0.0);
        assert!(r.hit_wall);
    }

    #[test]
    fn tile_size_scales_snapping() {
        // tile 2.0: 바닥 셀 gy0 상단 = y 2.0.
        let floor_only = |_: i32, gy: i32| gy < 1;
        let mut pos = Vec3::new(3.0, 5.0, 0.0);
        let mut vel = Vec3::ZERO;
        for _ in 0..120 {
            vel.y += G * (1.0 / 60.0);
            let r = move_and_collide_2d(floor_only, 2.0, pos, vel, HALF, 1.0 / 60.0);
            pos = r.pos;
            vel = r.vel;
        }
        assert!((pos.y - (2.0 + HALF[1])).abs() < 1e-3, "pos.y={}", pos.y);
    }
}
