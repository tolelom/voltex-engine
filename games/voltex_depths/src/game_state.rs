//! 문 개방 / 시작·재시작 / HUD.
use crate::components::*;
use crate::level::{build_level, LevelMap};
use voltex::editor::UiContext;
use voltex::prelude::*;

/// 매 프레임 호출: EnemyKilled 이벤트를 소비해 RegionCounts 갱신,
/// count가 0이 된 gate region의 문을 개방(despawn + map.open_door).
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
pub fn start_game(world: &mut World, map: &mut LevelMap, source: &LevelMap) {
    world.clear();
    *map = source.clone();
    build_level(world, map);
}

/// 인-윈도우 HUD: phase별 오버레이 + Playing 중 HP/점수.
pub fn draw_hud(ui: &mut UiContext, world: &World, phase: GamePhase) {
    let (w, h) = (ui.screen_width, ui.screen_height);
    match phase {
        GamePhase::Title => {
            ui.draw_rect(0.0, 0.0, w, h, [10, 10, 20, 230]);
            ui.draw_text("VOLTEX DEPTHS", w / 2.0 - 120.0, h / 2.0 - 40.0, [240, 220, 120, 255]);
            ui.draw_text(
                "WASD move / mouse aim+fire",
                w / 2.0 - 140.0,
                h / 2.0,
                [200, 200, 200, 255],
            );
            ui.draw_text(
                "press SPACE to start",
                w / 2.0 - 110.0,
                h / 2.0 + 24.0,
                [120, 230, 140, 255],
            );
        }
        GamePhase::Playing => {
            let hp = world
                .find_by_tag("player")
                .and_then(|p| world.get::<Health>(p))
                .map(|h| h.hp)
                .unwrap_or(0);
            let score = world.resource::<Score>().map(|s| s.0).unwrap_or(0);
            ui.draw_text(&format!("HP {hp}"), 16.0, 16.0, [240, 90, 90, 255]);
            ui.draw_text(&format!("SCORE {score}"), 16.0, 36.0, [240, 220, 120, 255]);
        }
        GamePhase::GameOver => {
            ui.draw_rect(0.0, 0.0, w, h, [30, 5, 5, 200]);
            ui.draw_text("YOU DIED", w / 2.0 - 70.0, h / 2.0 - 12.0, [240, 80, 80, 255]);
            ui.draw_text(
                "press R to restart",
                w / 2.0 - 100.0,
                h / 2.0 + 16.0,
                [200, 200, 200, 255],
            );
        }
        GamePhase::Victory => {
            ui.draw_rect(0.0, 0.0, w, h, [5, 25, 10, 200]);
            ui.draw_text(
                "DUNGEON CLEARED!",
                w / 2.0 - 110.0,
                h / 2.0 - 12.0,
                [120, 240, 140, 255],
            );
            ui.draw_text(
                "press R for title",
                w / 2.0 - 95.0,
                h / 2.0 + 16.0,
                [200, 200, 200, 255],
            );
        }
    }
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
        // gate는 BFS depth 계산 결과 — 플레이어 시작 region에서 가까운 쪽이 gate.
        // 이 맵에선 왼쪽(시작) 방이 gate. 적은 오른쪽 region뿐이므로
        // gate(시작 방, 적 0)가 빈 문은 즉시 열려야 함.
        let mut map = m.clone();
        let mut w = World::new();
        build_level(&mut w, &map);
        door_system(&mut w, &mut map);
        assert_eq!(w.query::<Door>().count(), 0, "gate region(시작방, 적 0)이 빈 문은 즉시 개방");
        assert!(!map.blocked_at(map.world_pos(4, 2, 0.5)));
    }

    #[test]
    fn door_with_enemies_in_gate_waits_for_kill_events() {
        // 가운데 방에도 c를 둬서 doors[1]의 gate(가운데 방) count가 0이 아니게 함
        // — 첫 kill 후 doors[1]이 여전히 닫혀 있음을 단언하기 위한 전제.
        let m = LevelMap::parse(&[
            "###########",
            "#P.c#.c#B.#",
            "#...D..D..#",
            "###########",
        ])
        .unwrap();
        let mut map = m.clone();
        let mut w = World::new();
        build_level(&mut w, &map);
        let first_door_cell = map.doors[0];
        let second_door_cell = map.doors[1];
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
        assert!(
            map.blocked_at(map.world_pos(second_door_cell.0, second_door_cell.1, 0.5)),
            "두 번째 문은 가운데 방 c 생존 동안 여전히 닫힘"
        );
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
        w.resource_mut::<Score>().unwrap().0 = 999; // 더티 상태 — 리셋이 0으로 되돌리는지
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
