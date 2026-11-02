//! 절차 생성 에셋: SFX/BGM wav + 스프라이트시트 PNG.
//! `cargo run -p leap --bin gen_assets` 로 assets/ 재생성.
//!
//! 플레이어는 단일 8×1 시트 — idle 0-1 / run 2-5 / jump 6 / 여분 7.
//! 클립 선택은 `SpriteAnimation::clip`의 first_frame (friction-2 #1 해소 검증).
use std::f32::consts::TAU;
use std::io::Write;

const RATE: u32 = 44_100;
const FRAME: usize = 32;

// ---- WAV ----

fn write_wav(path: &str, samples: &[f32]) -> std::io::Result<()> {
    let n = samples.len() as u32 * 2;
    let mut f = std::fs::File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + n).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&RATE.to_le_bytes())?;
    f.write_all(&(RATE * 2).to_le_bytes())?; // byte rate
    f.write_all(&2u16.to_le_bytes())?; // block align
    f.write_all(&16u16.to_le_bytes())?; // bits
    f.write_all(b"data")?;
    f.write_all(&n.to_le_bytes())?;
    for s in samples {
        f.write_all(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes())?;
    }
    Ok(())
}

fn tone(freq: f32, secs: f32, decay: f32) -> Vec<f32> {
    (0..(RATE as f32 * secs) as usize)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            (t * freq * TAU).sin() * (-t * decay).exp() * 0.6
        })
        .collect()
}

/// f0→f1 선형 스윕 (점프의 "휙" 상승감).
fn sweep(f0: f32, f1: f32, secs: f32, decay: f32) -> Vec<f32> {
    let total = (RATE as f32 * secs) as usize;
    let mut phase = 0.0f32;
    (0..total)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = f0 + (f1 - f0) * (t / secs);
            phase += f * TAU / RATE as f32;
            phase.sin() * (-t * decay).exp() * 0.55
        })
        .collect()
}

// ---- PNG 시트 ----

struct Sheet {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Sheet {
    fn new(frames: usize) -> Self {
        let (w, h) = (frames * FRAME, FRAME);
        Self { w, h, px: vec![0; w * h * 4] }
    }

    fn set(&mut self, x: usize, y: usize, c: [u8; 4]) {
        if x < self.w && y < self.h {
            let i = (y * self.w + x) * 4;
            self.px[i..i + 4].copy_from_slice(&c);
        }
    }

    fn rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, c: [u8; 4]) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.set(x, y, c);
            }
        }
    }

    fn save(&self, path: &str) -> std::io::Result<()> {
        let png = voltex::image::encode_rgba8(self.w as u32, self.h as u32, &self.px);
        std::fs::write(path, png)
    }
}

const BODY: [u8; 4] = [90, 160, 240, 255];
const BODY_DARK: [u8; 4] = [50, 110, 190, 255];
const EYE: [u8; 4] = [250, 250, 250, 255];
const PUPIL: [u8; 4] = [20, 30, 50, 255];
const GOLD: [u8; 4] = [250, 210, 60, 255];
const GOLD_DARK: [u8; 4] = [180, 140, 20, 255];

/// 오른쪽을 보는 캐릭터 한 프레임. `legs`: (왼발 dx, 오른발 dx), `bob`: 몸통 y 오프셋.
fn draw_player(s: &mut Sheet, frame: usize, legs: (isize, isize), bob: usize, tucked: bool) {
    let ox = frame * FRAME;
    // 몸통 (아래로 넓은 캡슐 느낌의 2단 사각형)
    s.rect(ox + 10, 6 + bob, ox + 22, 10 + bob, BODY_DARK);
    s.rect(ox + 9, 10 + bob, ox + 23, 24 + bob, BODY);
    // 눈 (오른쪽 응시)
    s.rect(ox + 16, 11 + bob, ox + 21, 16 + bob, EYE);
    s.rect(ox + 19, 12 + bob, ox + 21, 15 + bob, PUPIL);
    // 다리
    if tucked {
        s.rect(ox + 11, 24 + bob, ox + 15, 27 + bob, BODY_DARK);
        s.rect(ox + 17, 24 + bob, ox + 21, 27 + bob, BODY_DARK);
    } else {
        let lx = (ox as isize + 11 + legs.0) as usize;
        let rx = (ox as isize + 17 + legs.1) as usize;
        s.rect(lx, 24 + bob, lx + 4, 30, BODY_DARK);
        s.rect(rx, 24 + bob, rx + 4, 30, BODY_DARK);
    }
}

/// 회전하는 코인 한 프레임 — 가로 반지름 배율로 회전 착시.
fn draw_coin(s: &mut Sheet, frame: usize, squash: f32) {
    let ox = frame as f32 * FRAME as f32;
    let (cx, cy, r) = (16.0f32, 16.0f32, 10.0f32);
    for y in 0..FRAME {
        for x in 0..FRAME {
            let dx = (x as f32 - cx) / squash.max(0.05);
            let dy = y as f32 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            if d < r - 1.5 {
                s.set(frame * FRAME + x, y, GOLD);
            } else if d < r {
                s.set((ox as usize) + x, y, GOLD_DARK);
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
    std::fs::create_dir_all(dir)?;

    // ---- SFX/BGM
    write_wav(&format!("{dir}/sfx_jump.wav"), &sweep(280.0, 700.0, 0.12, 12.0))?;
    let mut coin = tone(1318.5, 0.06, 10.0); // E6 → B6 (마리오풍 띠링)
    coin.extend(tone(1975.5, 0.12, 12.0));
    write_wav(&format!("{dir}/sfx_coin.wav"), &coin)?;
    write_wav(&format!("{dir}/sfx_death.wav"), &sweep(400.0, 90.0, 0.4, 6.0))?;
    let mut clear = tone(523.0, 0.12, 8.0);
    clear.extend(tone(659.0, 0.12, 8.0));
    clear.extend(tone(784.0, 0.12, 8.0));
    clear.extend(tone(1046.5, 0.25, 5.0));
    write_wav(&format!("{dir}/sfx_clear.wav"), &clear)?;
    // BGM: 8초 4코드 아르페지오 루프
    let mut bgm = Vec::new();
    for &f in &[146.8f32, 174.6, 196.0, 174.6] {
        for i in 0..(RATE * 2) as usize {
            let t = i as f32 / RATE as f32;
            let arp = [1.0, 1.5, 2.0, 1.5][(t * 4.0) as usize % 4];
            bgm.push(((t * f * arp * TAU).sin() + 0.25 * (t * f * 2.0 * TAU).sin()) * 0.16);
        }
    }
    write_wav(&format!("{dir}/bgm.wav"), &bgm)?;

    // ---- 스프라이트시트 (플레이어 단일 시트: idle 0-1 / run 2-5 / jump 6)
    let mut player = Sheet::new(8);
    draw_player(&mut player, 0, (0, 0), 0, false); // idle a
    draw_player(&mut player, 1, (0, 0), 1, false); // idle b (숨쉬기 bob)
    draw_player(&mut player, 2, (-2, 2), 0, false); // run 1
    draw_player(&mut player, 3, (0, 0), 1, false); // run 2
    draw_player(&mut player, 4, (2, -2), 0, false); // run 3
    draw_player(&mut player, 5, (0, 0), 1, false); // run 4
    draw_player(&mut player, 6, (0, 0), 0, true); // jump (다리 접음)
    // 프레임 7: 여분 (투명)
    player.save(&format!("{dir}/player.png"))?;

    let mut coin_sheet = Sheet::new(4);
    for (i, sq) in [1.0f32, 0.6, 0.25, 0.6].iter().enumerate() {
        draw_coin(&mut coin_sheet, i, *sq);
    }
    coin_sheet.save(&format!("{dir}/coin.png"))?;

    println!("wrote 5 wav + 2 png to {dir}");
    Ok(())
}
