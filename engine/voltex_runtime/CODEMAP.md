# voltex_runtime

## 책임 (한 줄)
표준 부트스트랩 + 메인 루프 + 기본 시스템 등록 + winit 통합. 외부 사용자가 다섯 줄 이내의 main 으로 게임을 띄우게 한다.

## 공개 API 진입점
- `RuntimeApp` — 모든 subsystem 보관 + winit ApplicationHandler 통합 (src/app.rs)
- `RuntimeConfig` — 빌더 패턴 설정 (window, scene, callbacks) (src/config.rs)
- `RuntimeError`, `RuntimeResult` — thiserror 기반 에러 (src/error.rs)

## 모듈 구조
- `app` — RuntimeApp (모든 subsystem 보관 + `tick_systems_only` + `set_hud_fn` + Lua `script_input` 발행) (src/app.rs)
- `config` — RuntimeConfig + builder 메서드 + 4개 callback 타입 (RegistrySetupFn, SceneBuilderFn, SystemSetupFn, GpuInitFn). `with_asset_root(dir)` — 상대 에셋 경로(TexturePath/MeshPath/AudioPath, audio_files)의 루트; 상대 루트는 boot 시 exe 디렉토리 기준 해석 (`resolved_asset_root`/`resolve_asset_root_with`) — cwd 무관 배포 exe (friction #6) (src/config.rs)
- `error` — RuntimeError enum + Result alias (src/error.rs)
- `asset_paths` — `AssetPathCache`(경로별 핸들 공유 + 실패 None 캐시) + `resolve_asset_paths_with`(주입 로더 코어, GPU 무관 테스트 가능) + `resolve_asset_paths`(**Vfs 우선 + fs fallback** read → 업로드 실 래퍼; 키 = forward-slash 경로 = 에디터 영속 형식 = `.vpkg` 엔트리). 씬 영속 TexturePath/MeshPath → 핸들. handler 가 materialize 직후 매 프레임 호출(멱등), load_scene 이 clear_failures (v1.0.98~100). resolve 성공 시 1회 `resolved N texture path(s)` log_info — dist exe E2E 의 양성 증거 (v1.0.102). read_asset 순서: Vfs → asset_root(상대 경로만) → cwd fs (src/asset_paths.rs)
- `audio_sync` — `audio_sync_system(world, audio, listener, &mut AudioVoiceTracker)` (AudioSource→AudioSystem 브릿지) + 순수 `decide_action` 엣지 로직 + `AudioVoiceTracker`(entity→VoiceId; play 엣지 등록, stop 엣지는 해당 voice 만 정지, entity 소멸(despawn/clear/컴포넌트 제거) 시 orphan voice 자동 stop — friction #9 해소, 수동 `src.stop()` 후 despawn 불필요) + `poll_audio_hot_reload(watcher, watched, audio)` (변경 파일 재디코드 → replace_clip; v1.0.90) (src/audio_sync/mod.rs)
- `audio_paths` — `AudioPath(String)` 컴포넌트(NAME `audio_path`, `path_component!` impl, 앱 registry 자동 등록) + `AudioClipCache`(경로별 ClipId 공유 + 실패 None 캐시) + `resolve_audio_paths_with`(주입 로더 코어) + `resolve_audio_paths`(Vfs 우선 read → `decode_clip_bytes` → `add_clip`). AudioSource 없으면 stopped 로 부착, 있으면 stale clip_id 재바인딩 — **AudioPath 보유 엔티티의 clip_id 는 resolver 소유** (수동 제어는 AudioPath 제거 후). tick audio 단계 첫머리 호출(멱등), load_scene 이 clear_failures. 성공 시 1회 `resolved N audio path(s)` log_info (v1.0.103). `AudioClipCache::seed(path, id)`/`get` — RuntimeApp 부트가 `with_audio_files` 매핑(설정된 경로 형태 그대로)을 시드해 같은 파일의 AudioPath 가 기존 클립 재사용 (이중 디코드 제거, 핫리로드 공유 — friction #5) (src/audio_paths.rs)
- `RuntimeApp::watch_audio_clip(path, clip_index)` — 오디오 파일 감시 등록 (전용 `audio_watcher`, script watcher 와 분리 — 그쪽은 pool 이 변경을 전부 소비). tick 의 audio 단계에서 자동 폴링 + 로그 (src/app.rs)
- `handler` (crate-private) — winit ApplicationHandler 구현. 창 생성 직후(`resumed`) + `Resized` 이벤트에서 `app.input.set_window_size(물리 픽셀)` 주입 — 게임 시스템이 `input.window_size()`로 DPI/리사이즈 무관 screen→NDC 변환 가능 (0x0 최소화 이벤트는 skip, 마지막 유효 크기 유지) (src/handler.rs)

## 의존성
- voltex crate: voltex_math, voltex_platform, voltex_ecs, voltex_asset, voltex_renderer, voltex_renderer_ecs, voltex_physics, voltex_script, voltex_editor (HUD UiContext/UiRenderer), voltex_audio (AudioSystem)
- 외부 crate: winit, wgpu, bytemuck, thiserror

## RuntimeConfig 빌더 메서드 (13개)
| 메서드 | 시점 | 인자 |
|---|---|---|
| `with_window_title(s)` | new 직전 | window.title |
| `with_window_size(w, h)` | new 직전 | window dims |
| `with_initial_scene(path)` | new (실패 시 log_error + 빈 월드 degrade — silent 아님, v1.0.102) | binary 우선 → JSON fallback |
| `with_registry_setup(F)` | new 첫 단계 | `&mut ComponentRegistry` (register_defaults 이후) |
| `with_scene_builder(F)` | new 마지막 단계 | `&mut World, &mut ComponentRegistry` — GPU 없이 가능한 entity seed |
| `with_system_setup(F)` | new 마지막 단계 | `&mut RuntimeApp` — `add_system` 호출 등 |
| `with_gpu_init(F)` | 첫 `resumed` 시 (GPU 준비 후) | `&mut RuntimeApp, &Device, &Queue` — 메시 업로드, MeshHandle 부착 |
| `with_fixed_dt(f32)` | new 직후 | delta 시간 고정화 |
| `disable_physics()` | new 직후 | physics_step skip |
| `disable_scripts()` | new 직후 | script 시스템 skip |
| `with_vfs(Vfs)` | new 직후 | 가상 파일시스템 (`.vpkg` 아카이브 지원) |
| `with_crash_log_dir(path)` | new 직후 | crash log 저장 경로 |
| `disable_crash_log()` | new 직후 | panic hook 비활성화 (headless/테스트 용) |
| `voltex_runtime::set_crash_{gpu_adapter,scene,fps}` | 어디서나 | 크래시 리포트 헤더 컨텍스트 (RuntimeApp이 어댑터·씬·FPS를 자동으로 채움, v1.0.127) |
| `with_hud(bool)` | new 직후 | 인-윈도우 HUD 오버레이 활성화 (`set_hud_fn` 과 함께) |
| `with_camera(pos, yaw, pitch)` / `with_camera_fov(f)` / `with_orthographic(h)` | new 직후 | 초기 카메라 프레이밍 (`set_camera_fn` 로 매 프레임 추적). `with_orthographic`은 2D/탑다운용 직교 투영(view height h) |
| `with_pbr(bool)` | new 직후 | 메시 패스를 colored Blinn-Phong → 풀 PBR로 전환 |
| `with_light(dir, color, ambient)` | new 직후 | directional light (`set_light_fn` 로 매 프레임 day/night) |
| `with_audio(clips)` | new 직후 | AudioSystem 생성 + audio_sync_system 활성화 |
| `with_audio_files(paths)` | new 직후 | 파일 디코드(wav/ogg) + ClipId 자동 부여(audio_clips 뒤 이어붙음) + 핫리로드 자동 감시. 디코드 실패는 무음 placeholder 로 인덱스 안정 유지 (v1.0.94) |

## RuntimeApp::tick_systems_only(dt) 실행 순서
0. `time.advance(dt)` — 프레임 클록 갱신(`elapsed_secs`/`delta_secs`/`frame_count`). `add_timed_system` + HUD 가 일관된 시간 관측
1. `propagate_transforms(world)` (항상) + `update_skinned_poses` + `update_particles` + `update_sprite_animations` (no-op if 해당 엔티티 없음)
2. `physics_step(world, &config)` (enable_physics 시)
3. `script_start_all(world, pool)` + `script_update_all(world, pool, dt)` + `pool.poll_reload(watcher)` (enable_scripts 시)
4. 사용자 등록 시스템 (등록 순서, `add_system` 으로 추가)
5. 입력 시스템 (`add_input_system`) → 6. 액션 시스템 (`add_action_system`, `&InputMap` 전달) → 7. 타임드 시스템 (`add_timed_system`, `&Time` 전달)
8. `audio_sync_system` (enable_audio 시) — 같은 프레임의 playback 토글/listener 이동 반영
9. `update_lifetimes` — 마지막. `Lifetime` 엔티티를 카운트다운 후 만료 시 `despawn_recursive` (사용자 시스템이 마지막 프레임을 관측한 뒤 제거). no-op if 없음

`Time` 외 `Timer`/`TimerMode`(src/time.rs): `Timer::once`/`repeating(dur)` + `tick(dt)` → `just_finished`/`finished`/`fraction`/`reset`. 쿨다운·스폰 간격·능력 재충전용 순수 카운트다운. 0/음수 duration 즉시 발사, 반복은 dt>period 시 wrap. `Tween`(src/time.rs): Timer+easing+lerp 묶음 — `new(from,to,dur,easing)`/`linear` + `tick(dt)->f32`/`value`/`value_normalized`(Vec lerp용)/`finished`/`reset`.

매 프레임 `run()` 의 winit handler 가 `tick_systems_only` 호출 → `materialize_renderables`(이번 프레임에 스폰된 `PrimitiveMesh`/`MaterialDesc` 엔티티를 GPU 메시/머티리얼로, device 필요해 handler 에서 실행) → `BasicRenderer::render` (+ enable_hud 시 UI 오버레이) 로 화면 출력. materialize 는 idempotent + pending 없으면 저렴.

## 변경 시 주의점
- **Drop 순서**: `RuntimeApp` 의 필드 선언 순서가 곧 drop 순서. `runtime_state` (GPU) → `user_systems` → `file_watcher`/`timer`/`input`/`physics_config` → `script_pool` (LuaState 의 `__voltex_world` light userdata 가 World 보다 먼저 drop) → `assets`/`registry`/`world` → `config`. 이 순서는 LuaState use-after-free 회피용. 필드 추가/순서 변경 시 LuaState 가 World 보다 먼저 drop 됨을 유지.
- **GPU init lazy**: `RuntimeApp::new` 는 window/GPU 를 만들지 않는다. `with_gpu_init` 콜백이 첫 `resumed` 에서 한 번만 실행. 헤드리스 단위 테스트에서는 `tick_systems_only` 만 호출 가능.
- **BasicRenderer 시그너처**: handler.rs 의 `redraw` 가 직접 호출. `voltex_renderer_ecs::BasicRenderer::render` 인자가 바뀌면 함께 갱신.
- **카메라**: 초기 위치/방향은 `RuntimeConfig::with_camera(pos, yaw, pitch)` / `with_camera_fov` (기본 `(0,10,20)`, yaw 0, pitch -0.4, 45°). 매 프레임 추적(follow/orbit)은 `RuntimeApp::set_camera_fn(\|cam, world, dt\| ...)` — tick 직후 실행되며 기본 RMB fly-camera를 override. 2D/탑다운은 `RuntimeConfig::with_orthographic(height)` → `Camera::ortho_height=Some` (parallel 투영, near/far -100..100, 깊이 무관 footprint). 캡처 회귀 `orthographic_footprint_is_depth_independent`. **주의**: 기본 카메라가 멀어서(z=20) 1-unit 오브젝트는 화면에서 ~15px 점으로 보임 — 게임은 거의 항상 with_camera/set_camera_fn로 프레이밍해야 함 (voltex_capture `camera_framing_controls_visible_object_size` 회귀가 가드).
- **씬 저장/로드**: `RuntimeApp::save_scene_json(path)` (registry 등록 컴포넌트만 직렬화) / `load_scene(path)` (World clear 후 binary→JSON, Vfs 우선). `with_initial_scene` 는 부팅 시 로드.
- **충돌/트리거 이벤트**: `RuntimeApp::set_collision_fn(\|contacts, &mut world, dt\| ...)` — 매 프레임 `detect_collisions`(Collider+Transform 엔티티) 결과를 game에 전달. `enable_physics` 무관하게 실행 → 강체 없이 트리거 존으로도 사용. physics_step 직후 실행.
- **조명**: `RuntimeConfig::with_light(dir, color, ambient)` (기본 `(0,-1,-1)`/흰색/0.1) + `RuntimeApp::set_light_fn(\|light, world, dt\| ...)` (day/night·flicker). camera_fn 직후 실행. 단일 directional light.
- **점광원/스폿라이트**: `RuntimeApp::set_point_lights(Vec<PointLight>)` — 매 프레임 BasicRenderer에 업로드, mesh_colored가 additive (MAX_POINT_LIGHTS=8). `PointLight::new(pos, color, range, intensity)` = 무지향 점광원, `PointLight::spot(pos, dir, color, range, intensity, cone_degrees)` = 원뿔 스폿(부드러운 가장자리, `spot_cos_outer < -1.5` 가 omni 센티넬). colored 파이프라인 라이팅 정상 검증됨(흰 큐브: 광원 향한 면 밝음, 구: 앞 반구 밝음). 점광원 수학 캡처 회귀(주변 6광원→밝아짐), 스폿 원뿔 캡처 회귀(`spotlight_lights_cone_footprint_not_corners` + docs/visual/spot_cone.png).
- **PBR 경로**: `RuntimeConfig::with_pbr(true)` → `BasicRenderer::set_pbr_enabled`. colored Blinn-Phong 대신 Cook-Torrance PBR(멀티라이트 `LightsUniform` = sun + point/spot 접합, per-entity 머티리얼 파라미터, 4-맵 텍스처 via `PbrTextures` 컴포넌트, IBL BRDF LUT). 기본 off(colored). `with_shadows`/`with_light`/`set_point_lights`와 조합. 캡처 회귀 `basic_renderer_pbr_path_shades_differently_than_colored` + `docs/visual/pbr_runtime.png`. 스킨드 메시도 PBR 적용(`pbr_skinned.wgsl`, palette를 group0에 접합, per-entity 머티리얼/텍스처 반영). env는 procedural sky(SH 0).
- **그림자**: `RuntimeConfig::with_shadows(true)` → BasicRenderer가 directional shadow depth pre-pass(group 3, ortho EXTENT=30) + mesh_colored 3x3 PCF 샘플. 기본 off(uniform size 0 → 셰이더 풀라이트, 동작 불변). 캡처 회귀 `shadows_darken_ground_under_occluder` + 시각 확인(docs/visual/shadow_on.png). 한계: 스킨드 메시는 caster 아님, ortho 고정(카메라 프러스텀 fit 미적용), 바닥에 붙은 오브젝트는 자기 그림자 가림.
- **디버그 드로우**: `RuntimeApp::set_debug_draw_fn(\|d: &mut DebugLines, world, dt\| ...)` — 매 프레임 light_fn 직후 실행. 공유 `DebugLines` 버퍼를 clear 후 콜백이 gizmo 세그먼트 큐잉 → `BasicRenderer::set_debug_lines` 업로드 → depth-tested `LineList` 패스로 3D 위에 렌더. `DebugLines` 헬퍼: `line(a,b,color)` / `ray(origin,dir,len,color)` / `aabb(min,max,color)`(12 edges) / `cross(center,half,color)`. 캡처 회귀 `debug_lines_draw_colored_segments` + `docs/visual/debug_lines.png`. 한계: 라인 두께 1px 고정, MAX 8192 verts.
- **사용자 시스템 시그너처**: `FnMut(&mut World, f32) + 'static`. dt 는 호출 프레임의 dt (fixed_dt 가 Some 면 그 값).
- **HUD 오버레이**: `with_hud(true)` + `set_hud_fn(|ui, world| ...)`. UiContext/UiRenderer 는 `RuntimeState`(GPU) 에 보관, `resumed` 에서 생성. `redraw` 가 BasicRenderer 다음 같은 encoder 로 `LoadOp::Load` UI 패스. 콜백은 `take()` 로 꺼내 `app.world` 와 disjoint-field 借用 충돌 회피.
- **Lua input 발행**: `enable_scripts` 시 매 프레임 `fill_script_input` 으로 `script_input` 갱신 → `voltex_script::set_current_input(ptr)` (실행 후 null). 키 이름↔winit keycode 매핑 `SCRIPT_KEY_NAMES` 가 여기 소유.
- **audio**: `with_audio(clips)` 시 `RuntimeApp::new` 에서 `AudioSystem::new` (스레드 spawn, GPU 무관, 실패 시 silent None). `set_audio_listener(entity)` 로 리스너 지정. `audio_sync_system` 이 `AudioSource::playing` 의 엣지(`active` 플래그)에서만 play/stop 발행 — held playing 이 매 프레임 voice 를 새로 만들지 않음. 한계: SpatialParams min/max 고정(1.0/50.0), AudioSource 씬 직렬화 미등록 — v1.1.
- **single-window only**: winit `run_app` 자체가 single window 지원. multi-window 는 v1.1+.

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_runtime --lib`
- 현재 테스트 개수: 60개 (scene save/load roundtrip ×1 추가)
- GPU/winit 의존 테스트는 직접 검증 불가 → examples/runtime_demo 수동 실행 검증

## 예제
- `examples/runtime_demo` — 솔라 시스템 (sun + 3 planet + 1 moon) 자전 데모. `main` 본문 4줄.
