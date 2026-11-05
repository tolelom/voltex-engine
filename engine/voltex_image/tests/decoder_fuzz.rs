//! Mutation fuzz for the image decoders: every real PNG / JPEG / deflate
//! fixture is corrupted (byte overwrites, bit flips, truncation, chunk
//! duplication) thousands of times. Decoders read untrusted asset files, so
//! they must return `Err` (or a well-formed image) — never panic.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use voltex_image::{decode_jpg, decode_png, deflate};

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

fn fixtures(ext: &str) -> Vec<(String, Vec<u8>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()))
        .collect();
    out.sort();
    assert!(!out.is_empty(), "no .{ext} fixtures");
    out
}

fn mutate(rng: &mut XorShift, src: &[u8]) -> Vec<u8> {
    let mut b = src.to_vec();
    match rng.next() % 4 {
        0 => {
            for _ in 0..1 + rng.next() % 8 {
                let i = rng.next() as usize % b.len();
                b[i] = rng.next() as u8;
            }
        }
        1 => {
            for _ in 0..1 + rng.next() % 8 {
                let i = rng.next() as usize % b.len();
                b[i] ^= 1 << (rng.next() % 8);
            }
        }
        2 => b.truncate(rng.next() as usize % b.len()),
        _ => {
            let start = rng.next() as usize % b.len();
            let len = (rng.next() as usize % 64).min(b.len() - start);
            let chunk = b[start..start + len].to_vec();
            let at = rng.next() as usize % b.len();
            b.splice(at..at, chunk);
        }
    }
    b
}

fn fuzz(ext: &str, iterations: u32, decode: impl Fn(&[u8])) {
    let mut rng = XorShift(0xF022_1A6E);
    for (name, data) in fixtures(ext) {
        for i in 0..iterations {
            let input = mutate(&mut rng, &data);
            let result = catch_unwind(AssertUnwindSafe(|| decode(&input)));
            if result.is_err() {
                let dump = std::env::temp_dir().join(format!("voltex_fuzz_{name}_{i}.bin"));
                let _ = std::fs::write(&dump, &input);
                panic!("{name} mutation {i} panicked the decoder (input saved to {})", dump.display());
            }
        }
    }
}

#[test]
fn png_decoder_never_panics_on_corrupted_files() {
    fuzz("png", 400, |b| {
        if let Ok(img) = decode_png(b) {
            assert_eq!(img.rgba.len(), img.width as usize * img.height as usize * 4);
        }
    });
}

#[test]
fn jpeg_decoder_never_panics_on_corrupted_files() {
    fuzz("jpg", 400, |b| {
        if let Ok(img) = decode_jpg(b) {
            assert_eq!(img.rgba.len(), img.width as usize * img.height as usize * 4);
        }
    });
}

#[test]
fn inflate_never_panics_on_corrupted_streams() {
    fuzz("zz", 2000, |b| {
        let _ = deflate::inflate(b);
    });
}
