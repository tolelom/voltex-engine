//! Mutation fuzz for the in-house TrueType parser + rasterizer. Fonts are
//! user-supplied files (the editor loads any `.ttf`), so a corrupted font
//! must yield `Err` from `TtfFont::new` or glyphs that rasterize without
//! panicking. Seeds from a system font when one exists, otherwise from the
//! minimal in-memory test font.

use std::panic::{catch_unwind, AssertUnwindSafe};
use voltex_editor::TtfFont;

#[path = "../src/test_font.rs"]
mod test_font;

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

fn seed_font() -> Vec<u8> {
    ["C:/Windows/Fonts/arial.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/Library/Fonts/Arial.ttf"]
        .iter()
        .find_map(|p| std::fs::read(p).ok())
        .unwrap_or_else(test_font::minimal_ttf)
}

#[test]
fn ttf_loader_never_panics_on_corrupted_fonts() {
    let font = seed_font();
    let mut rng = XorShift(0x77F0_0001);
    let iterations: u32 = std::env::var("VOLTEX_TTF_FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
    for i in 0..iterations {
        let mut b = font.clone();
        match rng.next() % 3 {
            // Header + table directory: offsets and lengths.
            0 => {
                for _ in 0..1 + rng.next() % 4 {
                    let at = rng.next() as usize % 512.min(b.len());
                    b[at] = rng.next() as u8;
                }
            }
            1 => {
                for _ in 0..1 + rng.next() % 16 {
                    let at = rng.next() as usize % b.len();
                    b[at] = rng.next() as u8;
                }
            }
            _ => b.truncate(rng.next() as usize % b.len()),
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(mut f) = TtfFont::new(&b, 16.0) {
                for ch in ['A', 'g', '?', '@', 'é', '가', ' '] {
                    let _ = f.glyph(ch);
                }
                let _ = f.text_width("Hello, wörld");
            }
        }));
        if outcome.is_err() {
            let dump = std::env::temp_dir().join(format!("voltex_ttf_fuzz_{i}.ttf"));
            let _ = std::fs::write(&dump, &b);
            panic!("mutation {i} panicked the TTF loader (saved to {})", dump.display());
        }
    }
}
