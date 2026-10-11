//! Mutation fuzz for the WAV parser: real-world fixtures and generated
//! 16/24/32-bit files are corrupted thousands of times. Audio files are
//! untrusted assets, so `parse_wav` must return `Err` or a consistent clip —
//! never panic or abort.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use voltex_audio::parse_wav;

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
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
            // Target the header area, where sizes and format fields live.
            for _ in 0..1 + rng.next() % 4 {
                let i = rng.next() as usize % b.len().min(64);
                b[i] = rng.next() as u8;
            }
        }
        2 => b.truncate(rng.next() as usize % b.len()),
        _ => {
            let at = rng.next() as usize % b.len().min(64);
            let v = [0u8, 0xFF, 0x7F, 0x80][rng.next() as usize % 4];
            for k in 0..4.min(b.len() - at) {
                b[at + k] = v;
            }
        }
    }
    b
}

#[test]
fn wav_parser_never_panics_on_corrupted_files() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut inputs: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()))
        .collect();
    let tone: Vec<f32> = (0..400).map(|i| (i as f32 * 0.1).sin()).collect();
    inputs.push(("gen16".into(), voltex_audio::generate_wav_bytes(&tone, 22_050)));
    inputs.push(("gen24".into(), voltex_audio::wav::generate_wav_bytes_24bit(&tone, 22_050)));
    inputs.push(("gen32f".into(), voltex_audio::wav::generate_wav_bytes_f32(&tone, 22_050)));
    inputs.sort();

    let mut rng = XorShift(0xA0D1_0F22);
    for (name, data) in inputs {
        for i in 0..1500 {
            let input = mutate(&mut rng, &data);
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                if let Ok(clip) = parse_wav(&input) {
                    assert!(clip.channels > 0, "accepted a 0-channel clip");
                    assert_eq!(clip.samples.len() % clip.channels as usize, 0, "partial frame");
                    assert!(clip.samples.iter().all(|s| s.is_finite()), "non-finite sample");
                }
            }));
            assert!(outcome.is_ok(), "{name} mutation {i} panicked parse_wav ({} bytes)", input.len());
        }
    }
}
