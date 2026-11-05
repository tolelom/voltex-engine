//! Render the demo scenes to PNG files for visual verification.
//!
//! Usage: `cargo run -p voltex_capture --bin capture [out_dir]`
//! (default out_dir: `target/captures`).

use voltex_capture::{save_png, scene, HeadlessGpu, ImageStats};

/// Scene renderer: `(gpu, width, height) -> RGBA8`.
type RenderFn = fn(&HeadlessGpu, u32, u32) -> Vec<u8>;

/// One capture: output file stem, renderer and requested size.
struct Shot {
    name: &'static str,
    render: RenderFn,
    width: u32,
    height: u32,
}

const fn shot(name: &'static str, render: RenderFn, width: u32, height: u32) -> Shot {
    Shot {
        name,
        render,
        width,
        height,
    }
}

/// Shrink `(width, height)` proportionally so the longer side is at most
/// `max_side`. Sizes already within the cap are returned unchanged.
fn fit_within(width: u32, height: u32, max_side: Option<u32>) -> (u32, u32) {
    match max_side {
        Some(cap) if width.max(height) > cap => {
            let scale = cap as f32 / width.max(height) as f32;
            (
                ((width as f32 * scale) as u32).max(1),
                ((height as f32 * scale) as u32).max(1),
            )
        }
        _ => (width, height),
    }
}

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/captures".to_string());
    // Optional second arg: cap the longest dimension (downscale the requested
    // sizes proportionally) to produce small reference thumbnails.
    let max_side: Option<u32> = std::env::args().nth(2).and_then(|s| s.parse().ok());
    std::fs::create_dir_all(&out_dir).expect("create out dir");

    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("No GPU available: {e}");
            std::process::exit(1);
        }
    };
    println!("adapter: {}", gpu.adapter_name);

    // 파일 이름(name.png)은 docs/visual 참조와 맞물려 있으니 바꾸지 말 것.
    let shots = [
        shot("single_cube", scene::render_single_cube, 512, 512),
        shot("cube_grid", scene::render_cube_grid, 640, 480),
        shot("declarative_primitives", scene::render_materialized_primitives, 512, 512),
        shot("boids_swarm", scene::render_boids_swarm, 512, 512),
        shot("survivor_seed", scene::render_survivor_seed_scene, 512, 320),
        shot("pbr_grid", scene::raw_pbr::render_pbr_grid, 768, 768),
        shot("shadow_scene", scene::raw_pbr::render_shadow_scene, 800, 600),
        shot("multi_light", scene::raw_pbr::render_multi_light, 800, 600),
        shot("gizmo_translate", scene::render_gizmo_translate, 480, 480),
        shot("gizmo_rotate", scene::render_gizmo_rotate, 480, 480),
        shot("gizmo_scale", scene::render_gizmo_scale, 480, 480),
        shot("editor_ui", scene::render_editor_ui, 640, 400),
        shot("skinned_rest", scene::render_skinned_rest, 384, 384),
        shot("skinned_bent", scene::render_skinned_bent_demo, 384, 384),
    ];

    let mut failures = 0;
    for shot in &shots {
        let (w, h) = fit_within(shot.width, shot.height, max_side);
        let rgba = (shot.render)(&gpu, w, h);
        let stats = ImageStats::analyze(&rgba);
        let path = format!("{out_dir}/{}.png", shot.name);
        save_png(&path, w, h, &rgba).expect("save png");
        let flag = if stats.looks_blank() { " [BLANK!]" } else { "" };
        println!(
            "{path}: {w}x{h} luminance={:.3} black={:.1}% colors={}{flag}",
            stats.mean_luminance,
            stats.black_fraction * 100.0,
            stats.distinct_colors,
        );
        if stats.looks_blank() {
            failures += 1;
        }
    }

    if failures > 0 {
        eprintln!("{failures} scene(s) rendered blank");
        std::process::exit(2);
    }
}
