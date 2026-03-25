# Phase 8-4: Immediate Mode UI — Design Spec

## Overview

`voltex_editor` crate를 신규 생성한다. Immediate Mode UI 프레임워크를 구현하여 에디터의 기반을 마련한다.

## Scope

- 비트맵 폰트 아틀라스 (8x12 고정폭 ASCII, 코드 생성)
- DrawList (정점/인덱스/커맨드 버퍼)
- UiContext (IMGUI 상태, hot/active 위젯)
- 위젯: text, button, slider, checkbox, begin_panel/end_panel
- 커서 기반 자동 레이아웃
- wgpu UI 렌더 파이프라인 (2D, alpha blending)
- editor_demo 예제

## Out of Scope

- 씬 뷰포트 (3D 임베드)
- 엔티티 인스펙터 / 에셋 브라우저
- 텍스트 입력 (키보드 → 문자열)
- 스크롤, 드래그앤드롭
- TTF 파싱 / 가변 크기 폰트
- 도킹, 탭, 윈도우 드래그

## Module Structure

```
crates/voltex_editor/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── font.rs          — FontAtlas (비트맵 생성 + GPU 텍스처)
    ├── draw_list.rs     — DrawVertex, DrawCommand, DrawList
    ├── ui_context.rs    — UiContext (IMGUI 상태)
    ├── widgets.rs       — text, button, slider, checkbox, panel
    ├── layout.rs        — 커서 기반 레이아웃
    └── renderer.rs      — UI 렌더 파이프라인 (wgpu 셰이더)
```

## Dependencies

- `voltex_math` — Vec2
- `wgpu`, `bytemuck` — GPU 렌더링

## Types

### FontAtlas

```rust
pub struct FontAtlas {
    pub width: u32,       // 아틀라스 텍스처 너비
    pub height: u32,
    pub glyph_width: u32, // 8
    pub glyph_height: u32, // 12
    pub pixels: Vec<u8>,  // R8 grayscale
}
```

- `generate() -> Self` — 하드코딩된 8x12 비트맵 데이터로 ASCII 32~126 생성
- `glyph_uv(ch: char) -> (f32, f32, f32, f32)` — (u0, v0, u1, v1) UV 좌표

아틀라스 레이아웃: 16열 x 6행 = 96 글리프. 텍스처 크기: 128x72 (16*8 x 6*12).

### DrawVertex

```rust
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct DrawVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4], // RGBA
}
```

### DrawCommand

```rust
pub struct DrawCommand {
    pub index_offset: u32,
    pub index_count: u32,
    pub clip_rect: [f32; 4], // x, y, w, h (미사용, 추후)
}
```

### DrawList

```rust
pub struct DrawList {
    pub vertices: Vec<DrawVertex>,
    pub indices: Vec<u16>,
    pub commands: Vec<DrawCommand>,
}
```

- `clear()` — 매 프레임 리셋
- `add_rect(x, y, w, h, color)` — 색상 사각형
- `add_rect_uv(x, y, w, h, u0, v0, u1, v1, color)` — 텍스처 사각형
- `add_text(font, text, x, y, color)` — 문자열 (글리프별 쿼드)

### UiContext

```rust
pub struct UiContext {
    // Widget state
    pub hot: Option<u64>,
    pub active: Option<u64>,

    // Draw output
    pub draw_list: DrawList,

    // Layout cursor
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub indent: f32,
    pub line_height: f32,

    // Input (매 프레임 외부에서 설정)
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_down: bool,
    pub mouse_clicked: bool,
    pub mouse_released: bool,

    // Screen
    pub screen_width: f32,
    pub screen_height: f32,

    // Font
    pub font: FontAtlas,

    // Internal
    id_counter: u64,
}
```

- `new(screen_w, screen_h) -> Self`
- `begin_frame(mouse_x, mouse_y, mouse_down)` — 프레임 시작, draw_list 클리어
- `end_frame()` — draw_list 확정
- `gen_id() -> u64` — 위젯별 고유 ID (호출 순서 기반)

### Widget Functions (UiContext methods)

```rust
impl UiContext {
    pub fn text(&mut self, text: &str)
    pub fn button(&mut self, label: &str) -> bool
    pub fn slider(&mut self, label: &str, value: f32, min: f32, max: f32) -> f32
    pub fn checkbox(&mut self, label: &str, checked: bool) -> bool
    pub fn begin_panel(&mut self, title: &str, x: f32, y: f32, w: f32, h: f32)
    pub fn end_panel(&mut self)
}
```

**button 로직:**
1. ID 생성
2. 사각형 그리기 (배경색 = hot이면 밝게, active면 더 밝게)
3. 텍스트 그리기
4. 마우스가 사각형 안이면 hot 설정
5. hot + mouse_clicked이면 active 설정
6. active + mouse_released이면 → true 반환 (클릭됨)

**slider 로직:**
1. 배경 바 그리기
2. 현재 값 위치에 핸들 그리기
3. active이면 마우스 X로 값 업데이트
4. 변경된 값 반환

### UI Render Pipeline

```wgsl
// ui_shader.wgsl
struct UiUniform {
    projection: mat4x4<f32>, // orthographic screen projection
};

@group(0) @binding(0) var<uniform> ui: UiUniform;
@group(0) @binding(1) var t_atlas: texture_2d<f32>;
@group(0) @binding(2) var s_atlas: sampler;

@vertex
fn vs_main(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<u32>) -> ... {
    // Transform by orthographic projection
    // Pass UV and color
}

@fragment
fn fs_main(...) -> @location(0) vec4<f32> {
    let tex = textureSample(t_atlas, s_atlas, uv);
    return vec4(color.rgb * tex.r, color.a * tex.r); // font alpha from texture
}
```

프로젝션: `orthographic(0, screen_w, screen_h, 0, -1, 1)` — 좌상단 원점.

비텍스처 사각형(배경, 버튼)은 UV=(0,0) 영역(아틀라스의 화이트 픽셀)을 사용하거나, 셰이더에서 UV=(0,0)일 때 색상만 사용.

## Test Plan

### font.rs
- generate(): 아틀라스 크기 확인 (128x72)
- glyph_uv('A'): UV 범위 확인

### draw_list.rs
- add_rect: 정점 4개 + 인덱스 6개 추가
- add_text: 글자 수 * 4 정점
- clear: 비어있음

### ui_context.rs
- button: mouse over → hot, click → returns true
- slider: value 변경 확인

### 통합
- editor_demo 예제: 텍스트, 버튼, 슬라이더가 화면에 표시
