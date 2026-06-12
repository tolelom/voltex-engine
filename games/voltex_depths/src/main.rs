//! Voltex Depths — 탑다운 트윈스틱 던전. 도그푸딩 게임 1편.
//! 설계: docs/superpowers/specs/2026-06-11-voltex-depths-game-design.md
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use voltex::audio::AudioSource;
use voltex::ecs::Tag;
use voltex::prelude::*;
use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult, Timer};

use voltex_depths::{combat, enemy, game_state};
use voltex_depths::combat::*;
use voltex_depths::components::*;
use voltex_depths::level::*;

const MAP: &[&str] = &[
    "#############################################################",
    "#.....#..........#..........#..........#..........#.........#",
    "#.....#...c..#...#..c..c....#..s...#...#..c..s....#.........#",
    "#..P..D......#...D..........D......#...D..........D....B....#",
    "#.....#..........#...##.....#..........#..........#.........#",
    "#.....#..........#..........#..........#..........#.........#",
    "#############################################################",
];

const PLAYER_SPEED: f32 = 6.0;
const FIRE_COOLDOWN: f32 = 0.18;
/// 초기 창 크기 (논리 픽셀). 조준 NDC 변환은 이 상수가 아니라
/// `input.window_size()`(물리 픽셀, 리사이즈/DPI 반영)를 쓴다 — friction #4 해소.
const SCREEN: (u32, u32) = (1280, 720);
/// 에셋은 cwd 무관하게 crate 기준 절대 경로 (배포 exe에선 깨짐 — known limitation).
const ASSET_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

// ClipId = with_audio_files 목록 인덱스.
const CLIP_SHOOT: u32 = 0;
const CLIP_HIT: u32 = 1;
const CLIP_CLEAR: u32 = 2;
const CLIP_BGM: u32 = 3;

fn main() -> RuntimeResult<()> {
    env_logger::init();
    let source = LevelMap::parse(MAP).expect("MAP must parse");
    let nav = build_navmesh(&source);
    let map = Rc::new(RefCell::new(source.clone()));
    let phase = Rc::new(RefCell::new(GamePhase::Title));
    // 카메라 VP 공유 — 조준 unprojection용. camera_fn이 시스템 tick 뒤에 돌아
    // action system은 직전 프레임 VP를 본다 (1프레임 지연 허용).
    let cam_vp = Rc::new(RefCell::new(Mat4::IDENTITY));
    // 이번 프레임 조준점 (Playing 중 마우스 unproject 결과). action system이
    // 쓰고 debug_draw_fn이 읽는다 — debug_fn 시그니처(|d, world, dt|)에
    // input이 없어 Rc 공유로 전달.
    let aim_point: Rc<RefCell<Option<Vec3>>> = Rc::new(RefCell::new(None));

    // 오디오 파일은 T14에서 생성됨. with_audio_files는 부재/디코드 실패 시
    // silent placeholder로 부트를 계속하고, 파일이 생기면 핫리로드로 살아난다.
    let audio_files: Vec<PathBuf> = ["sfx_shoot.wav", "sfx_hit.wav", "sfx_clear.wav", "bgm.wav"]
        .iter()
        .map(|f| PathBuf::from(format!("{ASSET_DIR}/{f}")))
        .collect();

    let mut app = RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Depths")
            .with_window_size(SCREEN.0, SCREEN.1)
            .with_shadows(true)
            .with_hud(true)
            .with_audio_files(audio_files)
            .disable_physics()
            .disable_scripts(),
    )?;

    // ---- 입력 바인딩
    app.input_map
        .bind_axis("move_x", KeyCode::KeyD, KeyCode::KeyA)
        .bind_axis("move_y", KeyCode::KeyW, KeyCode::KeyS);

    // ---- 카메라: 플레이어 추적 탑다운 + VP 공유
    {
        let phase = phase.clone();
        let cam_vp = cam_vp.clone();
        app.set_camera_fn(move |cam, world, _dt| {
            let target = match (*phase.borrow(), world.find_by_tag("player")) {
                (GamePhase::Playing, Some(p)) => world
                    .get::<Transform>(p)
                    .map(|t| t.position)
                    .unwrap_or(Vec3::ZERO),
                _ => Vec3::ZERO,
            };
            cam.position = target + Vec3::new(0.0, 16.0, 9.0);
            cam.yaw = 0.0;
            cam.pitch = -1.05;
            *cam_vp.borrow_mut() = cam.view_projection();
        });
    }

    // ---- 액션: 이동 + 발사 + phase 키
    // tick 순서는 user systems(아래 게임플레이) → action systems(여기) —
    // 여기서 만든 phase 전환/투사체는 다음 프레임부터 게임플레이 시스템이
    // 처리한다. 1프레임 지연 수준이라 문제 없음.
    {
        let phase = phase.clone();
        let map = map.clone();
        let cam_vp = cam_vp.clone();
        let aim_point = aim_point.clone();
        let source = source.clone();
        // Once + 만료 시작: 첫 클릭 즉시 발사, 이후 finished/reset 쿨다운
        // (components.rs touch 타이머와 같은 sticky 패턴 — Repeating
        //  just_finished는 클릭과 wrap 프레임이 겹쳐야만 발사돼 fragile).
        let mut fire_cd = Timer::once(FIRE_COOLDOWN);
        fire_cd.tick(FIRE_COOLDOWN);
        app.add_action_system(move |world, input, imap, dt| {
            let ph = *phase.borrow();
            *aim_point.borrow_mut() = None; // Playing 분기에서만 채움
            match ph {
                GamePhase::Title => {
                    if input.is_key_just_pressed(KeyCode::Space) {
                        game_state::start_game(world, &mut map.borrow_mut(), &source);
                        spawn_bgm(world);
                        *phase.borrow_mut() = GamePhase::Playing;
                    }
                }
                GamePhase::GameOver | GamePhase::Victory => {
                    if input.is_key_just_pressed(KeyCode::KeyR) {
                        // world.clear()는 여기서 하지 않는다 — BGM AudioSource를
                        // despawn하면 playing=false 엣지가 영원히 오지 않아 루핑
                        // voice가 mixer에 잔류하고, 재시작마다 BGM이 중첩된다
                        // (엔진 갭; dogfood-friction #9).
                        //
                        // 대신 BGM 소스의 playing을 false로 내린다. audio_sync는
                        // tick_systems_only의 맨 마지막(action_systems 이후)에 돌아
                        // 같은 프레임 내에 (playing=false, active=true) 엣지를 보고
                        // mixer에 Stop을 즉시 발행한다.
                        //
                        // 잔존 씬 엔티티는 Title 오버레이(alpha 230)가 가리며,
                        // Space의 start_game이 world.clear()로 정리한다.
                        if let Some(bgm) = world.find_by_tag("bgm") {
                            if let Some(src) = world.get_mut::<AudioSource>(bgm) {
                                src.stop();
                            }
                        }
                        *phase.borrow_mut() = GamePhase::Title;
                    }
                }
                GamePhase::Playing => {
                    let m = map.borrow();
                    let mv = imap.vector("move_x", "move_y", input);
                    let dir = Vec3::new(mv.x, 0.0, -mv.y).normalize_or_zero();
                    if let Some(p) = world.find_by_tag("player") {
                        if let Some(t) = world.get_mut::<Transform>(p) {
                            t.position = try_move(&m, t.position, dir * PLAYER_SPEED * dt, 0.45);
                        }
                    }
                    fire_cd.tick(dt);
                    // 조준점: 마우스(물리 픽셀) → NDC → 플레이어 평면 unproject.
                    // 창 크기는 input.window_size()(물리 픽셀) — mouse_position과
                    // 단위가 같아 리사이즈/DPI 배율 무관 (friction #4 해소).
                    // 매 프레임 계산해 발사와 디버그 마커가 공유.
                    let (ww, wh) = input.window_size();
                    let aim = if ww > 0 && wh > 0 {
                        let (mx, my) = input.mouse_position();
                        let ndc = ndc_from_mouse(mx, my, ww as f32, wh as f32);
                        cam_vp
                            .borrow()
                            .inverse()
                            .and_then(|inv| unproject_to_plane(&inv, ndc, PLAYER_Y))
                    } else {
                        None
                    };
                    *aim_point.borrow_mut() = aim;
                    if input.is_mouse_button_pressed(MouseButton::Left) && fire_cd.finished() {
                        if let (Some(p), Some(aim)) = (world.find_by_tag("player"), aim) {
                            if let Some(from) = world.get::<Transform>(p).map(|t| t.position) {
                                fire_cd.reset();
                                let dir = Vec3::new(aim.x - from.x, 0.0, aim.z - from.z);
                                spawn_projectile(
                                    world,
                                    Vec3::new(from.x, 0.6, from.z),
                                    dir,
                                    Team::Player,
                                    1,
                                    14.0,
                                );
                                play_sfx(world, CLIP_SHOOT, from);
                            }
                        }
                    }
                }
            }
        });
    }

    // ---- 게임플레이 시스템 (Playing 동안만)
    {
        let phase = phase.clone();
        let map = map.clone();
        let mut repath = Timer::repeating(0.5);
        app.add_system(move |world, dt| {
            if *phase.borrow() != GamePhase::Playing {
                return;
            }
            let due = repath.tick(dt).just_finished();
            let m = map.borrow().clone(); // 문 개방 반영 스냅샷 (맵 작아 저렴)
            enemy::enemy_ai_system(world, &m, dt, due);
            voltex::ai::nav_agent_system(world, &nav, dt);
            enemy::velocity_system(world, &m, dt);
            combat::projectile_system(world, &m, dt);
            let out = combat::death_system(world);
            // 방 클리어음: door_system 전후 Door 수 비교
            let doors_before = world.query::<Door>().count();
            game_state::door_system(world, &mut map.borrow_mut());
            let doors_after = world.query::<Door>().count();
            if doors_after < doors_before {
                let player_pos = world
                    .find_by_tag("player")
                    .and_then(|p| world.get::<Transform>(p).map(|t| t.position))
                    .unwrap_or(Vec3::ZERO);
                play_sfx(world, CLIP_CLEAR, player_pos);
            }
            if out.boss_died {
                *phase.borrow_mut() = GamePhase::Victory;
            } else if out.player_died {
                *phase.borrow_mut() = GamePhase::GameOver;
            }
        });
    }

    // ---- 충돌 → 데미지 (Collider+Transform 엔티티의 detect_collisions 결과)
    {
        let phase = phase.clone();
        app.set_collision_fn(move |contacts, world, _dt| {
            if *phase.borrow() != GamePhase::Playing {
                return;
            }
            // 플레이어 피격음: HP 스냅샷 비교
            let hp_before = world
                .find_by_tag("player")
                .and_then(|p| world.get::<Health>(p).map(|h| h.hp));
            combat::handle_contacts(contacts, world);
            let hp_after = world
                .find_by_tag("player")
                .and_then(|p| world.get::<Health>(p).map(|h| h.hp));
            if let (Some(b), Some(a)) = (hp_before, hp_after) {
                // 치사 타격 프레임에도 피격음 재생 — 의도 (죽는 순간 무음 방지).
                if a < b {
                    let player_pos = world
                        .find_by_tag("player")
                        .and_then(|p| world.get::<Transform>(p).map(|t| t.position))
                        .unwrap_or(Vec3::ZERO);
                    play_sfx(world, CLIP_HIT, player_pos);
                }
            }
        });
    }

    // ---- 조준점 디버그 마커: 게임이 인식한 마우스 unproject 지점에 십자 표시.
    // 도그푸딩 게임이라 항상 켜둠 — 조준이 어긋나면 "마커 위치 = 게임이 본
    // 클릭 위치"라 화면만 보고 입력 좌표계 문제를 진단할 수 있다 (friction #4).
    {
        let aim_point = aim_point.clone();
        app.set_debug_draw_fn(move |d, _world, _dt| {
            if let Some(aim) = *aim_point.borrow() {
                d.cross(aim, 0.4, [1.0, 0.9, 0.2]);
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

fn spawn_bgm(world: &mut World) {
    let mut src = AudioSource::new(CLIP_BGM);
    src.looping = true;
    src.volume = 0.35;
    src.playing = true;
    world.spawn_bundle((Transform::new(), src, Tag("bgm".into())));
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_depths::components::EnemyKind;

    #[test]
    fn main_map_parses_with_expected_structure() {
        let m = LevelMap::parse(MAP).expect("MAP must parse (모든 행 같은 길이)");
        assert_eq!(m.doors.len(), 5, "방 6개를 잇는 문 5개");
        assert_eq!(m.region_count, 6, "세로 벽 5개로 나뉜 방 6개");
        let bosses = m
            .enemies
            .iter()
            .filter(|&&(_, _, k)| k == EnemyKind::Boss)
            .count();
        assert_eq!(bosses, 1, "보스는 마지막 방에 1마리");
        assert!(m.enemies.len() >= 6, "잡몹 + 보스");
    }
}
