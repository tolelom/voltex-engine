//! GPU 시각 회귀: 던전 씬이 실제 픽셀로 렌더되는지 (RTX 4080에서 -- --ignored로 실행).
use voltex::prelude::*;
use voltex_capture::{HeadlessGpu, ImageStats};
use voltex_depths::level::{build_level, LevelMap};

#[test]
#[ignore] // GPU 필요: cargo test -p voltex_depths --test visual -- --ignored
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
    for p in rgba.chunks_exact(4) {
        let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
        if g > r + 30 && g > b + 30 {
            green = true;
        }
        if r > g + 30 && r > b + 30 {
            red = true;
        }
    }
    assert!(
        green && red,
        "player(green)/enemy(red) must be visible (green={green} red={red})"
    );
    // 스크린샷 산출물 (README/docs용)
    let png = voltex_capture::encode_rgba8(320, 240, &rgba);
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/visual/depths_dungeon.png"
        ),
        png,
    )
    .unwrap();
}
