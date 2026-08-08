//! 게임 컴포넌트 + 플레이어 플랫포머 컨트롤.

use crate::level::LevelMap;
use crate::physics2d::{move_and_collide, GRAVITY};
use voltex::prelude::*;

/// 플레이어 반폭/반높이 (AABB).
pub const PLAYER_HALF: [f32; 2] = [0.35, 0.45];
/// 수평 이동 속도 (유닛/s).
pub const PLAYER_SPEED: f32 = 5.0;
/// 점프 초기 상승 속도 (유닛/s).
pub const JUMP_VELOCITY: f32 = 12.0;

/// 플레이어 이동 상태. `facing`은 +1(우) / -1(좌).
#[derive(Debug, Clone, Copy)]
pub struct Player {
    pub vel: Vec3,
    pub grounded: bool,
    pub facing: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self { vel: Vec3::ZERO, grounded: false, facing: 1.0 }
    }
}

/// 좌우 순찰 적. `dir`은 +1/-1.
#[derive(Debug, Clone, Copy)]
pub struct Walker {
    pub dir: f32,
    pub speed: f32,
}

/// 수집 코인 마커.
#[derive(Debug, Clone, Copy)]
pub struct Coin;

/// 클리어 문 마커.
#[derive(Debug, Clone, Copy)]
pub struct DoorMarker;

/// 치명 가시 마커.
#[derive(Debug, Clone, Copy)]
pub struct Spike;

/// [`player_control`]이 보고하는 프레임 이벤트 (SFX/파티클 트리거용).
#[derive(Debug, Default, Clone, Copy)]
pub struct PlayerEvents {
    pub jumped: bool,
    pub landed: bool,
}

/// Tag "player" 엔티티에 입력을 적용해 이동/점프/중력을 처리한다.
/// `landed`는 grounded false→true 전이 프레임에 true.
pub fn player_control(
    map: &LevelMap,
    world: &mut World,
    move_x: f32,
    jump_pressed: bool,
    dt: f32,
) -> PlayerEvents {
    let mut events = PlayerEvents::default();
    let Some(entity) = world.find_by_tag("player") else {
        return events;
    };
    let (Some(mut player), Some(pos)) = (
        world.get::<Player>(entity).copied(),
        world.get::<Transform>(entity).map(|t| t.position),
    ) else {
        return events;
    };

    player.vel.x = move_x * PLAYER_SPEED;
    if move_x != 0.0 {
        player.facing = move_x.signum();
    }
    player.vel.y += GRAVITY * dt;
    if jump_pressed && player.grounded {
        player.vel.y = JUMP_VELOCITY;
        events.jumped = true;
    }

    let r = move_and_collide(map, pos, player.vel, PLAYER_HALF, dt);
    events.landed = !player.grounded && r.grounded;
    player.vel = r.vel;
    player.grounded = r.grounded;

    if let Some(t) = world.get_mut::<Transform>(entity) {
        t.position = r.pos;
    }
    if let Some(slot) = world.get_mut::<Player>(entity) {
        *slot = player;
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{cell_center, LevelMap};
    use voltex::ecs::Tag;

    const MAP: &[&str] = &[
        "......",
        "..P...",
        "######",
    ];

    fn setup() -> (LevelMap, World, Entity) {
        let m = LevelMap::parse_no_door(MAP).unwrap();
        let mut w = World::new();
        let p = w.spawn_bundle((
            Transform::from_position(cell_center(2, 1)),
            Player::default(),
            Tag("player".into()),
        ));
        (m, w, p)
    }

    #[test]
    fn walks_right_and_faces_direction() {
        let (m, mut w, p) = setup();
        for _ in 0..30 {
            player_control(&m, &mut w, 1.0, false, 1.0 / 60.0);
        }
        let t = w.get::<Transform>(p).unwrap();
        assert!(t.position.x > 2.5 + 1.0, "x={}", t.position.x);
        assert_eq!(w.get::<Player>(p).unwrap().facing, 1.0);
        player_control(&m, &mut w, -1.0, false, 1.0 / 60.0);
        assert_eq!(w.get::<Player>(p).unwrap().facing, -1.0);
    }

    #[test]
    fn jump_only_when_grounded_and_reports_events() {
        let (m, mut w, p) = setup();
        // 착지까지 안정화
        for _ in 0..60 {
            player_control(&m, &mut w, 0.0, false, 1.0 / 60.0);
        }
        assert!(w.get::<Player>(p).unwrap().grounded);
        let ev = player_control(&m, &mut w, 0.0, true, 1.0 / 60.0);
        assert!(ev.jumped);
        // 공중에서 재점프 불가
        let ev2 = player_control(&m, &mut w, 0.0, true, 1.0 / 60.0);
        assert!(!ev2.jumped);
        // 낙하 후 착지 이벤트
        let mut landed = false;
        for _ in 0..120 {
            landed |= player_control(&m, &mut w, 0.0, false, 1.0 / 60.0).landed;
        }
        assert!(landed);
        assert!(w.get::<Player>(p).unwrap().grounded);
    }
}
