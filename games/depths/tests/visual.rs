//! GPU 시각 회귀: 던전 씬이 실제 픽셀로 렌더되는지 (RTX 4080에서 -- --ignored로 실행).
use voltex::prelude::*;
use voltex_capture::{HeadlessGpu, ImageStats};
use depths::level::{build_level, LevelMap};

#[test]
#[ignore] // GPU 필요: cargo test -p depths --test visual -- --ignored
fn dungeon_scene_renders_walls_player_enemies() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let m = LevelMap::parse(&[
        "#########",
        "#...#...#",
        "#.P.D.c.#",
        "#...#...#",
        "#########",
    ])
    .unwrap();
    let mut w = World::new();
    build_level(&mut w, &m);
    let rgba = voltex_capture::render_world(
        &gpu,
        320,
        240,
        &mut w,
        Vec3::new(0.0, 16.0, 9.0),
        0.0,
        -1.05,
    );
    let stats = ImageStats::analyze(&rgba);
    eprintln!("dungeon frame stats: {stats:?}");
    assert!(!stats.looks_blank(), "dungeon frame blank: {stats:?}");
    let (mut green, mut red) = (false, false);
    for p in rgba.as_chunks::<4>().0 {
        let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
        // 비율 기반 — 셰이딩 스케일 무관, 문 주황 false-positive 배제.
        // 주의: 프레임버퍼는 sRGB라 선형 비율이 감마 압축됨 (비율^(1/2.4)).
        //   적   [0.90,0.25,0.20] r/g 선형 3.6 → 감마 후 실측 1.93
        //   플레이어 [0.20,0.85,0.35] g/r 1.98, g/b 1.5 (실측)
        //   문   [0.85,0.65,0.20] r/g 선형 1.3 → 감마 후 ≈1.12 (배제 대상)
        if g > 60 && g as f32 > r as f32 * 1.5 && g as f32 > b as f32 * 1.25 {
            green = true;
        }
        if r > 60 && r as f32 > g as f32 * 1.5 && r as f32 > b as f32 * 1.5 {
            red = true;
        }
    }
    assert!(
        green && red,
        "player(green)/enemy(red) must be visible (green={green} red={red})"
    );
    // 스크린샷 산출물 (README/docs용) — 저장 실패는 픽셀 단언과 무관하므로 비치명.
    let png = voltex_capture::encode_rgba8(320, 240, &rgba);
    if let Err(e) = std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/visual/depths_dungeon.png"
        ),
        png,
    ) {
        eprintln!("screenshot write failed: {e}");
    }
}
