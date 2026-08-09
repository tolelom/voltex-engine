//! GPU 시각 회귀: 2D 레벨이 실제 픽셀로 렌더되는지 (GPU 머신에서 -- --ignored).
//!
//! `render_world`는 perspective 고정 계약(FOV 45°)이라 게임의 직교 카메라를
//! 재현할 수 없다 — dogfood-friction-2 로그 대상. 여기선 perspective 원거리
//! 샷으로 색 비율만 단언한다.
use voltex::prelude::*;
use voltex_capture::{HeadlessGpu, ImageStats};
use voltex_leap::game_state::spawn_level;
use voltex_leap::level::LevelMap;
use voltex_leap::MAP;

#[test]
#[ignore] // GPU 필요: cargo test -p voltex_leap --test visual -- --ignored
fn level_scene_renders_tiles_spikes_door_coins() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let map = LevelMap::parse(MAP).unwrap();
    let mut world = World::new();
    spawn_level(&mut world, &map);
    // 레벨 전폭(48유닛)이 잡히게 뒤로 뺀 perspective 샷 (yaw 0 → -Z 방향).
    let rgba = voltex_capture::render_world(
        &gpu,
        640,
        360,
        &mut world,
        Vec3::new(24.0, 4.0, 34.0),
        0.0,
        0.0,
    );
    let stats = ImageStats::analyze(&rgba);
    eprintln!("leap frame stats: {stats:?}");
    assert!(!stats.looks_blank(), "leap frame blank: {stats:?}");

    // 비율 기반 색 존재 단언 (sRGB 감마로 선형 비율이 압축됨 — depths 미러).
    //   가시   [0.90,0.20,0.20] → r 우세
    //   문     [0.20,0.90,0.40] → g 우세
    //   코인   [1.00,0.85,0.20] → r≈g, b 열세
    let (mut red, mut green, mut gold) = (0u32, 0u32, 0u32);
    for p in rgba.chunks_exact(4) {
        let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
        if r > 60.0 && r > g * 1.5 && r > b * 1.5 {
            red += 1;
        }
        if g > 60.0 && g > r * 1.5 && g > b * 1.25 {
            green += 1;
        }
        if r > 80.0 && g > 70.0 && (r - g).abs() < r * 0.35 && b * 1.4 < g {
            gold += 1;
        }
    }
    eprintln!("leap color pixels: red={red} green={green} gold={gold}");
    assert!(red >= 30, "spike red pixels: {red}");
    assert!(green >= 30, "door green pixels: {green}");
    assert!(gold >= 20, "coin gold pixels: {gold}");

    // 스크린샷 산출물 (docs용) — 저장 실패는 비치명.
    let png = voltex_capture::encode_rgba8(640, 360, &rgba);
    if let Err(e) = std::fs::write(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/leap_level.png"),
        png,
    ) {
        eprintln!("screenshot write failed: {e}");
    }
}
