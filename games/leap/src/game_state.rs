//! phase 전이 + 레벨 스폰/재시작 + 코인/문 판정 + HUD.

use crate::components::{Coin, CoinCount, DoorMarker, Player, Spike, Walker};
use crate::enemy::{lethal_contact, walker_system};
use crate::level::{cell_center, LevelMap, TILE};
use voltex::ecs::Tag;
use voltex::editor::UiContext;
use voltex::prelude::*;

/// 게임 진행 상태.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GamePhase {
    Title,
    Playing,
    Dead,
    Clear,
}

/// 코인/문 상호작용 반경 (중심 거리).
const PICKUP_RADIUS: f32 = 0.6;
/// 워커 순찰 속도.
const WALKER_SPEED: f32 = 1.8;

/// 타일 회색.
pub const COLOR_TILE: [f32; 4] = [0.45, 0.45, 0.5, 1.0];
/// 가시 빨강.
pub const COLOR_SPIKE: [f32; 4] = [0.9, 0.2, 0.2, 1.0];
/// 코인 노랑.
pub const COLOR_COIN: [f32; 4] = [1.0, 0.85, 0.2, 1.0];
/// 문 초록.
pub const COLOR_DOOR: [f32; 4] = [0.2, 0.9, 0.4, 1.0];
/// 워커 보라.
pub const COLOR_WALKER: [f32; 4] = [0.7, 0.3, 0.9, 1.0];

/// [`playing_step`] 결과 — main이 SFX/파티클/phase 전환에 사용.
#[derive(Debug, Default, Clone, Copy)]
pub struct StepOutcome {
    pub coins_gained: u32,
    pub died: bool,
    pub cleared: bool,
}

/// world를 비우고 레벨을 스폰한다 (재시작 = 재호출).
/// 스프라이트는 솔리드 색 — 텍스처/애니메이션은 main이 스폰 후 입힌다.
pub fn spawn_level(world: &mut World, map: &LevelMap) {
    world.clear();
    // 솔리드 셀 전체를 Tilemap 하나로 — 셀 (gx,gy)가 [gx,gx+1]×[gy,gy+1]을
    // 덮는 규약이 cell_center와 같아 원점에 두면 그대로 겹친다.
    let mut tiles = Tilemap::solid(map.cols as u32, map.rows as u32, TILE, COLOR_TILE);
    for gy in 0..map.rows {
        for gx in 0..map.cols {
            if map.is_solid(gx as i32, gy as i32) {
                tiles.set(gx as u32, gy as u32, 1);
            }
        }
    }
    world.spawn_bundle((Transform::from_position(Vec3::ZERO), tiles));
    for &(gx, gy) in &map.spikes {
        let mut pos = cell_center(gx, gy);
        pos.y -= 0.2; // 셀 하단에 깔린 형태 (enemy::SPIKE_HALF와 정합)
        world.spawn_bundle((
            Transform::from_position(pos),
            Sprite::solid([0.8, 0.6], COLOR_SPIKE),
            Spike,
        ));
    }
    for &(gx, gy) in &map.coins {
        world.spawn_bundle((
            Transform::from_position(cell_center(gx, gy)),
            Sprite::solid([0.5, 0.5], COLOR_COIN),
            Coin,
        ));
    }
    for &(gx, gy) in &map.walkers {
        world.spawn_bundle((
            Transform::from_position(cell_center(gx, gy)),
            Sprite::solid([0.7, 0.7], COLOR_WALKER),
            Walker { dir: 1.0, speed: WALKER_SPEED },
        ));
    }
    world.spawn_bundle((
        Transform::from_position(cell_center(map.door.0, map.door.1)),
        Sprite::solid([0.9, 1.0], COLOR_DOOR),
        DoorMarker,
    ));
    world.spawn_bundle((
        Transform::from_position(cell_center(map.player_spawn.0, map.player_spawn.1)),
        Sprite::solid([0.7, 0.9], [1.0, 1.0, 1.0, 1.0]),
        Player::default(),
        CoinCount(0),
        Tag("player".into()),
    ));
}

/// Playing 한 스텝: 워커 이동 → 치명 접촉 → 코인 수집 → 문 도달.
/// 플레이어 이동은 여기 없음 — action system이 입력으로 수행 (depths 분리 미러).
pub fn playing_step(map: &LevelMap, world: &mut World, dt: f32) -> StepOutcome {
    let mut out = StepOutcome::default();
    walker_system(map, world, dt);
    if lethal_contact(world) {
        out.died = true;
        return out;
    }
    let Some(p) = world.find_by_tag("player") else {
        return out;
    };
    let Some(ppos) = world.get::<Transform>(p).map(|t| t.position) else {
        return out;
    };
    let near = |world: &World, e: Entity| {
        world.get::<Transform>(e).is_some_and(|t| {
            let (dx, dy) = (t.position.x - ppos.x, t.position.y - ppos.y);
            dx * dx + dy * dy < PICKUP_RADIUS * PICKUP_RADIUS
        })
    };
    let picked: Vec<Entity> = world
        .query::<Coin>()
        .map(|(e, _)| e)
        .filter(|&e| near(world, e))
        .collect();
    for c in &picked {
        world.despawn(*c);
    }
    out.coins_gained = picked.len() as u32;
    if out.coins_gained > 0 {
        if let Some(cc) = world.get_mut::<CoinCount>(p) {
            cc.0 += out.coins_gained;
        }
    }
    out.cleared = world
        .query::<DoorMarker>()
        .map(|(e, _)| e)
        .collect::<Vec<_>>()
        .into_iter()
        .any(|e| near(world, e));
    out
}

/// 인-윈도우 HUD: phase별 오버레이 + Playing 중 코인 수.
pub fn draw_hud(ui: &mut UiContext, world: &World, phase: GamePhase) {
    let (w, h) = (ui.screen_width, ui.screen_height);
    match phase {
        GamePhase::Title => {
            ui.draw_rect(0.0, 0.0, w, h, [12, 12, 24, 230]);
            ui.draw_text("VOLTEX LEAP", w / 2.0 - 100.0, h / 2.0 - 40.0, [240, 220, 120, 255]);
            ui.draw_text(
                "A/D move  SPACE jump  reach the green door",
                w / 2.0 - 190.0,
                h / 2.0,
                [200, 200, 210, 255],
            );
            ui.draw_text("SPACE to start", w / 2.0 - 70.0, h / 2.0 + 30.0, [140, 220, 140, 255]);
        }
        GamePhase::Playing => {
            let coins = world
                .find_by_tag("player")
                .and_then(|p| world.get::<CoinCount>(p).map(|c| c.0))
                .unwrap_or(0);
            ui.draw_text(&format!("COINS {coins}"), 16.0, 16.0, [240, 220, 120, 255]);
        }
        GamePhase::Dead => {
            ui.draw_rect(0.0, 0.0, w, h, [30, 5, 5, 200]);
            ui.draw_text("YOU DIED", w / 2.0 - 70.0, h / 2.0 - 12.0, [240, 80, 80, 255]);
            ui.draw_text("R to retry", w / 2.0 - 55.0, h / 2.0 + 16.0, [200, 200, 210, 255]);
        }
        GamePhase::Clear => {
            ui.draw_rect(0.0, 0.0, w, h, [5, 25, 10, 200]);
            ui.draw_text("CLEAR!", w / 2.0 - 50.0, h / 2.0 - 12.0, [120, 240, 140, 255]);
            ui.draw_text("R to restart", w / 2.0 - 62.0, h / 2.0 + 16.0, [200, 200, 210, 255]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &[&str] = &[
        "......",
        ".P.o.D",
        "######",
    ];

    #[test]
    fn spawn_populates_and_respawn_resets() {
        let m = LevelMap::parse(MAP).unwrap();
        let mut w = World::new();
        spawn_level(&mut w, &m);
        assert!(w.find_by_tag("player").is_some());
        assert_eq!(w.query::<Coin>().count(), 1);
        assert_eq!(w.query::<DoorMarker>().count(), 1);
        // 코인 하나 지우고 재스폰하면 복구
        let c = w.query::<Coin>().map(|(e, _)| e).next().unwrap();
        w.despawn(c);
        spawn_level(&mut w, &m);
        assert_eq!(w.query::<Coin>().count(), 1);
        // 솔리드 셀은 Tilemap 엔티티 하나로 (셀당 Sprite 아님 — friction-2 #5).
        let maps: Vec<_> = w.query::<Tilemap>().map(|(_, t)| t.clone()).collect();
        assert_eq!(maps.len(), 1, "exactly one tilemap after respawn");
        let solids = (0..m.rows)
            .flat_map(|gy| (0..m.cols).map(move |gx| (gx, gy)))
            .filter(|&(gx, gy)| m.is_solid(gx as i32, gy as i32))
            .count();
        assert_eq!(maps[0].filled_count(), solids);
        // 남은 Sprite = 코인 1 + 문 1 + 플레이어 1 (이 맵엔 가시·워커 없음)
        assert_eq!(w.query::<Sprite>().count(), 3);
    }

    #[test]
    fn coin_pickup_and_door_clear() {
        let m = LevelMap::parse(MAP).unwrap();
        let mut w = World::new();
        spawn_level(&mut w, &m);
        let p = w.find_by_tag("player").unwrap();
        // 코인 위로 순간이동 → 수집
        w.get_mut::<Transform>(p).unwrap().position = cell_center(3, 1);
        let out = playing_step(&m, &mut w, 1.0 / 60.0);
        assert_eq!(out.coins_gained, 1);
        assert!(!out.died && !out.cleared);
        assert_eq!(w.query::<Coin>().count(), 0);
        assert_eq!(w.get::<CoinCount>(p).unwrap().0, 1);
        // 문 위로 → 클리어
        w.get_mut::<Transform>(p).unwrap().position = cell_center(5, 1);
        assert!(playing_step(&m, &mut w, 1.0 / 60.0).cleared);
    }

    #[test]
    fn walker_contact_reports_death() {
        let m = LevelMap::parse(MAP).unwrap();
        let mut w = World::new();
        spawn_level(&mut w, &m);
        let p = w.find_by_tag("player").unwrap();
        let pos = w.get::<Transform>(p).unwrap().position;
        w.spawn_bundle((
            Transform::from_position(pos),
            Walker { dir: 1.0, speed: 0.0 },
        ));
        assert!(playing_step(&m, &mut w, 1.0 / 60.0).died);
    }
}
