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
- `handler` (crate-private) — winit ApplicationHandler 구현 (src/handler.rs)

## 의존성
- voltex crate: voltex_math, voltex_platform, voltex_ecs, voltex_asset, voltex_renderer, voltex_renderer_ecs, voltex_physics, voltex_script, voltex_editor (HUD UiContext/UiRenderer)
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

## RuntimeApp::tick_systems_only(dt) 실행 순서
1. `propagate_transforms(world)` (항상)
2. `physics_step(world, &config)` (enable_physics 시)
3. `script_start_all(world, pool)` + `script_update_all(world, pool, dt)` + `pool.poll_reload(watcher)` (enable_scripts 시)
4. 사용자 등록 시스템 (등록 순서, `add_system` 으로 추가)

매 프레임 `run()` 의 winit handler 가 `tick_systems_only` 호출 후 `BasicRenderer::render` 로 화면 출력.

## 변경 시 주의점
- **Drop 순서**: `RuntimeApp` 의 필드 선언 순서가 곧 drop 순서. `runtime_state` (GPU) → `user_systems` → `file_watcher`/`timer`/`input`/`physics_config` → `script_pool` (LuaState 의 `__voltex_world` light userdata 가 World 보다 먼저 drop) → `assets`/`registry`/`world` → `config`. 이 순서는 LuaState use-after-free 회피용. 필드 추가/순서 변경 시 LuaState 가 World 보다 먼저 drop 됨을 유지.
- **GPU init lazy**: `RuntimeApp::new` 는 window/GPU 를 만들지 않는다. `with_gpu_init` 콜백이 첫 `resumed` 에서 한 번만 실행. 헤드리스 단위 테스트에서는 `tick_systems_only` 만 호출 가능.
- **BasicRenderer 시그너처**: handler.rs 의 `redraw` 가 직접 호출. `voltex_renderer_ecs::BasicRenderer::render` 인자가 바뀌면 함께 갱신.
- **카메라/조명 정책**: v1 에서는 고정 — `Camera::new(Vec3::new(0,10,20), aspect)` + pitch=-0.4, `LightUniform::new()` 기본값. 사용자가 RuntimeState 직접 접근 못 함 (private). 향후 v1.1 에서 카메라 커스터마이즈 API 추가 예정.
- **사용자 시스템 시그너처**: `FnMut(&mut World, f32) + 'static`. dt 는 호출 프레임의 dt (fixed_dt 가 Some 면 그 값).
- **HUD 오버레이**: `with_hud(true)` + `set_hud_fn(|ui, world| ...)`. UiContext/UiRenderer 는 `RuntimeState`(GPU) 에 보관, `resumed` 에서 생성. `redraw` 가 BasicRenderer 다음 같은 encoder 로 `LoadOp::Load` UI 패스. 콜백은 `take()` 로 꺼내 `app.world` 와 disjoint-field 借用 충돌 회피.
- **Lua input 발행**: `enable_scripts` 시 매 프레임 `fill_script_input` 으로 `script_input` 갱신 → `voltex_script::set_current_input(ptr)` (실행 후 null). 키 이름↔winit keycode 매핑 `SCRIPT_KEY_NAMES` 가 여기 소유.
- **single-window only**: winit `run_app` 자체가 single window 지원. multi-window 는 v1.1+.

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_runtime --lib`
- 현재 테스트 개수: 50개 (Lua input 매핑/전체체인 + HUD 등록 + config 포함)
- GPU/winit 의존 테스트는 직접 검증 불가 → examples/runtime_demo 수동 실행 검증

## 예제
- `examples/runtime_demo` — 솔라 시스템 (sun + 3 planet + 1 moon) 자전 데모. `main` 본문 4줄.
