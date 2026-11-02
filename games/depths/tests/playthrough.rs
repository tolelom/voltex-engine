//! GPU 없는 풀 플레이스루: 스크립트된 입력으로 던전 한 판을 끝까지 시뮬레이션.
//! RuntimeApp::tick_systems_only의 실제 실행 순서(collision_fn → user_systems)를
//! 따라 main.rs의 시스템들을 직접 조합한다.
use voltex::prelude::*;
use depths::{combat, components::*, game_state, level};

/// P·문·적이 모두 z=2 행에 정렬된 3방 맵 — 스크립트가 +x 직선 이동만으로
/// 문을 통과할 수 있다. (문 gate: 왼쪽 방=c, 가운데 방=s 클리어 시 개방)
const RUN_MAP: &[&str] = &[
    "###########",
    "#...#..#..#",
    "#P.cD.sD.B#",
    "#...#..#..#",
    "###########",
];

#[test]
fn full_playthrough_kill_unlock_boss_victory() {
    let source = level::LevelMap::parse(RUN_MAP).unwrap();
    let nav = level::build_navmesh(&source);
    let mut map = source.clone();
    let mut w = World::new();
    level::build_level(&mut w, &map);
    let player = w.find_by_tag("player").unwrap();

    let mut victory = false;
    for frame in 0..3600 {
        // 60초 상한
        let dt = 1.0 / 60.0;

        // 치트 입력: 12프레임마다 가장 가까운 적에게 발사
        if frame % 12 == 0 {
            let p = w.get::<Transform>(player).unwrap().position;
            let target = w
                .query::<Enemy>()
                .map(|(e, _)| w.get::<Transform>(e).unwrap().position)
                .min_by(|a, b| (*a - p).length().total_cmp(&(*b - p).length()));
            if let Some(t) = target {
                combat::spawn_projectile(
                    &mut w,
                    Vec3::new(p.x, 0.6, p.z),
                    t - p,
                    Team::Player,
                    1,
                    14.0,
                );
            }
        }

        // 플레이어 전진: +x로 걷기 (열린 문 통과)
        {
            let pos = w.get::<Transform>(player).unwrap().position;
            let next = level::try_move(&map, pos, Vec3::new(3.0 * dt, 0.0, 0.0), 0.45);
            w.get_mut::<Transform>(player).unwrap().position = next;
        }

        // === RuntimeApp::tick_systems_only 실순서 재현 ===
        // collision_fn은 user_systems보다 먼저 실행됨 (app.rs: physics/collision
        // → user_systems). 그 아래는 main.rs add_system 블록과 동일 순서.
        let contacts = voltex::physics::detect_collisions(&w);
        combat::handle_contacts(&contacts, &mut w);
        // main.rs repath = Timer::repeating(0.5) → frame 29에 첫 발화.
        let due = frame % 30 == 29;
        let m = map.clone();
        depths::enemy::enemy_ai_system(&mut w, &m, dt, due);
        voltex::ai::nav_agent_system(&mut w, &nav, dt);
        depths::enemy::velocity_system(&mut w, &m, dt);
        combat::projectile_system(&mut w, &m, dt);
        let out = combat::death_system(&mut w);
        game_state::door_system(&mut w, &mut map);
        voltex_runtime::update_lifetimes(&mut w, dt);

        // 1초마다 진행 스냅샷 — 실패 시에만 cargo가 출력해줌
        if frame % 60 == 0 {
            let p = w.get::<Transform>(player).unwrap().position;
            let hp = w.get::<Health>(player).map(|h| h.hp).unwrap_or(-1);
            let enemies: Vec<(EnemyKind, i32, Vec3)> = w
                .query::<Enemy>()
                .map(|(e, en)| {
                    (
                        en.kind,
                        w.get::<Health>(e).map(|h| h.hp).unwrap_or(-1),
                        w.get::<Transform>(e).unwrap().position,
                    )
                })
                .collect();
            println!(
                "t={:>2}s player=({:.1},{:.1}) hp={} doors={} enemies={:?}",
                frame / 60,
                p.x,
                p.z,
                hp,
                w.query::<Door>().count(),
                enemies
            );
        }

        if out.boss_died {
            victory = true;
            break;
        }
        assert!(
            !out.player_died,
            "player must survive this scripted run (frame {frame})"
        );
    }

    assert!(victory, "boss must die within 60 simulated seconds");
    assert_eq!(w.query::<Door>().count(), 0, "victory implies all doors opened");
}
