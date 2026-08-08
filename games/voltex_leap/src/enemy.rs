//! 순찰 워커 AI + 플레이어 치명 접촉 판정.

use crate::components::{Player, Spike, Walker, PLAYER_HALF};
use crate::level::{grid_at, LevelMap};
use crate::physics2d::{move_and_collide, GRAVITY};
use voltex::prelude::*;

/// 워커 반폭/반높이.
pub const WALKER_HALF: [f32; 2] = [0.35, 0.35];
/// 가시 AABB 반높이 (셀 하단에 깔린 형태).
const SPIKE_HALF: [f32; 2] = [0.4, 0.3];

/// 워커를 dir 방향으로 순찰시킨다. 벽에 막히거나(전방 hit_wall) 다음
/// 발판이 비면(전방 아래 셀 !solid) dir 반전. 중력 적용.
pub fn walker_system(map: &LevelMap, world: &mut World, dt: f32) {
    let walkers: Vec<Entity> = world.query::<Walker>().map(|(e, _)| e).collect();
    for e in walkers {
        let (Some(mut walker), Some(pos)) = (
            world.get::<Walker>(e).copied(),
            world.get::<Transform>(e).map(|t| t.position),
        ) else {
            continue;
        };
        // 전방 발판 프로브 — 이동 전에 반전해 발판을 벗어나지 않는다.
        let front_x = pos.x + walker.dir * (WALKER_HALF[0] + 0.05);
        let (fgx, fgy) = grid_at(front_x, pos.y - WALKER_HALF[1] - 0.05);
        if !map.is_solid(fgx, fgy) {
            walker.dir = -walker.dir;
        }
        // 고정 하강 속도로 바닥에 밀착 (충돌 해소가 0으로 끊음 — 상태 저장 불요).
        let vel = Vec3::new(walker.dir * walker.speed, GRAVITY * dt, 0.0);
        let r = move_and_collide(map, pos, vel, WALKER_HALF, dt);
        if r.hit_wall {
            walker.dir = -walker.dir;
        }
        if let Some(t) = world.get_mut::<Transform>(e) {
            t.position = r.pos;
        }
        if let Some(slot) = world.get_mut::<Walker>(e) {
            *slot = walker;
        }
    }
}

fn aabb_overlap(a: Vec3, ah: [f32; 2], b: Vec3, bh: [f32; 2]) -> bool {
    (a.x - b.x).abs() < ah[0] + bh[0] && (a.y - b.y).abs() < ah[1] + bh[1]
}

/// 플레이어 AABB가 Walker 또는 Spike AABB와 겹치면 true.
pub fn lethal_contact(world: &World) -> bool {
    let Some(p) = world.find_by_tag("player") else {
        return false;
    };
    if world.get::<Player>(p).is_none() {
        return false;
    }
    let Some(ppos) = world.get::<Transform>(p).map(|t| t.position) else {
        return false;
    };
    let walker_hit = world.query::<Walker>().any(|(e, _)| {
        world
            .get::<Transform>(e)
            .is_some_and(|t| aabb_overlap(ppos, PLAYER_HALF, t.position, WALKER_HALF))
    });
    if walker_hit {
        return true;
    }
    world.query::<Spike>().any(|(e, _)| {
        world
            .get::<Transform>(e)
            .is_some_and(|t| aabb_overlap(ppos, PLAYER_HALF, t.position, SPIKE_HALF))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::*;
    use crate::level::{cell_center, LevelMap};
    use voltex::ecs::Tag;

    const MAP: &[&str] = &[
        "......",
        ".E....",
        "###...",
    ];

    #[test]
    fn walker_turns_at_ledge_and_wall() {
        let m = LevelMap::parse_no_door(MAP).unwrap();
        let mut w = World::new();
        let e = w.spawn_bundle((
            Transform::from_position(cell_center(1, 1)),
            Walker { dir: 1.0, speed: 2.0 },
        ));
        // 4초 순찰 — 낭떠러지(x=3)와 왼쪽 벽(x=0 맵 밖) 사이 왕복.
        for _ in 0..240 {
            walker_system(&m, &mut w, 1.0 / 60.0);
        }
        let x = w.get::<Transform>(e).unwrap().position.x;
        assert!(x > 0.0 && x < 3.0, "발판 위에 머물러야 함: x={x}");
        // 계속 움직인다 (어딘가 끼어 정지하지 않음)
        let before = x;
        for _ in 0..60 {
            walker_system(&m, &mut w, 1.0 / 60.0);
        }
        assert_ne!(before, w.get::<Transform>(e).unwrap().position.x);
    }

    #[test]
    fn player_dies_on_walker_or_spike_overlap() {
        let mut w = World::new();
        w.spawn_bundle((
            Transform::from_position(Vec3::new(2.0, 1.5, 0.0)),
            Player::default(),
            Tag("player".into()),
        ));
        assert!(!lethal_contact(&w));
        let walker = w.spawn_bundle((
            Transform::from_position(Vec3::new(2.3, 1.5, 0.0)),
            Walker { dir: 1.0, speed: 0.0 },
        ));
        assert!(lethal_contact(&w));
        w.despawn(walker);
        assert!(!lethal_contact(&w));
        // 가시: 셀 하단 기준 — 플레이어 발이 겹치면 사망
        w.spawn_bundle((
            Transform::from_position(Vec3::new(2.0, 1.2, 0.0)),
            Spike,
        ));
        assert!(lethal_contact(&w));
    }
}
