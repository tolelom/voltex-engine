//! Render the demo scenes to PNG files for visual verification.
//!
//! Usage: `cargo run -p voltex_capture --bin capture [out_dir]`
//! (default out_dir: `target/captures`).

use voltex_capture::{save_png, scene, scene_ui, HeadlessGpu, ImageStats};

/// (name, render fn, width, height) for one capture.
type Shot = (&'static str, fn(&HeadlessGpu, u32, u32) -> Vec<u8>, u32, u32);

fn main() {
    let out_dir = std::env::args().nth(1).unwrap_or_else(|| "target/captures".to_string());
    std::fs::create_dir_all(&out_dir).expect("create out dir");

    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("No GPU available: {e}");
            std::process::exit(1);
        }
    };
    println!("adapter: {}", gpu.adapter_name);

    let shots: &[Shot] = &[
        ("single_cube", scene::render_single_cube, 512, 512),
        ("cube_grid", scene::render_cube_grid, 640, 480),
        ("pbr_grid", scene::render_pbr_grid, 768, 768),
        ("editor_ui", scene_ui::render_editor_ui, 640, 400),
    ];

    let mut failures = 0;
    for (name, f, w, h) in shots {
        let rgba = f(&gpu, *w, *h);
        let stats = ImageStats::analyze(&rgba);
        let path = format!("{out_dir}/{name}.png");
        save_png(&path, *w, *h, &rgba).expect("save png");
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
