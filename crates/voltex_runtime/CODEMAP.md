# voltex_runtime

## 책임 (한 줄)
표준 부트스트랩 + 메인 루프 + 기본 시스템 등록 + winit 통합. 외부 사용자가 다섯 줄 이내의 main 으로 게임을 띄우게 한다.

## 공개 API 진입점
- `RuntimeApp` — 모든 subsystem 보관 + winit ApplicationHandler 통합 (src/app.rs)
- `RuntimeConfig` — 빌더 패턴 설정 (window, scene, callbacks) (src/config.rs)
- `RuntimeError`, `RuntimeResult` — thiserror 기반 에러 (src/error.rs)

## 모듈 구조
- `app` — RuntimeApp (모든 subsystem 보관 + `tick_systems_only` + `set_hud_fn` + Lua `script_input` 발행) (src/app.rs)
- `config` — RuntimeConfig + 14개 builder 메서드 + 4개 callback 타입 (RegistrySetupFn, SceneBuilderFn, SystemSetupFn, GpuInitFn) (src/config.rs)
- `error` — RuntimeError enum + Result alias (src/error.rs)
- `audio_sync` — `audio_sync_system` (AudioSource→AudioSystem 브릿지) + 순수 `decide_action` 엣지 로직 (src/audio_sync.rs)
- `handler` (crate-private) — winit ApplicationHandler 구현 (src/handler.rs)

## 의존성
- voltex crate: voltex_math, voltex_platform, voltex_ecs, voltex_asset, voltex_renderer, voltex_renderer_ecs, voltex_physics, voltex_script, voltex_editor (HUD UiContext/UiRenderer), voltex_audio (AudioSystem)
- 외부 crate: winit, wgpu, bytemuck, thiserror

## RuntimeConfig 빌더 메서드 (13개)
| 메서드 | 시점 | 인자 |
|---|---|---|
| `with_window_title(s)` | new 직전 | window.title |
| `with_window_size(w, h)` | new 직전 | window dims |
| `with_initial_scene(path)` | new (silent 실패) | binary 우선 → JSON fallback |
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
| `with_hud(bool)` | new 직후 | 인-윈도우 HUD 오버레이 활성화 (`set_hud_fn` 과 함께) |
| `with_camera(pos, yaw, pitch)` / `with_camera_fov(f)` | new 직후 | 초기 카메라 프레이밍 (`set_camera_fn` 로 매 프레임 추적) |
| `with_light(dir, color, ambient)` | new 직후 | directional light (`set_light_fn` 로 매 프레임 day/night) |
| `with_audio(clips)` | new 직후 | AudioSystem 생성 + audio_sync_system 활성화 |

## RuntimeApp::tick_systems_only(dt) 실행 순서
1. `propagate_transforms(world)` (항상)
2. `physics_step(world, &config)` (enable_physics 시)
3. `script_start_all(world, pool)` + `script_update_all(world, pool, dt)` + `pool.poll_reload(watcher)` (enable_scripts 시)
4. 사용자 등록 시스템 (등록 순서, `add_system` 으로 추가)
5. 입력 시스템 (`add_input_system`)
6. `audio_sync_system` (enable_audio 시) — 마지막. 같은 프레임의 playback 토글/listener 이동 반영

매 프레임 `run()` 의 winit handler 가 `tick_systems_only` 호출 후 `BasicRenderer::render` (+ enable_hud 시 UI 오버레이) 로 화면 출력.

## 변경 시 주의점
- **Drop 순서**: `RuntimeApp` 의 필드 선언 순서가 곧 drop 순서. `runtime_state` (GPU) → `user_systems` → `file_watcher`/`timer`/`input`/`physics_config` → `script_pool` (LuaState 의 `__voltex_world` light userdata 가 World 보다 먼저 drop) → `assets`/`registry`/`world` → `config`. 이 순서는 LuaState use-after-free 회피용. 필드 추가/순서 변경 시 LuaState 가 World 보다 먼저 drop 됨을 유지.
- **GPU init lazy**: `RuntimeApp::new` 는 window/GPU 를 만들지 않는다. `with_gpu_init` 콜백이 첫 `resumed` 에서 한 번만 실행. 헤드리스 단위 테스트에서는 `tick_systems_only` 만 호출 가능.
- **BasicRenderer 시그너처**: handler.rs 의 `redraw` 가 직접 호출. `voltex_renderer_ecs::BasicRenderer::render` 인자가 바뀌면 함께 갱신.
- **카메라**: 초기 위치/방향은 `RuntimeConfig::with_camera(pos, yaw, pitch)` / `with_camera_fov` (기본 `(0,10,20)`, yaw 0, pitch -0.4, 45°). 매 프레임 추적(follow/orbit)은 `RuntimeApp::set_camera_fn(\|cam, world, dt\| ...)` — tick 직후 실행되며 기본 RMB fly-camera를 override. **주의**: 기본 카메라가 멀어서(z=20) 1-unit 오브젝트는 화면에서 ~15px 점으로 보임 — 게임은 거의 항상 with_camera/set_camera_fn로 프레이밍해야 함 (voltex_capture `camera_framing_controls_visible_object_size` 회귀가 가드).
- **충돌/트리거 이벤트**: `RuntimeApp::set_collision_fn(\|contacts, &mut world, dt\| ...)` — 매 프레임 `detect_collisions`(Collider+Transform 엔티티) 결과를 game에 전달. `enable_physics` 무관하게 실행 → 강체 없이 트리거 존으로도 사용. physics_step 직후 실행.
- **조명**: `RuntimeConfig::with_light(dir, color, ambient)` (기본 `(0,-1,-1)`/흰색/0.1) + `RuntimeApp::set_light_fn(\|light, world, dt\| ...)` (day/night·flicker). camera_fn 직후 실행. 단일 directional light (BasicRenderer 한계) — 멀티 라이트는 PBR 파이프라인 전용.
- **사용자 시스템 시그너처**: `FnMut(&mut World, f32) + 'static`. dt 는 호출 프레임의 dt (fixed_dt 가 Some 면 그 값).
- **HUD 오버레이**: `with_hud(true)` + `set_hud_fn(|ui, world| ...)`. UiContext/UiRenderer 는 `RuntimeState`(GPU) 에 보관, `resumed` 에서 생성. `redraw` 가 BasicRenderer 다음 같은 encoder 로 `LoadOp::Load` UI 패스. 콜백은 `take()` 로 꺼내 `app.world` 와 disjoint-field 借用 충돌 회피.
- **Lua input 발행**: `enable_scripts` 시 매 프레임 `fill_script_input` 으로 `script_input` 갱신 → `voltex_script::set_current_input(ptr)` (실행 후 null). 키 이름↔winit keycode 매핑 `SCRIPT_KEY_NAMES` 가 여기 소유.
- **audio**: `with_audio(clips)` 시 `RuntimeApp::new` 에서 `AudioSystem::new` (스레드 spawn, GPU 무관, 실패 시 silent None). `set_audio_listener(entity)` 로 리스너 지정. `audio_sync_system` 이 `AudioSource::playing` 의 엣지(`active` 플래그)에서만 play/stop 발행 — held playing 이 매 프레임 voice 를 새로 만들지 않음. 한계: SpatialParams min/max 고정(1.0/50.0), AudioSource 씬 직렬화 미등록 — v1.1.
- **single-window only**: winit `run_app` 자체가 single window 지원. multi-window 는 v1.1+.

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_runtime --lib`
- 현재 테스트 개수: 59개 (collision_fn ×1 추가)
- GPU/winit 의존 테스트는 직접 검증 불가 → examples/runtime_demo 수동 실행 검증

## 예제
- `examples/runtime_demo` — 솔라 시스템 (sun + 3 planet + 1 moon) 자전 데모. `main` 본문 4줄.
