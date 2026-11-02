//! 게임 물리 튜닝 + `voltex::physics::grid2d` 어댑터.
//!
//! 충돌 코어는 v1.0.113에서 `voltex_physics::grid2d`로 승격됨.
//! 여기엔 게임 소유 상수(중력)와 LevelMap 어댑터만
//! 남는다. 동작 동일성은 이 파일의 기존 테스트 4개가 회귀 가드.

use crate::level::{LevelMap, TILE};
use voltex::physics::{move_and_collide_2d, MoveResult2D};
use voltex::prelude::Vec3;

/// 중력 가속 (월드 유닛/s²), -Y. 게임 튜닝 값 — 엔진 소유 아님.
pub const GRAVITY: f32 = -30.0;

/// [`move_and_collide`] 결과 (grid2d 결과의 재export).
pub type MoveResult = MoveResult2D;

/// AABB(`half` 반폭/반높이)를 레벨 타일 그리드에 대해 이동·해소한다.
/// 경계 정책은 [`LevelMap::is_solid`] 소유 (밖 x 솔리드, 밖 y 빈칸).
pub fn move_and_collide(
    map: &LevelMap,
    pos: Vec3,
    vel: Vec3,
    half: [f32; 2],
    dt: f32,
) -> MoveResult {
    move_and_collide_2d(|gx, gy| map.is_solid(gx, gy), TILE, pos, vel, half, dt)
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
