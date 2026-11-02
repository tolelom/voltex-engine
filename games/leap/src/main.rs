//! Voltex Leap — 2D 사이드뷰 플랫포머. 도그푸딩 게임 2편.
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use voltex::audio::AudioSource;
use voltex::ecs::Tag;
use voltex::prelude::*;
use voltex::renderer_ecs::{spawn_particle, TextureHandle};
use voltex_runtime::{Lifetime, RuntimeApp, RuntimeConfig, RuntimeResult};

use leap::components::*;
use leap::game_state::{self, GamePhase};
use leap::level::*;
use leap::MAP;

/// 초기 창 크기 (논리 픽셀).
const SCREEN: (u32, u32) = (1280, 720);
/// 직교 카메라 세로 뷰 높이 (월드 유닛).
const ORTHO_HEIGHT: f32 = 12.0;

// ClipId = with_audio_files 목록 인덱스.
const CLIP_JUMP: u32 = 0;
const CLIP_COIN: u32 = 1;
const CLIP_DEATH: u32 = 2;
const CLIP_CLEAR: u32 = 3;
const CLIP_BGM: u32 = 4;

/// 에셋 루트: dev 체크아웃에선 crate assets/, 배포에선 exe 옆 assets/ (depths 미러).
fn asset_root() -> PathBuf {
    let dev = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"));
    if dev.is_dir() {
        dev.to_path_buf()
    } else {
        PathBuf::from("assets")
    }
}

/// 플레이어 애니메이션 상태. 단일 8×1 시트에서 `SpriteAnimation::clip`으로
/// 구간 선택 (idle 0-1 / run 2-5 / jump 6 — gen_assets.rs 레이아웃).
#[derive(Clone, Copy, PartialEq)]
enum AnimState {
    Idle,
    Run,
    Jump,
}

impl AnimState {
    /// 이 상태의 시트 클립 (first_frame, frame_count, fps).
    fn clip(self) -> SpriteAnimation {
        match self {
            AnimState::Idle => SpriteAnimation::clip(8, 1, 0, 2, 4.0),
            AnimState::Run => SpriteAnimation::clip(8, 1, 2, 4, 10.0),
            AnimState::Jump => SpriteAnimation::clip(8, 1, 6, 1, 1.0),
        }
    }
}

/// gpu_init에서 로드한 텍스처 핸들 묶음.
struct GameTex {
    player: TextureHandle,
    coin: TextureHandle,
}

fn main() -> RuntimeResult<()> {
    env_logger::init();
    let map = LevelMap::parse(MAP).expect("MAP must parse");
    let phase = Rc::new(RefCell::new(GamePhase::Title));
    let tex: Rc<RefCell<Option<GameTex>>> = Rc::new(RefCell::new(None));

    let audio_files: Vec<PathBuf> = [
        "sfx_jump.wav",
        "sfx_coin.wav",
        "sfx_death.wav",
        "sfx_clear.wav",
        "bgm.wav",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();

    let tex_boot = tex.clone();
    let mut app = RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Leap")
            .with_window_size(SCREEN.0, SCREEN.1)
            .with_orthographic(ORTHO_HEIGHT)
            .with_camera(Vec3::new(10.0, 4.0, 10.0), 0.0, 0.0)
            .with_hud(true)
            .with_asset_root(asset_root())
            .with_audio_files(audio_files)
            .disable_physics()
            .disable_scripts()
            .with_gpu_init(move |app, device, queue| {
                // 상태별 시트 4장 업로드. 실패는 로그만 — 텍스처 없이도
                // 솔리드 스프라이트로 플레이 가능해야 한다 (graceful).
                let root = asset_root();
                let mut load = |name: &str| -> Option<TextureHandle> {
                    let bytes = std::fs::read(root.join(name))
                        .map_err(|e| eprintln!("leap: read {name}: {e}"))
                        .ok()?;
                    app.assets
                        .add_texture_png(device, queue, &bytes)
                        .map_err(|e| eprintln!("leap: decode {name}: {e}"))
                        .ok()
                };
                let loaded = (|| {
                    Some(GameTex {
                        player: load("player.png")?,
                        coin: load("coin.png")?,
                    })
                })();
                if loaded.is_some() {
                    eprintln!("leap: loaded 2 spritesheets");
                }
                *tex_boot.borrow_mut() = loaded;
            }),
    )?;

    app.input_map.bind_axis("move_x", KeyCode::KeyD, KeyCode::KeyA);

    // ---- 카메라: 플레이어 X 추적 (레벨 밖이 안 보이게 clamp)
    {
        let phase = phase.clone();
        let cols = map.cols as f32;
        app.set_camera_fn(move |cam, world, _dt| {
            let aspect = SCREEN.0 as f32 / SCREEN.1 as f32;
            let half_w = ORTHO_HEIGHT * aspect * 0.5;
            let x = match (*phase.borrow(), world.find_by_tag("player")) {
                (GamePhase::Playing | GamePhase::Dead | GamePhase::Clear, Some(p)) => world
                    .get::<Transform>(p)
                    .map(|t| t.position.x)
                    .unwrap_or(cols * 0.5),
                _ => cols * 0.5,
            };
            let x = x.clamp(half_w.min(cols * 0.5), (cols - half_w).max(cols * 0.5));
            cam.position = Vec3::new(x, 4.0, 10.0);
            cam.yaw = 0.0;
            cam.pitch = 0.0;
        });
    }

    // ---- 게임 루프 전체 (입력 + 스텝 + 연출) — 단일 action system.
    // depths는 user/action을 분리했지만 여기선 플레이어 이동이 입력과
    // 강결합이라 한 곳이 단순하다. 애니/uv-flip도 여기서: tick 순서상
    // update_sprite_animations(런타임) → action system → 렌더라 flip이 유지됨.
    {
        let phase = phase.clone();
        let tex = tex.clone();
        let map_run = map.clone();
        let mut anim = AnimState::Idle;
        app.add_action_system(move |world, input, imap, dt| {
            let ph = *phase.borrow();
            match ph {
                GamePhase::Title => {
                    if input.is_key_just_pressed(KeyCode::Space) {
                        game_state::spawn_level(world, &map_run);
                        apply_visuals(world, &tex.borrow());
                        anim = AnimState::Idle;
                        spawn_bgm(world);
                        *phase.borrow_mut() = GamePhase::Playing;
                    }
                }
                GamePhase::Dead | GamePhase::Clear => {
                    if input.is_key_just_pressed(KeyCode::KeyR) {
                        // 즉시 무음 연출 (엔티티 소멸 orphan-voice는 엔진이 처리).
                        if let Some(bgm) = world.find_by_tag("bgm") {
                            if let Some(src) = world.get_mut::<AudioSource>(bgm) {
                                src.stop();
                            }
                        }
                        *phase.borrow_mut() = GamePhase::Title;
                    }
                }
                GamePhase::Playing => {
                    let move_x = imap.axis("move_x", input);
                    let jump = input.is_key_just_pressed(KeyCode::Space);
                    let ev = player_control(&map_run, world, move_x, jump, dt);
                    let feet = player_feet(world);
                    if ev.jumped {
                        play_sfx(world, CLIP_JUMP, feet);
                        dust_burst(world, feet);
                    }
                    if ev.landed {
                        dust_burst(world, feet);
                    }
                    let out = game_state::playing_step(&map_run, world, dt);
                    if out.coins_gained > 0 {
                        play_sfx(world, CLIP_COIN, feet);
                        sparkle_burst(world, feet + Vec3::new(0.0, 0.6, 0.0));
                    }
                    if out.died {
                        play_sfx(world, CLIP_DEATH, feet);
                        stop_bgm(world);
                        *phase.borrow_mut() = GamePhase::Dead;
                    } else if out.cleared {
                        play_sfx(world, CLIP_CLEAR, feet);
                        stop_bgm(world);
                        *phase.borrow_mut() = GamePhase::Clear;
                    }
                    update_player_visual(world, &tex.borrow(), &mut anim);
                }
            }
        });
    }

    // ---- HUD
    {
        let phase = phase.clone();
        app.set_hud_fn(move |ui, world| {
            game_state::draw_hud(ui, world, *phase.borrow());
        });
    }

    app.run()
}

/// 플레이어 발 위치 (파티클/SFX 기준점).
fn player_feet(world: &World) -> Vec3 {
    world
        .find_by_tag("player")
        .and_then(|p| world.get::<Transform>(p).map(|t| t.position))
        .map(|p| p - Vec3::new(0.0, PLAYER_HALF[1], 0.0))
        .unwrap_or(Vec3::ZERO)
}

/// 스폰 직후 텍스처/애니메이션을 입힌다 (텍스처 로드 실패 시 솔리드 유지).
fn apply_visuals(world: &mut World, tex: &Option<GameTex>) {
    let Some(tex) = tex.as_ref() else { return };
    if let Some(p) = world.find_by_tag("player") {
        if let Some(s) = world.get_mut::<Sprite>(p) {
            *s = Sprite::textured(tex.player, [0.9, 0.9]);
        }
        world.add(p, AnimState::Idle.clip());
    }
    let coins: Vec<Entity> = world.query::<Coin>().map(|(e, _)| e).collect();
    for c in coins {
        if let Some(s) = world.get_mut::<Sprite>(c) {
            *s = Sprite::textured(tex.coin, [0.6, 0.6]);
        }
        world.add(c, SpriteAnimation::new(4, 1, 4, 8.0));
    }
}

/// 이동 상태에 맞는 클립으로 전환하고, 바라보는 방향을 `flip_x`에 반영한다.
/// (시트는 한 장 — 텍스처 교체 없음. flip은 표시 속성이라 한 번 세팅으로 유지)
fn update_player_visual(world: &mut World, tex: &Option<GameTex>, anim: &mut AnimState) {
    if tex.is_none() {
        return;
    }
    let Some(p) = world.find_by_tag("player") else { return };
    let Some(player) = world.get::<Player>(p).copied() else { return };

    let desired = if !player.grounded {
        AnimState::Jump
    } else if player.vel.x.abs() > 0.01 {
        AnimState::Run
    } else {
        AnimState::Idle
    };
    if desired != *anim {
        *anim = desired;
        if let Some(a) = world.get_mut::<SpriteAnimation>(p) {
            *a = desired.clip();
        }
    }
    if let Some(s) = world.get_mut::<Sprite>(p) {
        s.flip_x = player.facing < 0.0;
    }
}

fn spawn_bgm(world: &mut World) {
    let mut src = AudioSource::new(CLIP_BGM);
    src.looping = true;
    src.volume = 0.3;
    src.playing = true;
    world.spawn_bundle((Transform::new(), src, Tag("bgm".into())));
}

fn stop_bgm(world: &mut World) {
    if let Some(bgm) = world.find_by_tag("bgm") {
        if let Some(src) = world.get_mut::<AudioSource>(bgm) {
            src.stop();
        }
    }
}

/// 1.5초 뒤 자동 소멸하는 원샷 SFX 엔티티 (depths 미러).
fn play_sfx(world: &mut World, clip: u32, pos: Vec3) {
    let mut src = AudioSource::new(clip);
    src.playing = true;
    src.volume = 0.8;
    world.spawn_bundle((Transform::from_position(pos), src, Lifetime::new(1.5)));
}

/// 점프/착지 먼지: 발밑에서 옆으로 퍼지는 회백색 입자.
fn dust_burst(world: &mut World, feet: Vec3) {
    for i in 0..6 {
        let vx = (i as f32 - 2.5) * 0.7;
        spawn_particle(
            world,
            feet,
            [0.82, 0.8, 0.75, 0.9],
            Particle {
                velocity: Vec3::new(vx, 1.2, 0.0),
                acceleration: Vec3::new(0.0, -4.0, 0.0),
                lifetime: 0.35,
                start_size: 0.16,
                end_size: 0.05,
                ..Default::default()
            },
        );
    }
}

/// 코인 획득 스파클: 위로 튀는 금색 입자.
fn sparkle_burst(world: &mut World, pos: Vec3) {
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        spawn_particle(
            world,
            pos,
            [1.0, 0.85, 0.3, 1.0],
            Particle {
                velocity: Vec3::new(a.cos() * 1.8, a.sin() * 1.8 + 1.0, 0.0),
                acceleration: Vec3::new(0.0, -3.0, 0.0),
                lifetime: 0.45,
                start_size: 0.12,
                end_size: 0.03,
                ..Default::default()
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_map_parses_with_expected_structure() {
        let m = LevelMap::parse(MAP).expect("MAP must parse (모든 행 같은 길이)");
        assert!(m.coins.len() >= 3, "코인 3개 이상: {}", m.coins.len());
        assert_eq!(m.walkers.len(), 2, "워커 2마리");
        assert!(!m.spikes.is_empty(), "가시 구간 존재");
        assert_eq!(m.rows, 8);
        assert_eq!(m.cols, 48);
    }
}
