//! 전투: 조준 unprojection, 투사체, 피격/사망.
use voltex::prelude::*;

// ── Task 8: 마우스 조준 unprojection ──────────────────────────────────────

/// 화면 픽셀 (mx, my) → NDC (x, y).
/// NDC x: [-1, 1] (좌 → 우), y: [-1, 1] (아래 → 위).
pub fn ndc_from_mouse(mx: f64, my: f64, w: f32, h: f32) -> (f32, f32) {
    ((2.0 * mx as f32 / w) - 1.0, 1.0 - (2.0 * my as f32 / h))
}

/// NDC 좌표 → 월드 레이 → y=plane_y 평면 교점.
/// 레이가 평면과 평행하거나 교점이 카메라 뒤면 None.
pub fn unproject_to_plane(vp_inv: &Mat4, ndc: (f32, f32), plane_y: f32) -> Option<Vec3> {
    let homo_div = |v: Vec4| -> Option<Vec3> {
        if v.w.abs() < 1e-5 {
            return None;
        }
        Some(Vec3::new(v.x / v.w, v.y / v.w, v.z / v.w))
    };
    // wgpu NDC z: near=0.0, far=1.0
    let near = homo_div(vp_inv.mul_vec4(Vec4::new(ndc.0, ndc.1, 0.0, 1.0)))?;
    let far = homo_div(vp_inv.mul_vec4(Vec4::new(ndc.0, ndc.1, 1.0, 1.0)))?;
    let dir = far - near;
    if dir.y.abs() < 1e-6 {
        return None;
    }
    let t = (plane_y - near.y) / dir.y;
    if t < 0.0 {
        return None;
    }
    Some(near + dir * t)
}

// ── Task 9: 투사체 스폰/이동/벽 소멸 ─────────────────────────────────────

use voltex::physics::Collider;
use voltex_runtime::Lifetime;

use crate::components::{Enemy, EnemyKind, EnemyKilled, Health, Projectile, Score, Team, Velocity};
use crate::level::LevelMap;

pub fn spawn_projectile(
    world: &mut World,
    from: Vec3,
    dir: Vec3,
    team: Team,
    damage: i32,
    speed: f32,
) {
    let d = dir.normalize_or_zero();
    if d.x == 0.0 && d.z == 0.0 {
        return;
    }
    let color = match team {
        Team::Player => [1.0, 0.95, 0.35, 1.0],
        Team::Enemy => [0.95, 0.30, 0.60, 1.0],
    };
    world.spawn_bundle((
        Transform::from_position(from),
        PrimitiveMesh::sphere_default(0.18),
        MaterialDesc::color(color),
        Projectile { team, damage },
        Velocity(d * speed),
        Collider::Sphere { radius: 0.25 },
        Lifetime::new(3.0),
    ));
}

/// 투사체 이동 + 벽 셀 진입 시 즉시 소멸 (Lifetime은 안전망).
pub fn projectile_system(world: &mut World, map: &LevelMap, dt: f32) {
    // 스냅샷 패턴: query 중 world.get_mut 빌림 충돌 방지
    let moves: Vec<(Entity, Vec3)> = world
        .query::<Projectile>()
        .filter_map(|(e, _)| {
            let v = world.get::<Velocity>(e)?.0;
            Some((e, v * dt))
        })
        .collect();

    let mut to_kill: Vec<Entity> = Vec::new();
    for (e, dp) in moves {
        if let Some(t) = world.get_mut::<Transform>(e) {
            t.position = t.position + dp;
            if map.blocked_at(t.position) {
                to_kill.push(e);
            }
        }
    }
    for e in to_kill {
        voltex::ecs::despawn_recursive(world, e);
    }
}

/// 원샷 효과음: AudioSource 엔티티 스폰 → audio_sync가 재생, Lifetime이 정리.
/// `clip`은 main의 with_audio_files 목록 인덱스 (CLIP_* 상수).
pub fn play_sfx(world: &mut World, clip: u32, pos: Vec3) {
    let mut src = voltex::audio::AudioSource::new(clip);
    src.playing = true;
    src.volume = 0.8;
    world.spawn_bundle((Transform::from_position(pos), src, Lifetime::new(1.5)));
}

// ── Task 10: 접촉 데미지 매트릭스 + 사망 처리 ────────────────────────────

/// 투사체×엔티티 / Enemy 접촉 데미지 처리.
/// touch 쿨다운 tick은 적 AI 시스템(Task 11)이 담당 — 여기선 finished 검사/리셋만.
pub fn handle_contacts(contacts: &[voltex::physics::ContactPoint], world: &mut World) {
    use voltex::ecs::Tag;

    for c in contacts {
        // (x)가 (y)를 타격하는 방향 양쪽 시도
        for &(x, y) in &[(c.entity_a, c.entity_b), (c.entity_b, c.entity_a)] {
            if !world.is_alive(x) || !world.is_alive(y) {
                continue;
            }

            // ── 투사체가 x일 경우 ──
            if let Some(p) = world.get::<Projectile>(x).copied() {
                let hit_enemy =
                    p.team == Team::Player && world.get::<Enemy>(y).is_some();
                let hit_player = p.team == Team::Enemy
                    && world
                        .get::<Tag>(y)
                        .map(|t| t.0 == "player")
                        .unwrap_or(false);
                if hit_enemy || hit_player {
                    if let Some(h) = world.get_mut::<Health>(y) {
                        h.hp -= p.damage;
                    }
                    if world.is_alive(x) {
                        voltex::ecs::despawn_recursive(world, x);
                    }
                    break; // 이 (x, y) 쌍 처리 완료
                }
            }

            // ── Enemy 접촉 데미지 (Chaser/Boss × player) ──
            let is_player = world
                .get::<Tag>(y)
                .map(|t| t.0 == "player")
                .unwrap_or(false);
            if is_player {
                let ready = world
                    .get::<Enemy>(x)
                    .map(|e| e.kind != EnemyKind::Shooter && e.touch.finished())
                    .unwrap_or(false);
                if ready {
                    if let Some(h) = world.get_mut::<Health>(y) {
                        h.hp -= 1;
                    }
                    if let Some(e) = world.get_mut::<Enemy>(x) {
                        e.touch.reset();
                    }
                }
            }
        }
    }
}

// 동시 true 가능. 호출부는 boss_died를 먼저 검사해야 Victory 우선이 됨.
pub struct DeathOutcome {
    pub player_died: bool,
    pub boss_died: bool,
}

pub fn death_system(world: &mut World) -> DeathOutcome {
    use voltex::ecs::Tag;

    let mut out = DeathOutcome {
        player_died: false,
        boss_died: false,
    };

    let dead: Vec<Entity> = world
        .query::<Health>()
        .filter(|(_, h)| h.hp <= 0)
        .map(|(e, _)| e)
        .collect();

    let mut killed: Vec<EnemyKilled> = Vec::new();
    for e in dead {
        if let Some(en) = world.get::<Enemy>(e) {
            if en.kind == EnemyKind::Boss {
                out.boss_died = true;
            }
            killed.push(EnemyKilled { region: en.region });
            voltex::ecs::despawn_recursive(world, e);
        } else if world
            .get::<Tag>(e)
            .map(|t| t.0 == "player")
            .unwrap_or(false)
        {
            out.player_died = true;
            // despawn 안 함 — phase 전환이 처리
        }
    }

    for k in killed {
        if let Some(s) = world.resource_mut::<Score>() {
            s.0 += 100;
        }
        world.send_event(k);
    }

    out
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Task 8 ──

    #[test]
    fn ndc_maps_screen_corners() {
        assert_eq!(ndc_from_mouse(0.0, 0.0, 1280.0, 720.0), (-1.0, 1.0));
        let (cx, cy) = ndc_from_mouse(640.0, 360.0, 1280.0, 720.0);
        assert!(cx.abs() < 1e-6 && cy.abs() < 1e-6);
    }

    #[test]
    fn unproject_center_hits_look_target() {
        let eye = Vec3::new(0.0, 10.0, 5.0);
        let view = Mat4::look_at(eye, Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let proj = Mat4::perspective(45f32.to_radians(), 16.0 / 9.0, 0.1, 100.0);
        // view_projection() = proj * view (camera.rs:101)
        let vp = proj * view;
        let inv = vp.inverse().unwrap();
        let hit = unproject_to_plane(&inv, (0.0, 0.0), 0.5).unwrap();
        assert!(
            hit.x.abs() < 0.1 && hit.z.abs() < 0.1,
            "center must hit look target: {hit:?}"
        );
        let right = unproject_to_plane(&inv, (0.5, 0.0), 0.5).unwrap();
        assert!(right.x > hit.x);
    }

    // ── Task 9 ──

    #[test]
    fn projectile_moves_and_dies_on_wall() {
        let map =
            crate::level::LevelMap::parse(&["#####", "#P..#", "#####"]).unwrap();
        let mut w = World::new();
        let from = map.world_pos(1, 1, 0.6);
        spawn_projectile(
            &mut w,
            from,
            Vec3::new(1.0, 0.0, 0.0),
            crate::components::Team::Player,
            1,
            14.0,
        );
        let e = w
            .query::<crate::components::Projectile>()
            .map(|(e, _)| e)
            .next()
            .unwrap();
        projectile_system(&mut w, &map, 0.1);
        let after = w.get::<Transform>(e).unwrap().position;
        assert!(after.x > from.x, "projectile must advance +x");
        for _ in 0..40 {
            projectile_system(&mut w, &map, 0.1);
        }
        assert!(!w.is_alive(e), "projectile must despawn on wall");
    }

    // ── Task 10 ──

    use voltex::physics::ContactPoint;
    use crate::components::{EnemyKind, Health, Projectile, Team};

    fn contact(a: Entity, b: Entity) -> ContactPoint {
        ContactPoint {
            entity_a: a,
            entity_b: b,
            normal: Vec3::new(0.0, 1.0, 0.0),
            depth: 0.1,
            point_on_a: Vec3::ZERO,
            point_on_b: Vec3::ZERO,
        }
    }

    #[test]
    fn player_projectile_damages_enemy_and_despawns() {
        let mut w = World::new();
        let enemy = w.spawn_bundle((
            Transform::new(),
            crate::components::Enemy::new(EnemyKind::Chaser, 0),
            Health { hp: 2 },
        ));
        let proj = w.spawn_bundle((
            Transform::new(),
            Projectile {
                team: Team::Player,
                damage: 1,
            },
            Velocity(Vec3::ZERO),
        ));
        handle_contacts(&[contact(proj, enemy)], &mut w);
        assert_eq!(w.get::<Health>(enemy).unwrap().hp, 1);
        assert!(!w.is_alive(proj));
    }

    #[test]
    fn chaser_touch_damages_player_once_per_cooldown() {
        let mut w = World::new();
        let player = w.spawn_bundle((
            Transform::new(),
            voltex::ecs::Tag("player".into()),
            Health { hp: 5 },
        ));
        let enemy = w.spawn_bundle((
            Transform::new(),
            crate::components::Enemy::new(EnemyKind::Chaser, 0),
            Health { hp: 2 },
        ));
        handle_contacts(&[contact(enemy, player)], &mut w);
        handle_contacts(&[contact(enemy, player)], &mut w);
        assert_eq!(
            w.get::<Health>(player).unwrap().hp,
            4,
            "touch damage must respect cooldown"
        );
    }

    #[test]
    fn touch_damage_survives_interleaved_ticks() {
        // 접촉 전에 AI tick이 여러 번 끼어도 첫 접촉은 즉시 데미지,
        // 이후 쿨다운 중엔 무시, 0.8s 경과 후 다시 데미지.
        // (Repeating finished()==just_finished 의존 회귀 방지 — Once sticky 필요)
        let mut w = World::new();
        let player = w.spawn_bundle((
            Transform::new(),
            voltex::ecs::Tag("player".into()),
            Health { hp: 5 },
        ));
        let enemy = w.spawn_bundle((
            Transform::new(),
            crate::components::Enemy::new(EnemyKind::Chaser, 0),
            Health { hp: 2 },
        ));
        // 스폰 후 AI가 먼저 여러 프레임 tick (접촉 없음)
        for _ in 0..10 {
            if let Some(e) = w.get_mut::<crate::components::Enemy>(enemy) {
                e.touch.tick(1.0 / 60.0);
            }
        }
        handle_contacts(&[contact(enemy, player)], &mut w);
        assert_eq!(
            w.get::<Health>(player).unwrap().hp,
            4,
            "first touch must damage immediately even after prior ticks"
        );
        // 쿨다운 중
        for _ in 0..10 {
            if let Some(e) = w.get_mut::<crate::components::Enemy>(enemy) {
                e.touch.tick(1.0 / 60.0);
            }
        }
        handle_contacts(&[contact(enemy, player)], &mut w);
        assert_eq!(w.get::<Health>(player).unwrap().hp, 4, "cooldown must block");
        // 0.8s 경과
        for _ in 0..60 {
            if let Some(e) = w.get_mut::<crate::components::Enemy>(enemy) {
                e.touch.tick(1.0 / 60.0);
            }
        }
        handle_contacts(&[contact(enemy, player)], &mut w);
        assert_eq!(
            w.get::<Health>(player).unwrap().hp,
            3,
            "after cooldown damage again"
        );
    }

    #[test]
    fn death_sweep_despawns_sends_event_and_reports() {
        let mut w = World::new();
        // player(hp5) + chaser(hp0, region1) + boss(hp0, region0)
        w.spawn_bundle((
            Transform::new(),
            voltex::ecs::Tag("player".into()),
            Health { hp: 5 },
        ));
        w.spawn_bundle((
            Transform::new(),
            crate::components::Enemy::new(EnemyKind::Chaser, 1),
            Health { hp: 0 },
        ));
        w.spawn_bundle((
            Transform::new(),
            crate::components::Enemy::new(EnemyKind::Boss, 0),
            Health { hp: 0 },
        ));
        w.insert_resource(crate::components::Score(0));
        let out = death_system(&mut w);
        assert!(out.boss_died);
        assert!(!out.player_died);
        let killed = w.drain_events::<crate::components::EnemyKilled>();
        assert_eq!(killed.len(), 2);
        assert_eq!(w.query::<crate::components::Enemy>().count(), 0);
        assert_eq!(w.resource::<crate::components::Score>().unwrap().0, 200);
    }
}
