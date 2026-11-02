//! XY 평면 AABB vs 타일 그리드 축 분리(X→Y) 이동·해소.
//!
//! 타일 기반 2D 게임(플랫포머/탑다운) 공통 캐릭터 이동 코어.
//! `is_solid(gx, gy)` 클로저가 그리드를 정의하므로 맵 타입에 비의존이고,
//! **경계 정책(맵 밖 solid/open)은 호출자 소유**다.
//!
//! 전제: 한 프레임 이동량이 `tile` 미만 (스윕 없음 — 속도 × dt ≪ tile).
//! 관통 시 접촉면에 스냅 + 해당 축 속도 0. 중력/점프 같은 튜닝 상수는
//! 게임 영역 (leap `physics2d.rs`에서 승격 — dogfood-friction-2 #4).

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
///
/// 한 호출의 이동량이 tile을 넘으면(히칭 프레임, 고속 투사체) 내부적으로
/// 서브스텝으로 쪼개 해소한다 — 호출자는 dt/속도 크기를 신경 쓸 필요 없음.
pub fn move_and_collide_2d(
    is_solid: impl Fn(i32, i32) -> bool,
    tile: f32,
    pos: Vec3,
    vel: Vec3,
    half: [f32; 2],
    dt: f32,
) -> MoveResult2D {
    debug_assert!(tile > 0.0, "grid2d: tile must be positive");
    let tile = tile.max(f32::MIN_POSITIVE);
    // 서브스텝당 축 이동량을 tile의 절반 이하로 — 셀 하나를 건너뛰는 일이
    // 없도록 (충돌 시 축 속도가 0이 되므로 이후 서브스텝은 자연히 저렴).
    let max_axis_move = vel.x.abs().max(vel.y.abs()) * dt;
    let steps = ((max_axis_move / (tile * 0.5)).ceil() as u32).max(1);
    let sub_dt = dt / steps as f32;
    let mut r = MoveResult2D { pos, vel, grounded: false, hit_wall: false };
    for _ in 0..steps {
        let hit_wall_so_far = r.hit_wall;
        r = step(&is_solid, tile, r.pos, r.vel, half, sub_dt);
        r.hit_wall |= hit_wall_so_far;
        if r.vel.x == 0.0 && r.vel.y == 0.0 {
            break; // 양축 정지 — 남은 서브스텝은 이동 없음
        }
    }
    r
}

/// 단일 스텝 해소 (이동량 < tile 전제는 호출자 [`move_and_collide_2d`]가 보장).
fn step(
    is_solid: &impl Fn(i32, i32) -> bool,
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
    fn fast_motion_does_not_tunnel_through_walls() {
        // 한 스텝 이동량이 벽 두께(1 tile)를 훌쩍 넘는 속도 — 히칭 프레임
        // (dt 0.25 클램프)이나 고속 투사체 시나리오. 내부 서브스텝이 없으면
        // AABB가 벽 반대편에 착지해 overlap 자체가 없고 그냥 통과한다.
        let r = move_and_collide_2d(
            room,
            1.0,
            Vec3::new(2.5, 1.5, 0.0),
            Vec3::new(200.0, 0.0, 0.0),
            HALF,
            0.1, // 이동량 20 tile
        );
        assert!((r.pos.x - (5.0 - HALF[0])).abs() < 1e-3, "pos.x={}", r.pos.x);
        assert_eq!(r.vel.x, 0.0);
        assert!(r.hit_wall);

        // 고속 낙하도 바닥에서 멈춘다.
        let r = move_and_collide_2d(
            room,
            1.0,
            Vec3::new(2.5, 2.4, 0.0),
            Vec3::new(0.0, -200.0, 0.0),
            HALF,
            0.1,
        );
        assert!((r.pos.y - (1.0 + HALF[1])).abs() < 1e-3, "pos.y={}", r.pos.y);
        assert!(r.grounded);
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
