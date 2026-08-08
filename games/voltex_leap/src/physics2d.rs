//! 중력 + AABB vs 타일 그리드 축 분리 충돌 해소.
//!
//! 전제: 한 프레임 이동량이 TILE 미만 (속도 × dt ≪ 1) — 스윕 생략.
//! X 이동→해소 후 Y 이동→해소. 관통 시 접촉면 스냅 + 해당 축 속도 0.

use crate::level::{LevelMap, TILE};
use voltex::prelude::Vec3;

/// 중력 가속 (월드 유닛/s²), -Y.
pub const GRAVITY: f32 = -30.0;

/// 셀 경계 flush 접촉을 겹침으로 오인하지 않기 위한 여유.
const EPS: f32 = 1e-4;
/// 발밑 접지 판정 프로브 깊이.
const GROUND_PROBE: f32 = 0.02;

/// [`move_and_collide`] 결과.
pub struct MoveResult {
    pub pos: Vec3,
    pub vel: Vec3,
    pub grounded: bool,
    pub hit_wall: bool,
}

/// AABB(`half` 반폭/반높이)를 타일 그리드에 대해 이동·해소한다.
pub fn move_and_collide(
    map: &LevelMap,
    pos: Vec3,
    vel: Vec3,
    half: [f32; 2],
    dt: f32,
) -> MoveResult {
    let mut p = pos;
    let mut v = vel;
    let mut hit_wall = false;
    let mut grounded = false;

    // ---- X 축
    if v.x != 0.0 {
        p.x += v.x * dt;
        if let Some((gx, _)) = first_solid_overlap(map, p, half) {
            if v.x > 0.0 {
                p.x = gx as f32 * TILE - half[0];
            } else {
                p.x = (gx + 1) as f32 * TILE + half[0];
            }
            v.x = 0.0;
            hit_wall = true;
        }
    }

    // ---- Y 축
    if v.y != 0.0 {
        p.y += v.y * dt;
        if let Some((_, gy)) = first_solid_overlap(map, p, half) {
            if v.y > 0.0 {
                p.y = gy as f32 * TILE - half[1];
            } else {
                p.y = (gy + 1) as f32 * TILE + half[1];
                grounded = true;
            }
            v.y = 0.0;
        }
    }

    // ---- 발밑 프로브 (수직 이동 없이도 접지 유지)
    if !grounded && v.y == 0.0 {
        let probe_y = p.y - half[1] - GROUND_PROBE;
        let gy = (probe_y / TILE).floor() as i32;
        let gx0 = ((p.x - half[0] + EPS) / TILE).floor() as i32;
        let gx1 = ((p.x + half[0] - EPS) / TILE).floor() as i32;
        grounded = (gx0..=gx1).any(|gx| map.is_solid(gx, gy));
    }

    MoveResult { pos: p, vel: v, grounded, hit_wall }
}

/// AABB가 겹치는 셀 중 첫 솔리드 셀. 이동 방향으로 스냅할 셀을 찾는 용도라
/// 이동축 기준 "가장 안쪽" 셀이 아니라 아무 솔리드나 돌려도 프레임 이동량 <
/// TILE 전제에서 동일하다 (겹칠 수 있는 솔리드가 진행 방향 경계 셀뿐).
fn first_solid_overlap(map: &LevelMap, p: Vec3, half: [f32; 2]) -> Option<(i32, i32)> {
    let gx0 = ((p.x - half[0] + EPS) / TILE).floor() as i32;
    let gx1 = ((p.x + half[0] - EPS) / TILE).floor() as i32;
    let gy0 = ((p.y - half[1] + EPS) / TILE).floor() as i32;
    let gy1 = ((p.y + half[1] - EPS) / TILE).floor() as i32;
    for gy in gy0..=gy1 {
        for gx in gx0..=gx1 {
            if map.is_solid(gx, gy) {
                return Some((gx, gy));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::LevelMap;
    use voltex::prelude::Vec3;

    const MAP: &[&str] = &[
        "######",
        "#....#",
        "#.P..#",
        "######",
    ];
    fn map() -> LevelMap {
        LevelMap::parse_no_door(MAP).unwrap()
    }

    const HALF: [f32; 2] = [0.35, 0.45];

    #[test]
    fn falls_and_lands_on_floor() {
        let m = map();
        let mut pos = Vec3::new(2.5, 2.4, 0.0);
        let mut vel = Vec3::ZERO;
        let mut grounded = false;
        for _ in 0..120 {
            vel.y += GRAVITY * (1.0 / 60.0);
            let r = move_and_collide(&m, pos, vel, HALF, 1.0 / 60.0);
            pos = r.pos;
            vel = r.vel;
            grounded = r.grounded;
        }
        // 바닥 윗면 y=1.0 위에 반높이만큼 떠서 정지.
        assert!((pos.y - (1.0 + HALF[1])).abs() < 1e-3, "pos.y={}", pos.y);
        assert_eq!(vel.y, 0.0);
        assert!(grounded);
    }

    #[test]
    fn wall_blocks_horizontal_motion() {
        let m = map();
        let r = move_and_collide(
            &m,
            Vec3::new(4.4, 1.5, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            HALF,
            0.1,
        );
        assert!((r.pos.x - (5.0 - HALF[0])).abs() < 1e-3, "pos.x={}", r.pos.x);
        assert_eq!(r.vel.x, 0.0);
        assert!(r.hit_wall);
    }

    #[test]
    fn ceiling_stops_upward_velocity() {
        let m = map();
        let r = move_and_collide(
            &m,
            Vec3::new(2.5, 2.4, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
            HALF,
            0.1,
        );
        // 천장(gy 3) 아랫면 y=3.0 아래에 반높이만큼.
        assert!((r.pos.y - (3.0 - HALF[1])).abs() < 1e-3, "pos.y={}", r.pos.y);
        assert_eq!(r.vel.y, 0.0);
        assert!(!r.grounded);
    }

    #[test]
    fn grounded_without_vertical_motion() {
        let m = map();
        let standing = Vec3::new(2.5, 1.0 + HALF[1], 0.0);
        let r = move_and_collide(&m, standing, Vec3::new(1.0, 0.0, 0.0), HALF, 0.016);
        assert!(r.grounded);
        assert!(!r.hit_wall);
    }
}
