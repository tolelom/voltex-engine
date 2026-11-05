//! voltex_capture — headless offscreen rendering + PNG capture for visual
//! verification.
//!
//! The engine's windowed examples can't be observed by automated tooling. This
//! crate renders the *real* GPU pipelines into an offscreen texture, reads the
//! pixels back, and writes a PNG — turning "does it actually draw?" into a
//! reproducible artifact.
//!
//! ```no_run
//! use voltex_capture::{HeadlessGpu, OffscreenTarget, save_png};
//! let gpu = HeadlessGpu::new().expect("gpu");
//! let target = OffscreenTarget::new(&gpu.device, 256, 256);
//! // ... render into target.color_view / target.depth_view ...
//! let pixels = target.read_rgba8(&gpu.device, &gpu.queue);
//! save_png("shot.png", target.width, target.height, &pixels).unwrap();
//! ```
#![warn(missing_docs)]

mod headless;
pub mod scene;
mod target;

pub use headless::HeadlessGpu;
pub use scene::{render_world, render_world_from, render_world_ortho, CameraPose, Projection};
pub use target::{OffscreenTarget, CAPTURE_FORMAT, DEPTH_FORMAT};
pub use voltex_image::encode_rgba8;

/// Encode an RGBA8 buffer to PNG and write it to `path`.
pub fn save_png(path: &str, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    let png = encode_rgba8(width, height, rgba);
    std::fs::write(path, png)
}

/// Environment variable that lets GPU tests rewrite the tracked reference
/// PNGs under `docs/visual/`. Unset, empty or `0` leaves them untouched.
pub const WRITE_SCREENSHOTS_ENV: &str = "VOLTEX_WRITE_SCREENSHOTS";

fn visual_record_enabled(value: Option<&str>) -> bool {
    matches!(value, Some(v) if !v.is_empty() && v != "0")
}

/// Write `rgba` to `docs/visual/<name>` when [`WRITE_SCREENSHOTS_ENV`] is set.
/// Rendering is not bit-identical across runs, so writing by default would
/// dirty the tracked PNGs on every GPU test run. A failed write is reported
/// on stderr and does not fail the caller.
pub fn write_visual_record(name: &str, width: u32, height: u32, rgba: &[u8]) {
    let value = std::env::var(WRITE_SCREENSHOTS_ENV).ok();
    if !visual_record_enabled(value.as_deref()) {
        return;
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/visual")
        .join(name);
    if let Err(e) = std::fs::write(&path, encode_rgba8(width, height, rgba)) {
        eprintln!("visual record write failed ({}): {e}", path.display());
    }
}

/// Summary statistics of an RGBA8 image, used to flag obviously-broken frames
/// (all black, single flat color, presence of fully-transparent or NaN-derived
/// pixels) without a human in the loop.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageStats {
    /// Number of pixels.
    pub pixels: usize,
    /// Fraction of pixels that are pure black (RGB all zero), 0.0..=1.0.
    pub black_fraction: f32,
    /// Number of distinct RGBA colors (capped internally for speed).
    pub distinct_colors: usize,
    /// Mean luminance across the image, 0.0..=1.0 (Rec. 601 weights).
    pub mean_luminance: f32,
}

impl ImageStats {
    /// Compute stats over a tightly-packed RGBA8 buffer.
    pub fn analyze(rgba: &[u8]) -> Self {
        let pixels = rgba.len() / 4;
        let mut black = 0usize;
        let mut lum_sum = 0.0f64;
        let mut colors = std::collections::HashSet::new();
        for px in rgba.as_chunks::<4>().0.iter() {
            let (r, g, b) = (px[0], px[1], px[2]);
            if r == 0 && g == 0 && b == 0 {
                black += 1;
            }
            lum_sum += 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            if colors.len() < 100_000 {
                colors.insert([px[0], px[1], px[2], px[3]]);
            }
        }
        Self {
            pixels,
            black_fraction: if pixels == 0 {
                0.0
            } else {
                black as f32 / pixels as f32
            },
            distinct_colors: colors.len(),
            mean_luminance: if pixels == 0 {
                0.0
            } else {
                (lum_sum / pixels as f64 / 255.0) as f32
            },
        }
    }

    /// Heuristic: a frame that is entirely one color is almost always a bug
    /// (cleared but nothing drawn, or a pipeline failure).
    pub fn looks_blank(&self) -> bool {
        self.distinct_colors <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_detect_blank_frame() {
        let img = vec![0u8; 4 * 16]; // 16 black pixels
        let s = ImageStats::analyze(&img);
        assert_eq!(s.pixels, 16);
        assert_eq!(s.black_fraction, 1.0);
        assert!(s.looks_blank());
        assert_eq!(s.mean_luminance, 0.0);
    }

    #[test]
    fn stats_detect_content() {
        let mut img = Vec::new();
        for i in 0..16u8 {
            img.extend_from_slice(&[i * 10, 255 - i * 10, i, 255]);
        }
        let s = ImageStats::analyze(&img);
        assert!(!s.looks_blank());
        assert!(s.distinct_colors > 1);
        assert!(s.mean_luminance > 0.0);
    }

    #[test]
    fn visual_record_writes_are_opt_in() {
        assert!(!visual_record_enabled(None));
        assert!(!visual_record_enabled(Some("")));
        assert!(!visual_record_enabled(Some("0")));
        assert!(visual_record_enabled(Some("1")));
        assert!(visual_record_enabled(Some("true")));
    }
}
