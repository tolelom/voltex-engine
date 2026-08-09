//! 헤드리스 풀 플레이스루 — main의 실제 tick 순서(player_control →
//! playing_step)를 미러한 봇이 본편 MAP을 완주한다.

use voltex::ecs::Tag;
use voltex::prelude::*;
use voltex_leap::components::*;
use voltex_leap::enemy::WALKER_HALF;
use voltex_leap::game_state::{self, StepOutcome};
use voltex_leap::level::{grid_at, LevelMap};
use voltex_leap::MAP;

const DT: f32 = 1.0 / 60.0;

/// main action system과 같은 순서로 한 프레임 진행.
fn tick(map: &LevelMap, world: &mut World, move_x: f32, jump: bool) -> StepOutcome {
    player_control(map, world, move_x, jump, DT);
    game_state::playing_step(map, world, DT)
}

fn player_state(world: &World) -> (Vec3, Player) {
    let p = world.find_by_tag("player").expect("player alive");
    (
        world.get::<Transform>(p).unwrap().position,
        *world.get::<Player>(p).unwrap(),
    )
}

/// 우측 진행 봇의 점프 판단: 접지 상태에서 전방에 갭/벽/가시/워커가 보이면 점프.
fn should_jump(map: &LevelMap, world: &World, pos: Vec3, player: &Player) -> bool {
    if !player.grounded {
        return false;
    }
    // 전방 발판 갭
    let (ggx, ggy) = grid_at(pos.x + 0.9, pos.y - PLAYER_HALF[1] - 0.3);
    if !map.is_solid(ggx, ggy) {
        return true;
    }
    // 전방 벽
    let (wgx, wgy) = grid_at(pos.x + 0.7, pos.y);
    if map.is_solid(wgx, wgy) {
        return true;
    }
    // 전방 가시/워커
    let hazard_ahead = |t: &Transform, reach_x: f32, reach_y: f32| {
        let dx = t.position.x - pos.x;
        dx > 0.0 && dx < reach_x && (t.position.y - pos.y).abs() < reach_y
    };
    let spike = world.query::<Spike>().any(|(e, _)| {
        world
            .get::<Transform>(e)
            .is_some_and(|t| hazard_ahead(t, 1.6, 1.5))
    });
    if spike {
        return true;
    }
    world.query::<Walker>().any(|(e, _)| {
        world
            .get::<Transform>(e)
            .is_some_and(|t| hazard_ahead(t, 1.8, 1.0 + WALKER_HALF[1]))
    })
}

#[test]
fn full_clear_run_collects_a_coin() {
    let map = LevelMap::parse(MAP).unwrap();
    let mut world = World::new();
    game_state::spawn_level(&mut world, &map);
    let mut cleared = false;
    let mut coins = 0u32;
    for frame in 0..60 * 60 {
        let (pos, player) = player_state(&world);
        let jump = should_jump(&map, &world, pos, &player);
        let out = tick(&map, &mut world, 1.0, jump);
        coins += out.coins_gained;
        assert!(
            !out.died,
            "봇 사망 @frame {frame} pos=({:.2},{:.2})",
            pos.x, pos.y
        );
        if out.cleared {
            cleared = true;
            break;
        }
    }
    assert!(cleared, "60초 내 문 도달 실패");
    assert!(coins >= 1, "경로상 코인 최소 1개 수집");
}

#[test]
fn walking_into_spike_pit_dies_then_restart_resets() {
    let map = LevelMap::parse(MAP).unwrap();
    let mut world = World::new();
    game_state::spawn_level(&mut world, &map);
    let total_coins = world.query::<Coin>().count();

    // 점프 없이 우측 직진 → 첫 가시 갭(cols 6-7)에 낙하 → 사망.
    let mut died = false;
    for _ in 0..60 * 10 {
        let out = tick(&map, &mut world, 1.0, false);
        if out.coins_gained > 0 {
            // 코인을 주웠어도 계속 진행 (경로상 없음이 정상이지만 무해)
        }
        if out.died {
            died = true;
            break;
        }
    }
    assert!(died, "가시 구간에서 사망해야 함");

    // 재시작 = spawn_level 재호출: 코인/워커/플레이어 전부 리셋.
    game_state::spawn_level(&mut world, &map);
    assert_eq!(world.query::<Coin>().count(), total_coins);
    assert_eq!(world.query::<Walker>().count(), map.walkers.len());
    let p = world.find_by_tag("player").unwrap();
    assert_eq!(world.get::<CoinCount>(p).unwrap().0, 0);
    let pos = world.get::<Transform>(p).unwrap().position;
    let spawn = voltex_leap::level::cell_center(map.player_spawn.0, map.player_spawn.1);
    assert!((pos.x - spawn.x).abs() < 1e-6 && (pos.y - spawn.y).abs() < 1e-6);
    let _ = world.query::<Tag>(); // Tag 쿼리 가능 확인 (플레이어 태그 유지)
}
