# Cycle 6 (B0) — voltex_runtime 라이브러리 crate 설계

**작성일**: 2026-05-21
**상위 로드맵**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 6
**Sub-project**: B (Build pipeline)
**세션 수**: 3
**의존**: Cycle 2 (C0b — voltex_renderer_ecs), Cycle 5 (S1 — ScriptInstancePool / script_update_all) — 모두 완료 예정

---

## 1. 목적

신규 crate `voltex_runtime` — 표준 부트스트랩 + 메인 루프 + 기본 시스템 등록. 외부 사용자가 다섯 줄 이내의 `main` 으로 윈도우/렌더링/스크립트/physics 통합 게임을 띄울 수 있도록 한다. CLI 빌드 (Cycle 7~8) 가 생성하는 wrapper 가 이 crate 를 사용한다.

## 2. Scope / Out-of-scope

### 2.1 In-scope

- 신규 crate `voltex_runtime` (L3): voltex 의 모든 sub-crate 의존 (voltex facade 를 거치지 않고 직접 sub-crate 사용 — 의존 그래프 단순화)
- `RuntimeConfig` (builder 패턴): window 설정, 초기 scene 경로, 사용자 콜백 (registry setup / lifecycle setup / per-frame system setup)
- `RuntimeApp` 구조체: 모든 subsystem 보관 + winit ApplicationHandler 통합
- `RuntimeApp::run(config) -> RuntimeResult<()>` — 부트스트랩 → 이벤트 루프 → 깨끗한 셧다운
- 기본 시스템 셋 (헤드리스에서도 안전한 것들):
  - `world_transform_system` (자체 호출 — Scheduler에 등록)
  - `script_update_system` (pool + dt 캡쳐 closure)
  - `audio_sync_system` (no-op v1 — AudioSystem 이 자체 스레드라 World 동기화는 future)
  - `physics_step_system` (PhysicsConfig 캡쳐 closure)
  - `render_system` (별도 — Scheduler 에 안 넣음. RuntimeApp 내부 render() 메서드)
- `RuntimeError` (thiserror): WindowInit/GpuInit/SceneLoad/Generic
- `examples/runtime_demo`: hierarchy_demo 의 solar system 을 RuntimeApp 으로 재구성. `main` 본문 ≤ 5 줄.
- 단위 테스트 ≥5 (config builder, register_system count, shutdown safe drop 순서, error type 통합 등)

### 2.2 Out-of-scope (Cycle 7+ / v1.1+)

- CLI / voltex.toml (Cycle 7-8)
- asset packing / VFS (Cycle 9)
- 멀티 윈도우
- 폴링이 아닌 진정한 async loop
- 헤드리스 모드 (CI 용) — winit 이벤트 루프 없이 동작하는 path. v1 미지원, 단위 테스트는 RuntimeApp 의 부품만 검증.
- audio_sync_system 의 실제 World ↔ AudioSystem 동기화 (3D listener 위치 갱신 등)
- 핫리로드 watcher 등록의 builder API (사용자가 직접 pool.watch 호출)
- Scheduler 의 병렬화 / DependencyScheduler 통합

## 3. 아키텍처

### 3.1 의존성

```
voltex_runtime (L3)
  ├ voltex_math
  ├ voltex_platform
  ├ voltex_ecs
  ├ voltex_asset
  ├ voltex_renderer
  ├ voltex_renderer_ecs
  ├ voltex_physics
  ├ voltex_audio
  ├ voltex_script
  └ winit / wgpu (전이 의존)
```

voltex facade (L4) 와는 평행: voltex_runtime 도 모든 sub-crate 직접 사용. 외부 사용자 코드는 facade 또는 runtime 둘 다 import 가능 (`use voltex_runtime::RuntimeApp;` + `use voltex::prelude::*;`).

### 3.2 RuntimeConfig (builder)

```rust
// crates/voltex_runtime/src/config.rs

use voltex_ecs::{ComponentRegistry, World};
use voltex_platform::WindowConfig;

pub type RegistrySetupFn = Box<dyn FnOnce(&mut ComponentRegistry)>;
pub type SceneBuilderFn = Box<dyn FnOnce(&mut World, &mut ComponentRegistry)>;
pub type SystemSetupFn = Box<dyn FnOnce(&mut crate::app::RuntimeApp)>;

pub struct RuntimeConfig {
    pub window: WindowConfig,
    pub initial_scene: Option<String>,
    pub register_components: Option<RegistrySetupFn>,
    pub build_scene: Option<SceneBuilderFn>,
    pub setup_systems: Option<SystemSetupFn>,
    pub fixed_dt: Option<f32>,         // None = variable timestep (timer 사용)
    pub enable_physics: bool,
    pub enable_scripts: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            initial_scene: None,
            register_components: None,
            build_scene: None,
            setup_systems: None,
            fixed_dt: None,
            enable_physics: true,
            enable_scripts: true,
        }
    }
}

impl RuntimeConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_window_title(mut self, title: impl Into<String>) -> Self {
        self.window.title = title.into(); self
    }
    pub fn with_window_size(mut self, w: u32, h: u32) -> Self {
        self.window.width = w; self.window.height = h; self
    }
    pub fn with_initial_scene(mut self, path: impl Into<String>) -> Self {
        self.initial_scene = Some(path.into()); self
    }
    pub fn with_registry_setup<F: FnOnce(&mut ComponentRegistry) + 'static>(mut self, f: F) -> Self {
        self.register_components = Some(Box::new(f)); self
    }
    pub fn with_scene_builder<F: FnOnce(&mut World, &mut ComponentRegistry) + 'static>(mut self, f: F) -> Self {
        self.build_scene = Some(Box::new(f)); self
    }
    pub fn with_system_setup<F: FnOnce(&mut crate::app::RuntimeApp) + 'static>(mut self, f: F) -> Self {
        self.setup_systems = Some(Box::new(f)); self
    }
    pub fn with_fixed_dt(mut self, dt: f32) -> Self {
        self.fixed_dt = Some(dt); self
    }
    pub fn disable_physics(mut self) -> Self { self.enable_physics = false; self }
    pub fn disable_scripts(mut self) -> Self { self.enable_scripts = false; self }
}
```

### 3.3 RuntimeApp

```rust
// crates/voltex_runtime/src/app.rs

use std::sync::Arc;
use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop}, window::WindowId};
use voltex_ecs::{ComponentRegistry, World, propagate_transforms};
use voltex_platform::{GameTimer, InputState, VoltexWindow, WindowConfig};
use voltex_asset::FileWatcher;
use voltex_renderer::{Camera, CameraUniform, FpsController, GpuContext, Mesh};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};
use voltex_physics::PhysicsConfig;
use voltex_script::ScriptInstancePool;

use crate::config::RuntimeConfig;
use crate::error::{RuntimeError, RuntimeResult};

pub struct RuntimeApp {
    pub world: World,
    pub registry: ComponentRegistry,
    pub assets: RenderAssets,
    pub script_pool: ScriptInstancePool,
    pub physics_config: PhysicsConfig,
    pub file_watcher: FileWatcher,
    pub timer: GameTimer,
    pub input: InputState,
    user_systems: Vec<Box<dyn FnMut(&mut World, f32) + 'static>>,
    config: RuntimeConfig,
    // window/gpu/renderer/camera/buffer 들은 winit resumed 시 lazy init
    runtime_state: Option<RuntimeState>,
}

struct RuntimeState {
    window: VoltexWindow,
    gpu: GpuContext,
    renderer: BasicRenderer,
    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    fps_controller: FpsController,
    // (light buffer, depth view 등은 BasicRenderer가 관리하거나 여기서 직접 — 구현 시 결정)
}

impl RuntimeApp {
    /// Pre-window setup. The actual window/gpu init happens in winit's
    /// `resumed` callback. This step runs all `register_components` /
    /// `build_scene` / `setup_systems` callbacks so the World is ready before
    /// the first frame.
    pub fn new(mut config: RuntimeConfig) -> RuntimeResult<Self> {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();   // Transform/Tag/MeshHandle/MaterialHandle/Bounds/ScriptComponent
        if let Some(setup) = config.register_components.take() {
            setup(&mut registry);
        }

        let mut world = World::new();
        if let Some(scene_path) = config.initial_scene.as_ref() {
            // optional: load from disk; tolerate missing — user can also use
            // build_scene to programmatically seed.
            let _ = load_scene_into(&mut world, &registry, scene_path);
        }
        if let Some(build) = config.build_scene.take() {
            build(&mut world, &mut registry);
        }

        let mut app = Self {
            world,
            registry,
            assets: RenderAssets::new(),
            script_pool: ScriptInstancePool::new(),
            physics_config: PhysicsConfig::default(),
            file_watcher: FileWatcher::new(std::time::Duration::from_millis(500)),
            timer: GameTimer::new(),
            input: InputState::new(),
            user_systems: Vec::new(),
            config,
            runtime_state: None,
        };
        if let Some(setup) = app.config.setup_systems.take() {
            setup(&mut app);
        }
        Ok(app)
    }

    /// Register a user system that runs every frame with (world, dt). Order
    /// of registration is the order of invocation. Default systems
    /// (world_transform, physics, scripts) run BEFORE user systems unless the
    /// user re-registers them explicitly.
    pub fn add_system<F: FnMut(&mut World, f32) + 'static>(&mut self, system: F) {
        self.user_systems.push(Box::new(system));
    }

    /// Consume the app and run the winit event loop until window close.
    /// On exit, all subsystems drop in dependency order automatically
    /// (script_pool before world: see crate-level docs).
    pub fn run(mut self) -> RuntimeResult<()> {
        let event_loop = EventLoop::new().map_err(|e| RuntimeError::WindowInit(e.to_string()))?;
        let mut handler = RuntimeHandler { app: Some(self) };
        event_loop.run_app(&mut handler)
            .map_err(|e| RuntimeError::WindowInit(e.to_string()))?;
        Ok(())
    }

    /// Tick a single frame WITHOUT the winit loop. Tests / headless code can
    /// call this; the runtime_state must already be initialised (only valid
    /// after the first `resumed` callback, so this method is rarely useful
    /// in headless mode). For unit tests we provide a small `tick_systems_only`
    /// helper that runs the schedulable systems without rendering.
    pub fn tick_systems_only(&mut self, dt: f32) {
        propagate_transforms(&mut self.world);
        if self.config.enable_physics {
            voltex_physics::physics_step(&mut self.world, &self.physics_config);
        }
        if self.config.enable_scripts {
            voltex_script::script_start_all(&mut self.world, &mut self.script_pool);
            voltex_script::script_update_all(&mut self.world, &mut self.script_pool, dt);
        }
        // poll hot reload (silent)
        let _ = self.script_pool.poll_reload(&mut self.file_watcher);
        // user systems
        for sys in self.user_systems.iter_mut() {
            sys(&mut self.world, dt);
        }
    }
}

struct RuntimeHandler {
    app: Option<RuntimeApp>,
}

impl ApplicationHandler for RuntimeHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let app = self.app.as_mut().expect("RuntimeApp dropped before resumed");
        if app.runtime_state.is_some() { return; }
        let window = VoltexWindow::new(event_loop, &app.config.window);
        let gpu = GpuContext::new(window.handle.clone());
        let renderer = BasicRenderer::new(&gpu);
        let camera = Camera::default();   // user can override via system
        let camera_uniform = CameraUniform::default();
        let camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RuntimeApp camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let fps_controller = FpsController::default();
        app.runtime_state = Some(RuntimeState {
            window, gpu, renderer, camera, camera_uniform, camera_buffer, fps_controller,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(app) = self.app.as_mut() else { return; };
        let Some(state) = app.runtime_state.as_mut() else { return; };
        match event {
            WindowEvent::CloseRequested => { event_loop.exit(); }
            WindowEvent::RedrawRequested => {
                let dt = app.timer.tick();
                let dt = app.config.fixed_dt.unwrap_or(dt as f32);
                // input → camera update
                state.fps_controller.update(&mut state.camera, &app.input, dt);
                app.input.end_frame();
                app.tick_systems_only(dt);
                state.renderer.render(
                    &state.gpu, &mut app.world, &app.assets,
                    &state.camera_buffer, &state.camera_uniform,
                );
                state.window.handle.request_redraw();
            }
            _ => { app.input.handle_window_event(&event); }
        }
    }
}
```

위 코드는 설계 윤곽. 실제 `BasicRenderer::render` 시그너처는 cycle 2 의 구현을 보고 맞춰야 한다. v1 에서는 `BasicRenderer::new(&gpu)` 가 self-contained 하지 못하면 일부 buffer 를 `RuntimeState` 가 보관.

### 3.4 기본 시스템 정책

시스템 실행 순서 (frame 당):
1. 입력 폴링 (winit event handler)
2. timer.tick() → dt
3. `propagate_transforms(&mut world)` — 항상 (hierarchy 가 비어도 무해)
4. `physics_step` — `enable_physics` 가 true 면 (기본 true)
5. `script_start_all` + `script_update_all` — `enable_scripts` 가 true 면 (기본 true)
6. file_watcher poll → script_pool.poll_reload (silent)
7. 사용자 등록 시스템 (등록 순서)
8. 카메라 buffer 업데이트
9. `BasicRenderer::render` — 화면에 그림
10. swap chain present

`enable_physics=false` 또는 `enable_scripts=false` 로 셋팅한 사용자는 해당 단계가 자동 skip.

### 3.5 Error type

```rust
// crates/voltex_runtime/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("window init: {0}")]
    WindowInit(String),
    #[error("gpu init: {0}")]
    GpuInit(String),
    #[error("scene load: {0}")]
    SceneLoad(String),
    #[error("script error: {0}")]
    Script(#[from] voltex_script::ScriptError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Generic(String),
}

impl From<String> for RuntimeError { fn from(s: String) -> Self { Self::Generic(s) } }
impl From<&str> for RuntimeError { fn from(s: &str) -> Self { Self::Generic(s.to_string()) } }

pub type RuntimeResult<T> = Result<T, RuntimeError>;
```

### 3.6 runtime_demo

```rust
// examples/runtime_demo/src/main.rs
use voltex::prelude::*;
use voltex_runtime::{RuntimeApp, RuntimeConfig};

fn main() -> voltex_runtime::RuntimeResult<()> {
    let config = RuntimeConfig::new()
        .with_window_title("Voltex Runtime Demo — Solar System")
        .with_window_size(1280, 720)
        .with_scene_builder(setup_solar_system);
    RuntimeApp::new(config)?.run()
}

fn setup_solar_system(world: &mut World, _registry: &mut ComponentRegistry) {
    // sun
    let sun = world.spawn();
    world.add(sun, Transform::from_position(Vec3::ZERO));
    // earth as child of sun
    let earth = world.spawn();
    world.add(earth, Transform::from_position(Vec3::new(3.0, 0.0, 0.0)));
    add_child(world, sun, earth);
    // moon as child of earth
    let moon = world.spawn();
    world.add(moon, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
    add_child(world, earth, moon);
}
```

`main` 본문 4줄. orbit 애니메이션은 사용자 정의 시스템 또는 spinner.lua 통해 (Cycle 5 의 ScriptComponent + Lua) 부착 가능. v1 demo 는 정적 (회전 없음) — Cycle 5 의 spinner.lua 를 통합하려면 `script_path` 를 ScriptComponent 로 부착하면 됨.

### 3.7 Drop 순서 / 셧다운

Rust 의 struct field drop 순서는 선언 순서 (top-down). `RuntimeApp` 필드를 다음 순서로 둔다:

```
runtime_state: Option<RuntimeState>   // gpu, window, renderer 등 — 먼저 drop
user_systems
config
script_pool                            // LuaState drop — world 보다 먼저
world                                  // 마지막에 drop
```

LuaState 내부의 `__voltex_world` light userdata 가 World 보다 먼저 사라지도록 보장. world 가 먼저 drop 되면 dangling pointer (실제로는 LuaState drop 시 더는 외부 호출 안 하므로 무해하지만, 안전한 순서로 유지).

CODEMAP / spec 에 "Drop 순서가 의미 있다" 명시.

## 4. 데이터 흐름

### 4.1 부트스트랩
```
사용자 main:
  RuntimeConfig::new().with_*(...).with_scene_builder(...)
  → RuntimeApp::new(config)
       registry = ComponentRegistry::new(); registry.register_defaults()
       사용자 register_components 콜백 (옵션)
       world = World::new()
       초기 scene 디스크 로드 (옵션) / build_scene 콜백 (옵션)
       setup_systems 콜백 (옵션, add_system 으로 사용자 systems 등록)
  → RuntimeApp::run(self)
       EventLoop::new()
       event_loop.run_app(handler)
            resumed → VoltexWindow + GpuContext + BasicRenderer + Camera 초기화
            window_event(RedrawRequested) → 매 frame
                dt = timer.tick()
                tick_systems_only(dt)
                renderer.render(...)
                window.request_redraw()
            CloseRequested → exit
       정상 종료 시 Drop 호출
```

### 4.2 매 프레임 (window_event RedrawRequested)
```
state.fps_controller.update(camera, input, dt)
input.end_frame()
app.tick_systems_only(dt):
  propagate_transforms(world)
  if enable_physics: physics_step(world, &config)
  if enable_scripts: script_start_all + script_update_all
  script_pool.poll_reload(watcher)
  for sys in user_systems: sys(world, dt)
state.renderer.render(gpu, world, assets, camera_buffer, camera_uniform)
```

## 5. 모듈 구조

```
crates/voltex_runtime/
├── Cargo.toml
├── CODEMAP.md
└── src/
    ├── lib.rs        # re-exports
    ├── app.rs        # RuntimeApp + RuntimeHandler
    ├── config.rs     # RuntimeConfig + builder
    └── error.rs      # RuntimeError + RuntimeResult
```

LOC 추정:
- lib.rs ~20줄
- config.rs ~120줄
- app.rs ~250줄 (winit handler + tick_systems_only + new/run)
- error.rs ~40줄

600줄 한도 모두 만족.

```
examples/runtime_demo/
├── Cargo.toml
└── src/main.rs    # ~30줄
```

`Cargo.toml` workspace 에 등록.

## 6. 에러 처리

- `RuntimeApp::new` 가 실패할 수 있는 경로: 초기 scene 로드 실패는 silent (사용자가 build_scene 로 보충 가능). 사용자 콜백 panic 은 panic-as-bug (catch_unwind 안 함).
- `run()` 의 EventLoop 생성/실행 실패는 RuntimeError::WindowInit 으로 래핑.
- 매 프레임 에러 (lua script 에러 등) 는 silent (Cycle 5 와 동일).

## 7. 테스트 (≥5, 작성 6)

| # | 파일 | 케이스 |
|---|---|---|
| 1 | config.rs | `default_config_enables_physics_and_scripts` |
| 2 | config.rs | `builder_with_window_title_sets_title` |
| 3 | config.rs | `builder_with_fixed_dt_sets_value` |
| 4 | app.rs | `new_runs_register_components_and_scene_builder_in_order` (registry/world mutation 콜백 순서 검증) |
| 5 | app.rs | `tick_systems_only_runs_user_systems_in_order` (no GPU 필요) |
| 6 | error.rs | `runtime_error_from_string` (`From<String>` 동작) |

`run()` 자체는 winit + GPU 필요 → CI 적합 아님. 테스트는 부품 단위로.

## 8. 시각 검증 (수동)

- `cargo run -p runtime_demo` 가 윈도우 열고 정적 solar system 표시 (sun + earth + moon 3개 박스)
- Cycle 5 의 spinner.lua 부착해서 earth 자전 검증은 별도 task (이 cycle 의 done 기준 아님)
- 윈도우 X 클릭으로 정상 종료 (panic 없이)

## 9. 세션 분할

### Session 1: Crate skeleton + RuntimeConfig (4 tasks)
1. 새 crate 디렉토리 + Cargo.toml + workspace member 등록
2. `src/lib.rs`, `src/error.rs` (RuntimeError, RuntimeResult)
3. `src/config.rs` (RuntimeConfig + builder 메서드들)
4. 테스트 #1, #2, #3, #6 + `cargo build -p voltex_runtime` 깨끗
5. commit

### Session 2: RuntimeApp + tick_systems_only (3 tasks)
1. `src/app.rs` 의 `RuntimeApp` 구조체 + `new` + `add_system` + `tick_systems_only`
2. `propagate_transforms` / `physics_step` / `script_start_all` / `script_update_all` / `poll_reload` 통합
3. winit `RuntimeHandler` 구현 (`resumed`, `window_event`)
4. 테스트 #4, #5
5. commit

### Session 3: runtime_demo + drop 순서 + CODEMAP + 종료 (3 tasks)
1. `examples/runtime_demo` 신규 — solar system scene_builder
2. workspace 에 등록, `cargo run -p runtime_demo` 수동 검증
3. CODEMAP.md (voltex_runtime), STATUS.md, 로드맵 `[✅]`
4. commit

## 10. 위험 / 미지수

### 10.1 BasicRenderer::render 시그너처 호환
Cycle 2 에서 `BasicRenderer::render` 가 어떤 인자를 받는지 구현 시 확인. spec 의 가상 시그너처 (`gpu, world, assets, camera_buffer, camera_uniform`) 와 다를 수 있음. 다르면 RuntimeState 의 필드를 그에 맞춰 조정.

### 10.2 Camera/FpsController 통합
`Camera::default()` / `FpsController::default()` 가 존재하는지 확인 필요. 없으면 명시적 생성자 호출.

### 10.3 audio_sync_system v1 정책
AudioSystem 은 자체 워커 스레드 → World 와 동기화는 listener position 만 의미 있다. v1 에서는 audio_sync_system 자체를 등록하지 않고 향후 cycle 에 미룬다. 명시: CODEMAP 의 "현재 통합 시스템 목록" 에 "AudioSystem 은 사용자가 직접 생성/관리" 라고 기재.

### 10.4 GPU init 실패
헤드리스 환경 / wgpu 미지원 GPU 에서 `GpuContext::new` 가 panic 할 수 있음. v1 에서는 panic 그대로 (catch_unwind 없음). 사용자 OS 미스매치는 빌드 단계에서 알림.

### 10.5 drop 순서 보장
`RuntimeApp` 의 필드 선언 순서가 정확히 의도한 drop 순서가 되도록 유의. 컴파일러는 선언 순서대로 drop. 테스트 어렵 — 명시적 `impl Drop for RuntimeApp` 으로 명시적 drop sequence 도 옵션 (v1 에서는 선언 순서만 의존).

### 10.6 main 의 "5줄" 정의
roadmap 의 "사용자 main 이 5줄 미만" 은 본문 줄 수. spec 의 runtime_demo main 본문 = 4줄 (`let config = ...; RuntimeApp::new(config)?.run()`). scene_builder 등 헬퍼 함수는 main 외부.

## 11. 변경 영향도

| Crate / Module | 변경 |
|---|---|
| voltex_runtime | 신규 crate |
| Cargo.toml (workspace) | + voltex_runtime member + workspace dep |
| examples/runtime_demo | 신규 example |
| 다른 crate | 변경 없음 |
| docs/STATUS.md | Cycle 6 entry |
| docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md | `[✅]` |

facade crate `voltex` 는 `pub use voltex_runtime as runtime;` 한 줄 추가 옵션 (의도적 미적용 — voltex 는 "라이브러리 컬렉션", voltex_runtime 은 "조립된 앱" → 별도 패키지로 명확히 분리. Cycle 13 의 A 작업에서 재검토).

## 12. Done 기준 (로드맵 § Cycle 6)

- [ ] `examples/runtime_demo` 가 hierarchy_demo 와 시각적으로 동일 (사용자 수동 검증)
- [ ] runtime_demo `main` 본문 ≤ 5 줄
- [ ] voltex_runtime 단위 테스트 ≥5 (작성 6)
- [ ] `cargo build --workspace` 깨끗
- [ ] `cargo test --workspace --lib` 통과 — Cycle 5 의 ~860 + 6 ≈ 866
- [ ] STATUS.md, 로드맵 Cycle 6 `[✅]` + done commit hash
- [ ] CODEMAP.md (voltex_runtime) 작성
