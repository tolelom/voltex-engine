//! 절차 생성 SFX/BGM. `cargo run -p voltex_depths --bin gen_sfx` 로 assets/ 재생성.
use std::f32::consts::TAU;
use std::io::Write;

const RATE: u32 = 44_100;

fn write_wav(path: &str, samples: &[f32]) -> std::io::Result<()> {
    let n = samples.len() as u32 * 2;
    let mut f = std::fs::File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + n).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;            // PCM
    f.write_all(&1u16.to_le_bytes())?;            // mono
    f.write_all(&RATE.to_le_bytes())?;
    f.write_all(&(RATE * 2).to_le_bytes())?;      // byte rate
    f.write_all(&2u16.to_le_bytes())?;            // block align
    f.write_all(&16u16.to_le_bytes())?;           // bits
    f.write_all(b"data")?;
    f.write_all(&n.to_le_bytes())?;
    for s in samples {
        f.write_all(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes())?;
    }
    Ok(())
}

fn tone(freq: f32, secs: f32, decay: f32) -> Vec<f32> {
    (0..(RATE as f32 * secs) as usize).map(|i| {
        let t = i as f32 / RATE as f32;
        (t * freq * TAU).sin() * (-t * decay).exp() * 0.6
    }).collect()
}

fn main() -> std::io::Result<()> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
    write_wav(&format!("{dir}/sfx_shoot.wav"), &tone(880.0, 0.09, 35.0))?;
    write_wav(&format!("{dir}/sfx_hit.wav"), &tone(180.0, 0.15, 18.0))?;
    let mut clear = tone(523.0, 0.12, 8.0);
    clear.extend(tone(659.0, 0.12, 8.0));
    clear.extend(tone(784.0, 0.2, 6.0));
    write_wav(&format!("{dir}/sfx_clear.wav"), &clear)?;
    // BGM: 8초 4음 루프 (사인 + 3배음)
    let mut bgm = Vec::new();
    for &f in &[110.0f32, 138.6, 164.8, 138.6] {
        for i in 0..(RATE * 2) as usize {
            let t = i as f32 / RATE as f32;
            bgm.push(((t * f * TAU).sin() + 0.3 * (t * f * 3.0 * TAU).sin()) * 0.18);
        }
    }
    write_wav(&format!("{dir}/bgm.wav"), &bgm)?;
    println!("wrote 4 wav files to {dir}");
    Ok(())
}
