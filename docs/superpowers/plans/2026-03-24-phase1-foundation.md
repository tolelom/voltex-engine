# Phase 1: Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 윈도우에 색이 있는 삼각형을 렌더링하고, ESC로 종료할 수 있는 최소 엔진 기반을 구축한다.

**Architecture:** Cargo 워크스페이스로 3개의 crate를 만든다: `voltex_math` (수학), `voltex_platform` (윈도우/입력/게임루프), `voltex_renderer` (wgpu 렌더링). 각 crate는 독립 빌드/테스트 가능하며, 의존 방향은 renderer → platform → math 단방향이다.

**Tech Stack:** Rust 1.94, wgpu 28.0, winit 0.30, bytemuck 1.x, pollster 0.4, env_logger 0.11

**Spec:** `docs/superpowers/specs/2026-03-24-voltex-engine-design.md`

---

## File Structure

```
voltex/
├── Cargo.toml                          # 워크스페이스 루트
├── crates/
│   ├── voltex_math/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # 모듈 re-export
│   │       ├── vec2.rs                 # Vec2 구현
│   │       ├── vec3.rs                 # Vec3 구현
│   │       ├── vec4.rs                 # Vec4 구현
│   │       └── mat4.rs                 # Mat4 구현
│   ├── voltex_platform/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # 모듈 re-export
│   │       ├── window.rs               # Window 래퍼
│   │       ├── input.rs                # InputState
│   │       └── game_loop.rs            # 고정 타임스텝 게임 루프
│   └── voltex_renderer/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                  # 모듈 re-export
│           ├── gpu.rs                  # GpuContext (wgpu 초기화)
│           ├── pipeline.rs             # RenderPipeline 생성
│           ├── vertex.rs               # Vertex 구조체 + 버퍼 레이아웃
│           └── shader.wgsl             # WGSL 셰이더
├── examples/
│   └── triangle/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs                 # 삼각형 데모 앱
└── docs/
```

---

## Task 1: Cargo 워크스페이스 설정

**Files:**
- Create: `Cargo.toml` (워크스페이스 루트)
- Create: `crates/voltex_math/Cargo.toml`
- Create: `crates/voltex_math/src/lib.rs`
- Create: `crates/voltex_platform/Cargo.toml`
- Create: `crates/voltex_platform/src/lib.rs`
- Create: `crates/voltex_renderer/Cargo.toml`
- Create: `crates/voltex_renderer/src/lib.rs`
- Create: `examples/triangle/Cargo.toml`
- Create: `examples/triangle/src/main.rs`

- [ ] **Step 1: 워크스페이스 루트 Cargo.toml 작성**

```toml
# Cargo.toml (워크스페이스 루트)
[workspace]
resolver = "2"
members = [
    "crates/voltex_math",
    "crates/voltex_platform",
    "crates/voltex_renderer",
    "examples/triangle",
]

[workspace.dependencies]
voltex_math = { path = "crates/voltex_math" }
voltex_platform = { path = "crates/voltex_platform" }
voltex_renderer = { path = "crates/voltex_renderer" }
wgpu = "28.0"
winit = "0.30"
bytemuck = { version = "1", features = ["derive"] }
pollster = "0.4"
env_logger = "0.11"
log = "0.4"
```

- [ ] **Step 2: voltex_math crate 생성**

```toml
# crates/voltex_math/Cargo.toml
[package]
name = "voltex_math"
version = "0.1.0"
edition = "2021"

[dependencies]

[dev-dependencies]
```

```rust
// crates/voltex_math/src/lib.rs
// Voltex Math Library - Phase 1
```

- [ ] **Step 3: voltex_platform crate 생성**

```toml
# crates/voltex_platform/Cargo.toml
[package]
name = "voltex_platform"
version = "0.1.0"
edition = "2021"

[dependencies]
voltex_math.workspace = true
winit.workspace = true
log.workspace = true
```

```rust
// crates/voltex_platform/src/lib.rs
pub mod window;
pub mod input;
```

- [ ] **Step 4: voltex_renderer crate 생성**

```toml
# crates/voltex_renderer/Cargo.toml
[package]
name = "voltex_renderer"
version = "0.1.0"
edition = "2021"

[dependencies]
voltex_math.workspace = true
voltex_platform.workspace = true
wgpu.workspace = true
winit.workspace = true
bytemuck.workspace = true
pollster.workspace = true
log.workspace = true
```

```rust
// crates/voltex_renderer/src/lib.rs
pub mod gpu;
pub mod pipeline;
pub mod vertex;
```

- [ ] **Step 5: triangle 예제 crate 생성**

```toml
# examples/triangle/Cargo.toml
[package]
name = "triangle"
version = "0.1.0"
edition = "2021"

[dependencies]
voltex_math.workspace = true
voltex_platform.workspace = true
voltex_renderer.workspace = true
wgpu.workspace = true
winit.workspace = true
bytemuck.workspace = true
pollster.workspace = true
env_logger.workspace = true
log.workspace = true
```

```rust
// examples/triangle/src/main.rs
fn main() {
    println!("Voltex Triangle Demo");
}
```

- [ ] **Step 6: 빌드 확인**

Run: `cargo build --workspace`
Expected: 모든 crate 빌드 성공

- [ ] **Step 7: 커밋**

```bash
git add Cargo.toml crates/ examples/
git commit -m "feat: initialize cargo workspace with voltex_math, voltex_platform, voltex_renderer"
```

---

## Task 2: voltex_math - Vec3 구현

**Files:**
- Create: `crates/voltex_math/src/vec3.rs`
- Modify: `crates/voltex_math/src/lib.rs`

Phase 1에서는 렌더링에 필요한 최소한의 수학만 구현한다. Vec3는 정점 위치와 색상에 필요하다.
나머지 수학 타입 (Vec2, Vec4, Mat3, Mat4, Quat, Transform, AABB, Ray, Plane)은 Phase 2에서 카메라/3D 변환이 필요할 때 구현한다.

- [ ] **Step 1: 테스트 먼저 작성**

```rust
// crates/voltex_math/src/vec3.rs

/// 3D 벡터 (f32)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.z, 3.0);
    }

    #[test]
    fn test_zero() {
        let v = Vec3::ZERO;
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.z, 0.0);
    }

    #[test]
    fn test_add() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        let c = a + b;
        assert_eq!(c, Vec3::new(5.0, 7.0, 9.0));
    }

    #[test]
    fn test_sub() {
        let a = Vec3::new(4.0, 5.0, 6.0);
        let b = Vec3::new(1.0, 2.0, 3.0);
        let c = a - b;
        assert_eq!(c, Vec3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn test_scalar_mul() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let r = v * 2.0;
        assert_eq!(r, Vec3::new(2.0, 4.0, 6.0));
    }

    #[test]
    fn test_dot() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a.dot(b), 32.0); // 1*4 + 2*5 + 3*6
    }

    #[test]
    fn test_cross() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        let c = a.cross(b);
        assert_eq!(c, Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_length() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        assert!((v.length() - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize() {
        let v = Vec3::new(3.0, 0.0, 0.0);
        let n = v.normalize();
        assert!((n.length() - 1.0).abs() < 1e-6);
        assert_eq!(n, Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn test_neg() {
        let v = Vec3::new(1.0, -2.0, 3.0);
        let n = -v;
        assert_eq!(n, Vec3::new(-1.0, 2.0, -3.0));
    }
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p voltex_math`
Expected: FAIL — `Vec3::new`, 연산자 등 미구현

- [ ] **Step 3: Vec3 구현**

```rust
// crates/voltex_math/src/vec3.rs (tests 위에 구현 추가)

use std::ops::{Add, Sub, Mul, Neg};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };
    pub const ONE: Self = Self { x: 1.0, y: 1.0, z: 1.0 };
    pub const X: Self = Self { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y: Self = Self { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z: Self = Self { x: 0.0, y: 0.0, z: 1.0 };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    pub fn cross(self, rhs: Self) -> Self {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        Self {
            x: self.x / len,
            y: self.y / len,
            z: self.z / len,
        }
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { x: self.x + rhs.x, y: self.y + rhs.y, z: self.z + rhs.z }
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { x: self.x - rhs.x, y: self.y - rhs.y, z: self.z - rhs.z }
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self { x: self.x * rhs, y: self.y * rhs, z: self.z * rhs }
    }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self { x: -self.x, y: -self.y, z: -self.z }
    }
}
```

- [ ] **Step 4: lib.rs에 모듈 등록**

```rust
// crates/voltex_math/src/lib.rs
pub mod vec3;
pub use vec3::Vec3;
```

- [ ] **Step 5: 테스트 통과 확인**

Run: `cargo test -p voltex_math`
Expected: 모든 테스트 PASS

- [ ] **Step 6: 커밋**

```bash
git add crates/voltex_math/
git commit -m "feat(math): implement Vec3 with basic operations"
```

---

## Task 3: voltex_platform - Window 래퍼

**Files:**
- Create: `crates/voltex_platform/src/window.rs`
- Create: `crates/voltex_platform/src/input.rs`
- Modify: `crates/voltex_platform/src/lib.rs`

winit 0.30의 ApplicationHandler 패턴을 래핑한다.

- [ ] **Step 1: input.rs 작성**

```rust
// crates/voltex_platform/src/input.rs
use winit::keyboard::KeyCode;
use std::collections::HashSet;

use winit::event::MouseButton;

pub struct InputState {
    // Keyboard
    pressed: HashSet<KeyCode>,
    just_pressed: HashSet<KeyCode>,
    just_released: HashSet<KeyCode>,
    // Mouse
    mouse_position: (f64, f64),
    mouse_delta: (f64, f64),
    mouse_buttons: HashSet<MouseButton>,
    mouse_buttons_just_pressed: HashSet<MouseButton>,
    mouse_buttons_just_released: HashSet<MouseButton>,
    mouse_scroll_delta: f32,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
            just_pressed: HashSet::new(),
            just_released: HashSet::new(),
            mouse_position: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
            mouse_buttons: HashSet::new(),
            mouse_buttons_just_pressed: HashSet::new(),
            mouse_buttons_just_released: HashSet::new(),
            mouse_scroll_delta: 0.0,
        }
    }

    // Keyboard
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed.contains(&key)
    }

    pub fn is_key_just_released(&self, key: KeyCode) -> bool {
        self.just_released.contains(&key)
    }

    // Mouse
    pub fn mouse_position(&self) -> (f64, f64) {
        self.mouse_position
    }

    pub fn mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }

    pub fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons.contains(&button)
    }

    pub fn is_mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_just_pressed.contains(&button)
    }

    pub fn mouse_scroll(&self) -> f32 {
        self.mouse_scroll_delta
    }

    /// 프레임 시작 시 호출 — per-frame 상태 초기화
    pub fn begin_frame(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.mouse_buttons_just_pressed.clear();
        self.mouse_buttons_just_released.clear();
        self.mouse_delta = (0.0, 0.0);
        self.mouse_scroll_delta = 0.0;
    }

    /// winit 키 이벤트 처리
    pub fn process_key(&mut self, key: KeyCode, pressed: bool) {
        if pressed {
            if self.pressed.insert(key) {
                self.just_pressed.insert(key);
            }
        } else {
            if self.pressed.remove(&key) {
                self.just_released.insert(key);
            }
        }
    }

    pub fn process_mouse_move(&mut self, x: f64, y: f64) {
        self.mouse_delta.0 += x - self.mouse_position.0;
        self.mouse_delta.1 += y - self.mouse_position.1;
        self.mouse_position = (x, y);
    }

    pub fn process_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if pressed {
            if self.mouse_buttons.insert(button) {
                self.mouse_buttons_just_pressed.insert(button);
            }
        } else {
            if self.mouse_buttons.remove(&button) {
                self.mouse_buttons_just_released.insert(button);
            }
        }
    }

    pub fn process_scroll(&mut self, delta: f32) {
        self.mouse_scroll_delta += delta;
    }
}
```

- [ ] **Step 2: window.rs 작성**

```rust
// crates/voltex_platform/src/window.rs
use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window as WinitWindow, WindowAttributes};

pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub vsync: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Voltex Engine".to_string(),
            width: 1280,
            height: 720,
            fullscreen: false,
            vsync: true,
        }
    }
}

/// winit Window를 래핑한다. Arc로 감싸서 wgpu Surface와 공유 가능.
pub struct VoltexWindow {
    pub handle: Arc<WinitWindow>,
    pub vsync: bool,
}

impl VoltexWindow {
    pub fn new(event_loop: &ActiveEventLoop, config: &WindowConfig) -> Self {
        let mut attrs = WindowAttributes::default()
            .with_title(&config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(config.width, config.height));

        if config.fullscreen {
            attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }

        let window = event_loop.create_window(attrs).expect("Failed to create window");
        Self {
            handle: Arc::new(window),
            vsync: config.vsync,
        }
    }

    pub fn inner_size(&self) -> (u32, u32) {
        let size = self.handle.inner_size();
        (size.width, size.height)
    }

    pub fn request_redraw(&self) {
        self.handle.request_redraw();
    }
}
```

- [ ] **Step 3: lib.rs 업데이트**

```rust
// crates/voltex_platform/src/lib.rs
pub mod window;
pub mod input;

pub use window::{VoltexWindow, WindowConfig};
pub use input::InputState;
```

참고: `game_loop` 모듈과 `GameTimer` re-export은 Task 3.5에서 추가한다.

- [ ] **Step 4: 빌드 확인**

Run: `cargo build -p voltex_platform`
Expected: 빌드 성공

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_platform/
git commit -m "feat(platform): implement Window wrapper and InputState"
```

---

## Task 3.5: voltex_platform - 게임 루프 타이머

**Files:**
- Create: `crates/voltex_platform/src/game_loop.rs`
- Modify: `crates/voltex_platform/src/lib.rs`

spec에서 정의한 고정 타임스텝 + 가변 렌더링 패턴을 구현한다.

- [ ] **Step 1: game_loop.rs 작성**

```rust
// crates/voltex_platform/src/game_loop.rs
use std::time::{Duration, Instant};

pub struct GameTimer {
    last_frame: Instant,
    accumulator: Duration,
    fixed_dt: Duration,
    frame_time: Duration,
}

impl GameTimer {
    pub fn new(fixed_hz: u32) -> Self {
        Self {
            last_frame: Instant::now(),
            accumulator: Duration::ZERO,
            fixed_dt: Duration::from_secs_f64(1.0 / fixed_hz as f64),
            frame_time: Duration::ZERO,
        }
    }

    /// 매 프레임 시작 시 호출. 경과 시간을 측정하고 accumulator에 누적한다.
    pub fn tick(&mut self) {
        let now = Instant::now();
        self.frame_time = now - self.last_frame;
        // 스파이크 방지: 최대 250ms로 제한
        if self.frame_time > Duration::from_millis(250) {
            self.frame_time = Duration::from_millis(250);
        }
        self.accumulator += self.frame_time;
        self.last_frame = now;
    }

    /// fixed_update를 실행해야 하면 true 반환하고 accumulator를 감소.
    /// while timer.should_fixed_update() { fixed_update(timer.fixed_dt()); } 패턴으로 사용.
    pub fn should_fixed_update(&mut self) -> bool {
        if self.accumulator >= self.fixed_dt {
            self.accumulator -= self.fixed_dt;
            true
        } else {
            false
        }
    }

    /// 고정 타임스텝 간격 (초)
    pub fn fixed_dt(&self) -> f32 {
        self.fixed_dt.as_secs_f32()
    }

    /// 이번 프레임의 경과 시간 (초)
    pub fn frame_dt(&self) -> f32 {
        self.frame_time.as_secs_f32()
    }

    /// 렌더링 보간용 alpha 값 (0.0 ~ 1.0)
    pub fn alpha(&self) -> f32 {
        self.accumulator.as_secs_f32() / self.fixed_dt.as_secs_f32()
    }
}
```

- [ ] **Step 2: lib.rs에 모듈 추가**

```rust
// crates/voltex_platform/src/lib.rs
pub mod window;
pub mod input;
pub mod game_loop;
```

- [ ] **Step 3: 테스트 작성 및 확인**

```rust
// game_loop.rs 하단에 추가
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_fixed_dt() {
        let timer = GameTimer::new(60);
        let expected = 1.0 / 60.0;
        assert!((timer.fixed_dt() - expected).abs() < 1e-6);
    }

    #[test]
    fn test_should_fixed_update_accumulates() {
        let mut timer = GameTimer::new(60);
        // Simulate 100ms frame
        thread::sleep(Duration::from_millis(100));
        timer.tick();
        // At 60Hz (16.6ms per tick), 100ms should yield ~6 fixed updates
        let mut count = 0;
        while timer.should_fixed_update() {
            count += 1;
        }
        assert!(count >= 5 && count <= 7, "Expected ~6 fixed updates, got {count}");
    }

    #[test]
    fn test_alpha_range() {
        let mut timer = GameTimer::new(60);
        thread::sleep(Duration::from_millis(10));
        timer.tick();
        // Drain fixed updates
        while timer.should_fixed_update() {}
        let alpha = timer.alpha();
        assert!(alpha >= 0.0 && alpha <= 1.0, "Alpha should be 0..1, got {alpha}");
    }
}
```

Run: `cargo test -p voltex_platform`
Expected: 모든 테스트 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_platform/src/game_loop.rs
git commit -m "feat(platform): implement GameTimer with fixed timestep"
```

---

## Task 4: voltex_renderer - GPU 초기화

**Files:**
- Create: `crates/voltex_renderer/src/gpu.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

wgpu Instance → Adapter → Device + Queue → Surface 초기화를 담당하는 GpuContext를 구현한다.

- [ ] **Step 1: gpu.rs 작성**

```rust
// crates/voltex_renderer/src/gpu.rs
use std::sync::Arc;
use winit::window::Window;

pub struct GpuContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub surface_format: wgpu::TextureFormat,
}

impl GpuContext {
    pub fn new(window: Arc<Window>) -> Self {
        pollster::block_on(Self::new_async(window))
    }

    async fn new_async(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window).expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find a suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Voltex Device"),
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Self {
            surface,
            device,
            queue,
            config,
            surface_format,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}
```

- [ ] **Step 2: lib.rs 업데이트**

```rust
// crates/voltex_renderer/src/lib.rs
pub mod gpu;
pub mod pipeline;
pub mod vertex;

pub use gpu::GpuContext;
```

- [ ] **Step 3: 빌드 확인**

Run: `cargo build -p voltex_renderer`
Expected: 빌드 성공

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): implement GpuContext with wgpu initialization"
```

---

## Task 5: voltex_renderer - Vertex + Shader + Pipeline

**Files:**
- Create: `crates/voltex_renderer/src/vertex.rs`
- Create: `crates/voltex_renderer/src/shader.wgsl`
- Create: `crates/voltex_renderer/src/pipeline.rs`

- [ ] **Step 1: vertex.rs 작성**

```rust
// crates/voltex_renderer/src/vertex.rs
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl Vertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
        ],
    };
}
```

- [ ] **Step 2: shader.wgsl 작성**

```wgsl
// crates/voltex_renderer/src/shader.wgsl
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.clip_position = vec4<f32>(model.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
```

- [ ] **Step 3: pipeline.rs 작성**

```rust
// crates/voltex_renderer/src/pipeline.rs
use crate::vertex::Vertex;

pub fn create_render_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Voltex Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Vertex::LAYOUT],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}
```

- [ ] **Step 4: 빌드 확인**

Run: `cargo build -p voltex_renderer`
Expected: 빌드 성공

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): add Vertex, WGSL shader, and render pipeline"
```

---

## Task 6: Triangle 데모 앱

**Files:**
- Modify: `examples/triangle/src/main.rs`

모든 crate를 통합하여 실제 삼각형을 렌더링하는 앱을 만든다.

- [ ] **Step 1: main.rs 작성**

```rust
// examples/triangle/src/main.rs
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::{GpuContext, pipeline, vertex::Vertex};
use wgpu::util::DeviceExt;

const TRIANGLE_VERTICES: &[Vertex] = &[
    Vertex { position: [0.0, 0.5, 0.0], color: [1.0, 0.0, 0.0] },
    Vertex { position: [-0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
    Vertex { position: [0.5, -0.5, 0.0], color: [0.0, 0.0, 1.0] },
];

struct TriangleApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    input: InputState,
    timer: GameTimer,
}

impl ApplicationHandler for TriangleApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Triangle".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new(window.handle.clone());
        let pipeline = pipeline::create_render_pipeline(&gpu.device, gpu.surface_format);
        let vertex_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Triangle Vertex Buffer"),
            contents: bytemuck::cast_slice(TRIANGLE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        self.state = Some(AppState {
            window,
            gpu,
            pipeline,
            vertex_buffer,
            input: InputState::new(),
            timer: GameTimer::new(60),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput {
                event: winit::event::KeyEvent {
                    physical_key: PhysicalKey::Code(key_code),
                    state: key_state,
                    ..
                },
                ..
            } => {
                let pressed = key_state == winit::event::ElementState::Pressed;
                state.input.process_key(key_code, pressed);
                if key_code == KeyCode::Escape && pressed {
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                state.gpu.resize(size.width, size.height);
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.input.process_mouse_move(position.x, position.y);
            }

            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                let pressed = btn_state == winit::event::ElementState::Pressed;
                state.input.process_mouse_button(button, pressed);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                state.input.process_scroll(y);
            }

            WindowEvent::RedrawRequested => {
                state.timer.tick();
                state.input.begin_frame();

                // Fixed update loop
                while state.timer.should_fixed_update() {
                    // fixed_update logic goes here (physics, etc.)
                    let _fixed_dt = state.timer.fixed_dt();
                }

                // Variable update
                let _frame_dt = state.timer.frame_dt();
                let _alpha = state.timer.alpha();

                let output = match state.gpu.surface.get_current_texture() {
                    Ok(t) => t,
                    Err(wgpu::SurfaceError::Lost) => {
                        let (w, h) = state.window.inner_size();
                        state.gpu.resize(w, h);
                        return;
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        event_loop.exit();
                        return;
                    }
                    Err(_) => return,
                };

                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") },
                );

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.1,
                                    b: 0.15,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });

                    render_pass.set_pipeline(&state.pipeline);
                    render_pass.set_vertex_buffer(0, state.vertex_buffer.slice(..));
                    render_pass.draw(0..3, 0..1);
                }

                state.gpu.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = TriangleApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
```

- [ ] **Step 2: 빌드 확인**

Run: `cargo build -p triangle`
Expected: 빌드 성공

- [ ] **Step 3: 실행 확인**

Run: `cargo run -p triangle`
Expected: 윈도우가 열리고, 빨강-초록-파랑 삼각형이 어두운 배경 위에 보임. ESC로 종료 가능.

- [ ] **Step 4: 커밋**

```bash
git add examples/triangle/
git commit -m "feat: add triangle demo - colored triangle rendering with ESC to exit"
```

---

## Task 7: .gitignore + 정리

**Files:**
- Modify: `.gitignore`

- [ ] **Step 1: .gitignore 업데이트**

```gitignore
# .gitignore
/target
*.exe
*.pdb
Cargo.lock
```

참고: 라이브러리 프로젝트에서는 Cargo.lock을 커밋하지 않는 것이 관례이나, 워크스페이스에 실행 바이너리(examples)가 포함되어 있으므로 `Cargo.lock`을 커밋해도 좋다. 팀 결정에 따라 .gitignore에서 제거할 수 있다.

- [ ] **Step 2: 커밋**

```bash
git add .gitignore
git commit -m "chore: update .gitignore for Rust workspace"
```

---

## Phase 1 완료 기준 체크리스트

- [ ] `cargo build --workspace` 성공
- [ ] `cargo test -p voltex_math` — 모든 테스트 통과
- [ ] `cargo run -p triangle` — 윈도우 열림, 삼각형 렌더링, ESC 종료
- [ ] 각 crate가 독립적으로 빌드 가능
