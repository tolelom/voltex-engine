//! 적 AI: region 활성화 → 추적(NavAgent)/사수/보스. 이동 적분은
//! nav_agent_system(kinematic)과 velocity_system(보스 돌진)이 담당.
use crate::combat::spawn_projectile;
use crate::components::*;
use crate::level::{try_move, LevelMap, PLAYER_Y};
use voltex::ai::NavAgent;
use voltex::prelude::*;

const SHOOTER_RANGE: f32 = 9.0;
/// 이 거리 이내에선 발사 억제 — 총구가 플레이어 안에서 터지는 그림 방지.
const SHOOTER_DEAD_ZONE: f32 = 1.5;

// NOTE: nav 경로 추적 중 적의 y는 waypoint(y=0)로 가라앉는다. follow_path의
// waypoint 도달 판정이 3D 거리라 여기서 y를 PLAYER_Y로 강제하면 중간 waypoint에
// 영원히 도달 못 함 — y 보정은 넣지 않는다. 최종 goal(p_pos)은 y=PLAYER_Y라
// 도달 시 복귀하고, 테스트 거리 단언은 양 끝점만 보므로 영향 없음.
pub fn enemy_ai_system(world: &mut World, map: &LevelMap, dt: f32, repath: bool) {
    let Some(player) = world.find_by_tag("player") else { return };
    let p_pos = match world.get::<Transform>(player) {
        Some(t) => t.position,
        None => return,
    };
    let p_region = map.region_of(p_pos);

    let ids: Vec<Entity> = world.query::<Enemy>().map(|(e, _)| e).collect();
    let mut shots: Vec<(Vec3, Vec3)> = Vec::new();
    let mut rings: Vec<Vec3> = Vec::new();
    for e in ids {
        let pos = match world.get::<Transform>(e) {
            Some(t) => t.position,
            None => continue,
        };
        let (kind, region) = match world.get::<Enemy>(e) {
            Some(en) => (en.kind, en.region),
            None => continue,
        };
        // region 활성화: 플레이어가 같은 region에 있을 때만.
        let active = p_region == Some(region);
        if let Some(en) = world.get_mut::<Enemy>(e) {
            en.touch.tick(dt);
            if !active {
                en.shoot.tick(dt); // 쿨다운은 흘려보냄
            }
        }
        if !active { continue; }
        match kind {
            EnemyKind::Chaser => {
                if repath {
                    // set_target: 캐시된 path/reached를 무효화해 실제 재경로.
                    // (target 필드 직접 대입은 path를 안 비워 재경로가 안 됨)
                    if let Some(a) = world.get_mut::<NavAgent>(e) {
                        a.set_target(p_pos);
                    }
                }
            }
            EnemyKind::Shooter => {
                let dist = (p_pos - pos).length();
                let sees = map.los_clear(pos, p_pos);
                if sees && dist <= SHOOTER_RANGE {
                    if let Some(a) = world.get_mut::<NavAgent>(e) {
                        a.clear_target();
                    }
                    let fire = world
                        .get_mut::<Enemy>(e)
                        .map(|en| en.shoot.tick(dt).just_finished())
                        .unwrap_or(false);
                    if fire && dist > SHOOTER_DEAD_ZONE {
                        shots.push((pos, p_pos - pos));
                    }
                } else {
                    if let Some(en) = world.get_mut::<Enemy>(e) {
                        en.shoot.tick(dt);
                    }
                    if repath {
                        if let Some(a) = world.get_mut::<NavAgent>(e) {
                            a.set_target(p_pos);
                        }
                    }
                }
            }
            EnemyKind::Boss => {
                let (fire, dash) = world
                    .get_mut::<Enemy>(e)
                    .map(|en| {
                        let f = en.shoot.tick(dt).just_finished();
                        let d = en.boss_dash_next;
                        if f {
                            en.boss_dash_next = !en.boss_dash_next;
                        }
                        (f, d)
                    })
                    .unwrap_or((false, false));
                if fire {
                    if dash {
                        let dir = (p_pos - pos).normalize_or_zero();
                        if let Some(v) = world.get_mut::<Velocity>(e) {
                            v.0 = dir * 12.0;
                        }
                    } else {
                        rings.push(pos);
                    }
                }
                if repath {
                    if let Some(a) = world.get_mut::<NavAgent>(e) {
                        a.set_target(p_pos);
                    }
                }
            }
        }
    }
    for (from, dir) in shots {
        spawn_projectile(world, Vec3::new(from.x, 0.6, from.z), dir, Team::Enemy, 1, 8.0);
    }
    for c in rings {
        for i in 0..12 {
            let a = i as f32 / 12.0 * std::f32::consts::TAU;
            spawn_projectile(
                world,
                Vec3::new(c.x, 0.6, c.z),
                Vec3::new(a.cos(), 0.0, a.sin()),
                Team::Enemy,
                1,
                7.0,
            );
        }
    }
}

/// Velocity를 가진 모든 Enemy를 적분 — 현재는 보스 dash만 0이 아니지만, 넉백 등 다른 시스템이 Velocity를 쓰면 그대로 적용됨.
/// 보스 돌진 적분 + 감쇠. 벽은 try_move로 슬라이드.
pub fn velocity_system(world: &mut World, map: &LevelMap, dt: f32) {
    let ids: Vec<(Entity, Vec3)> = world
        .query::<Enemy>()
        .filter_map(|(e, _)| {
            let v = world.get::<Velocity>(e)?.0;
            if v.length() < 0.05 {
                return None;
            }
            Some((e, v))
        })
        .collect();
    for (e, v) in ids {
        if let Some(t) = world.get_mut::<Transform>(e) {
            t.position = try_move(map, t.position, v * dt, 0.6);
            t.position.y = PLAYER_Y;
        }
        if let Some(vel) = world.get_mut::<Velocity>(e) {
            vel.0 = vel.0 * (1.0 - (4.0 * dt).min(1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::*;

    /// 기둥 뒤의 추적자가 NavMesh를 타고 플레이어에게 접근 — NavAgent 첫 실전 E2E.
    #[test]
    fn chaser_approaches_player_around_pillar() {
        let m = LevelMap::parse(&[
            "#######",
            "#P....#",
            "#.###.#",
            "#..c..#",
            "#######",
        ])
        .unwrap();
        let nav = build_navmesh(&m);
        let mut w = World::new();
        build_level(&mut w, &m);
        let player = w.find_by_tag("player").unwrap();
        let p_pos = w.get::<Transform>(player).unwrap().position;
        let enemy = w.query::<Enemy>().map(|(e, _)| e).next().unwrap();
        let d0 = (w.get::<Transform>(enemy).unwrap().position - p_pos).length();

        for _ in 0..240 {
            // 4초
            enemy_ai_system(&mut w, &m, dt_const(), true);
            voltex::ai::nav_agent_system(&mut w, &nav, dt_const());
        }
        let d1 = (w.get::<Transform>(enemy).unwrap().position - p_pos).length();
        assert!(d1 < d0 * 0.5, "chaser must close distance via navmesh: {d0} -> {d1}");
    }

    /// 다른 region의 적은 플레이어가 올 때까지 idle.
    #[test]
    fn enemy_in_other_region_stays_idle() {
        let m = LevelMap::parse(&[
            "#########",
            "#...#...#",
            "#.P.D.c.#",
            "#...#...#",
            "#########",
        ])
        .unwrap();
        let nav = build_navmesh(&m);
        let mut w = World::new();
        build_level(&mut w, &m);
        let enemy = w.query::<Enemy>().map(|(e, _)| e).next().unwrap();
        let before = w.get::<Transform>(enemy).unwrap().position;
        for _ in 0..120 {
            enemy_ai_system(&mut w, &m, dt_const(), true);
            voltex::ai::nav_agent_system(&mut w, &nav, dt_const());
        }
        let after = w.get::<Transform>(enemy).unwrap().position;
        assert!(
            (after - before).length() < 0.3,
            "enemy must idle until player enters its region"
        );
    }

    /// 사수: 같은 region + LOS + 사거리 안 → 멈추고 주기 발사 (적팀 투사체 생성).
    #[test]
    fn shooter_in_los_fires_enemy_projectiles() {
        let m = LevelMap::parse(&[
            "########",
            "#P...s.#",
            "########",
        ])
        .unwrap();
        let mut w = World::new();
        build_level(&mut w, &m);
        // 발사 주기 1.4s × 2회 이상 경과
        for _ in 0..180 {
            enemy_ai_system(&mut w, &m, dt_const(), false);
        }
        let projs: Vec<_> = w
            .query::<Projectile>()
            .filter(|(_, p)| p.team == Team::Enemy)
            .collect();
        assert!(projs.len() >= 2, "shooter must fire at least twice in 3s, got {}", projs.len());
    }

    /// 보스 탄막 패턴: 12방향 링 (dash와 교대).
    #[test]
    fn boss_alternates_dash_and_ring() {
        let m = LevelMap::parse(&[
            "########",
            "#P...B.#",
            "########",
        ])
        .unwrap();
        let mut w = World::new();
        build_level(&mut w, &m);
        let boss = w.query::<Enemy>().map(|(e, _)| e).next().unwrap();
        // 2.5s 경과 → 첫 패턴 = dash (boss_dash_next=true)
        for _ in 0..160 {
            enemy_ai_system(&mut w, &m, dt_const(), false);
        }
        let v = w.get::<Velocity>(boss).unwrap().0;
        assert!(v.length() > 1.0, "first pattern must be a dash burst, got v={v:?}");
        assert_eq!(w.query::<Projectile>().count(), 0, "no ring during dash pattern");
        // 다음 2.5s → 링 12발
        for _ in 0..150 {
            enemy_ai_system(&mut w, &m, dt_const(), false);
        }
        assert_eq!(
            w.query::<Projectile>().filter(|(_, p)| p.team == Team::Enemy).count(),
            12,
            "second pattern must be a 12-projectile ring"
        );
    }

    /// 보스 돌진 적분: 위치는 전진하고 속도는 감쇠한다 (정확 수치 아님, 행동 단언).
    #[test]
    fn velocity_system_integrates_and_damps() {
        let m = LevelMap::parse(&["#######", "#P....#", "#######"]).unwrap();
        let mut w = World::new();
        let start = m.world_pos(2, 1, PLAYER_Y);
        let boss = w.spawn_bundle((
            Transform::from_position(start),
            Enemy::new(EnemyKind::Boss, 0),
            Velocity(Vec3::new(12.0, 0.0, 0.0)),
        ));
        for _ in 0..60 {
            velocity_system(&mut w, &m, dt_const());
        }
        let t = w.get::<Transform>(boss).unwrap();
        assert!(t.position.x > start.x + 1.0, "boss must travel +x: {:?}", t.position);
        assert!((t.position.y - PLAYER_Y).abs() < 1e-5, "y stays at PLAYER_Y");
        let v = w.get::<Velocity>(boss).unwrap().0;
        assert!(v.length() < 1.0, "velocity must damp out, got {v:?}");
    }

    fn dt_const() -> f32 {
        1.0 / 60.0
    }
}
