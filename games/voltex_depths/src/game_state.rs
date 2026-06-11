//! 문 개방 / 시작·재시작. (HUD는 Task 13)
use crate::components::*;
use crate::level::{build_level, LevelMap};
use voltex::prelude::*;

/// 매 프레임 호출: EnemyKilled 이벤트를 소비해 RegionCounts 갱신,
/// count가 0이 된 gate region의 문을 개방(despawn + map.open_door).
#[allow(dead_code)] // 호출자: 게임 루프 (후속 태스크)
pub fn door_system(world: &mut World, map: &mut LevelMap) {
    // 1) 이번 프레임 kill 이벤트 소비 → RegionCounts 갱신
    let kills = world.drain_events::<EnemyKilled>();
    for k in kills {
        if let Some(rc) = world.resource_mut::<RegionCounts>() {
            if let Some(n) = rc.0.get_mut(k.region) {
                *n = n.saturating_sub(1);
            }
        }
    }

    // 2) gate region count == 0인 문 수집 (borrow 분리)
    let counts = match world.resource::<RegionCounts>() {
        Some(rc) => rc.0.clone(),
        None => return,
    };
    let open: Vec<(Entity, (usize, usize))> = world
        .query::<Door>()
        .filter(|(_, d)| counts.get(d.gate_region).copied().unwrap_or(0) == 0)
        .map(|(e, d)| (e, d.cell))
        .collect();

    // 3) 해당 문 개방
    for (e, cell) in open {
        voltex::ecs::despawn_recursive(world, e);
        map.open_door(cell);
        if let Some(s) = world.resource_mut::<Score>() {
            s.0 += 250;
        }
    }
}

/// Playing 진입: 월드 리셋 + 레벨/리소스 재구성. (NavMesh는 불변 — 재빌드 불필요)
#[allow(dead_code)] // 호출자: 게임 루프 (후속 태스크)
pub fn start_game(world: &mut World, map: &mut LevelMap, source: &LevelMap) {
    world.clear();
    *map = source.clone();
    build_level(world, map);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::*;

    #[test]
    fn door_opens_when_gate_region_cleared() {
        let m = LevelMap::parse(&[
            "#########",
            "#...#...#",
            "#.P.D.c.#",
            "#...#...#",
            "#########",
        ])
        .unwrap();
        // 적은 오른쪽 region — 문 gate는 왼쪽(시작, 적 0) region이므로 즉시 열려야 함
        let mut map = m.clone();
        let mut w = World::new();
        build_level(&mut w, &map);
        door_system(&mut w, &mut map);
        assert_eq!(w.query::<Door>().count(), 0, "gate region(시작방, 적 0)이 빈 문은 즉시 개방");
        assert!(!map.blocked_at(map.world_pos(4, 2, 0.5)));
    }

    #[test]
    fn door_with_enemies_in_gate_waits_for_kill_events() {
        let m = LevelMap::parse(&[
            "###########",
            "#P.c#..#B.#",
            "#...D..D..#",
            "###########",
        ])
        .unwrap();
        let mut map = m.clone();
        let mut w = World::new();
        build_level(&mut w, &map);
        let first_door_cell = map.doors[0];
        door_system(&mut w, &mut map);
        assert!(
            map.blocked_at(map.world_pos(first_door_cell.0, first_door_cell.1, 0.5)),
            "첫 문은 c 생존 동안 닫힘"
        );
        let r = map.region_at((3, 1)).unwrap();
        w.send_event(EnemyKilled { region: r });
        door_system(&mut w, &mut map);
        assert!(!map.blocked_at(map.world_pos(
            first_door_cell.0,
            first_door_cell.1,
            0.5
        )));
    }

    #[test]
    fn start_game_resets_world_and_map() {
        let source = LevelMap::parse(&[
            "#########",
            "#...#...#",
            "#.P.D.c.#",
            "#...#...#",
            "#########",
        ])
        .unwrap();
        let mut map = source.clone();
        let mut w = World::new();
        build_level(&mut w, &map);
        // 진행 흉내: 문 개방 + 적 제거 + 더미 엔티티
        map.open_door((4, 2));
        let enemy = w.query::<Enemy>().map(|(e, _)| e).next().unwrap();
        voltex::ecs::despawn_recursive(&mut w, enemy);
        // 리셋
        start_game(&mut w, &mut map, &source);
        assert_eq!(w.query::<Enemy>().count(), 1, "enemy reseeded");
        assert!(w.find_by_tag("player").is_some());
        assert!(
            map.blocked_at(map.world_pos(4, 2, 0.5)),
            "door closed again"
        );
        assert_eq!(w.resource::<Score>().unwrap().0, 0, "score reset");
        assert_eq!(
            w.resource::<RegionCounts>().unwrap().0.iter().sum::<u32>(),
            1
        );
    }
}
