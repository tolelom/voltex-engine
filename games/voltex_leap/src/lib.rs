//! Voltex Leap — 2D 사이드뷰 플랫포머. 도그푸딩 게임 2편.
//! 설계: docs/superpowers/specs/2026-08-09-voltex-leap-game-design.md

pub mod components;
pub mod enemy;
pub mod game_state;
pub mod level;
pub mod physics2d;

/// 본편 레벨. main과 playthrough 테스트가 같은 맵을 쓴다 —
/// "이 레벨은 실제로 클리어 가능하다"가 테스트로 보증되도록.
pub const MAP: &[&str] = &[
    "................................................",
    "................................................",
    "................................................",
    "..........o................o....................",
    "................................................",
    "..P.........E..............E............o....D..",
    "######^^############..############^^############",
    "################################################",
];

#[cfg(test)]
mod asset_tests {
    /// gen_assets 산출물이 자체 PNG 디코더로 읽히고 기대 프레임 폭을 갖는지.
    /// (시트 재생성 시 프레임 수 변경이 조용히 깨지는 것 방지)
    #[test]
    fn generated_spritesheets_decode_with_expected_dims() {
        for (file, w) in [("player.png", 256u32), ("coin.png", 128)] {
            let path = format!("{}/assets/{file}", env!("CARGO_MANIFEST_DIR"));
            let bytes = std::fs::read(&path).unwrap_or_else(|e| {
                panic!("{path}: {e} — `cargo run -p voltex_leap --bin gen_assets` 먼저")
            });
            let (_, pw, ph) = voltex::image::parse_png(&bytes).expect(file);
            assert_eq!((pw, ph), (w, 32), "{file}");
        }
    }
}
