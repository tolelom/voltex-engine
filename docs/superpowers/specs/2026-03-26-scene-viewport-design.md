# Scene Viewport Design

## Overview

도킹 패널 안에 3D 씬을 렌더링한다. 오프스크린 텍스처에 Blinn-Phong forward 렌더링 후, UI 시스템에서 텍스처드 쿼드로 표시. 오빗 카메라로 조작.

## Scope

- 오프스크린 렌더 타겟 (color + depth)
- 텍스처를 UI 패널 영역에 표시하는 셰이더/파이프라인
- 오빗 카메라 (회전, 줌, 팬)
- 매 프레임 패널 크기 변경 시 텍스처 재생성
- 데모 씬 (큐브 + 방향광)

## Dependencies

`voltex_editor/Cargo.toml`에 추가 필요:
```toml
voltex_math.workspace = true
voltex_renderer.workspace = true
```
- `voltex_math`: OrbitCamera에서 Vec3, Mat4 사용
- `voltex_renderer`: Mesh, MeshVertex, 파이프라인 패턴 재사용

## ViewportTexture

오프스크린 렌더 타겟을 관리한다.

```rust
pub struct ViewportTexture {
    pub color_texture: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}
```

- `new(device, width, height)` — **Rgba8Unorm** (linear) color + Depth32Float depth 텍스처 생성
- 주의: sRGB 변환은 surface에 최종 출력 시 한 번만 적용 (이중 감마 보정 방지)
- color: usage = RENDER_ATTACHMENT | TEXTURE_BINDING (렌더 타겟 + 후속 샘플링)
- depth: usage = RENDER_ATTACHMENT
- `ensure_size(&mut self, device, w, h)` — 크기 다르면 재생성, 같으면 무시

## ViewportRenderer

오프스크린 텍스처를 UI 위에 텍스처드 쿼드로 그린다.

```rust
pub struct ViewportRenderer {
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
}
```

- `new(device, surface_format)` — 파이프라인, 샘플러, 바인드 그룹 레이아웃 생성
- `render(encoder, target_view, viewport_texture, screen_w, screen_h, rect)` — 패널 영역에 텍스처 매핑
- 바인드 그룹은 `render()` 호출 시 매 프레임 생성 (텍스처 리사이즈로 TextureView가 변경될 수 있으므로)

### 셰이더 (viewport_shader.wgsl)

버텍스 셰이더:
- uniform으로 rect 좌표 (x, y, w, h)와 screen 크기 (screen_w, screen_h) 전달
- 4개 버텍스를 rect 영역에 맞게 NDC로 변환
- UV = (0,0) ~ (1,1)

프래그먼트 셰이더:
- 텍스처 샘플링하여 출력

바인드 그룹:
- group(0) binding(0): uniform (rect + screen)
- group(0) binding(1): texture_2d
- group(0) binding(2): sampler

### 렌더 패스 설정
- target: surface texture view
- LoadOp::Load (기존 클리어된 배경 위에 오버레이)
- scissor rect: 패널 영역으로 클리핑
- 버텍스 버퍼 없음 — vertex_index로 4개 정점 생성 (triangle strip 또는 2 triangles)

## OrbitCamera

에디터 뷰포트 전용 카메라.

```rust
pub struct OrbitCamera {
    pub target: Vec3,     // 회전 중심점
    pub distance: f32,    // 중심에서 카메라까지 거리
    pub yaw: f32,         // 수평 회전 (라디안)
    pub pitch: f32,       // 수직 회전 (라디안, -PI/2+0.01 ~ PI/2-0.01 클램프)
    pub fov_y: f32,       // 시야각 (기본 PI/4)
    pub near: f32,        // 0.1
    pub far: f32,         // 100.0
}
```

### 카메라 위치 계산
```
position = target + Vec3(
    distance * cos(pitch) * sin(yaw),
    distance * sin(pitch),
    distance * cos(pitch) * cos(yaw),
)
```

### 행렬
- `view_matrix() -> Mat4` — look_at(position, target, up)
- `projection_matrix(aspect: f32) -> Mat4` — perspective(fov_y, aspect, near, far)
- `view_projection(aspect: f32) -> Mat4` — projection * view

### 입력 처리
- `orbit(&mut self, dx: f32, dy: f32)` — 좌클릭 드래그: yaw += dx * sensitivity, pitch += dy * sensitivity (클램프)
- `zoom(&mut self, delta: f32)` — 스크롤: distance *= (1.0 - delta * 0.1), min 0.5 ~ max 50.0
- `pan(&mut self, dx: f32, dy: f32)` — 미들 클릭 드래그: target을 카메라의 right/up 방향으로 이동. right 벡터가 영벡터에 가까울 때 (pitch ≈ ±90°) Vec3::X로 폴백.

입력은 뷰포트 패널 위에 마우스가 있을 때만 처리 (Rect::contains 활용).

## 3D 씬 렌더링

기존 mesh_shader.wgsl (Blinn-Phong)을 재사용한다.

### 데모 씬 구성
- 바닥 평면 (10x10)
- 큐브 3개 (위치 다르게)
- 방향광 1개

### 렌더 패스
- target: ViewportTexture.color_view (Rgba8Unorm)
- depth: ViewportTexture.depth_view
- LoadOp::Clear (매 프레임 클리어)
- 3D 씬 파이프라인은 **오프스크린 포맷(Rgba8Unorm)**으로 생성 (surface_format과 다름)
- PipelineLayoutDescriptor에 `immediate_size: 0` 필수 (wgpu 28.0)

### 바인드 그룹 레이아웃
기존 mesh_shader.wgsl 구조를 따른다:
- **group(0) binding(0)**: CameraUniform (view_proj, model, camera_pos) — 모델 변환 포함
- **group(0) binding(1)**: LightUniform (direction, color, ambient)
- **group(1)**: 텍스처 (사용 안 하면 더미 바인드)
- max_bind_groups=4 내에서 충분 (최대 2개 사용)

### WGSL vec3 패딩
Rust 측 uniform 구조체에서 vec3 필드 뒤에 `_padding: f32` 추가 (16바이트 정렬):
```rust
#[repr(C)]
struct LightUniform {
    direction: [f32; 3],
    _pad0: f32,
    color: [f32; 3],
    _pad1: f32,
    ambient_strength: f32,
    _pad2: [f32; 3],
}
```

- 기존 Mesh, MeshVertex, 파이프라인 생성 패턴 그대로 사용

## UI 통합

```rust
// editor_demo 프레임 루프
let areas = dock.layout(screen_rect);
dock.update(mx, my, mouse_down);
dock.draw_chrome(&mut ui);

for (panel_id, rect) in &areas {
    match panel_id {
        1 => {
            // 뷰포트 패널
            viewport_tex.ensure_size(&gpu.device, rect.w as u32, rect.h as u32);

            // 오빗 카메라 입력 (패널 위에 마우스 있을 때만)
            if rect.contains(mx, my) {
                if left_dragging { orbit_cam.orbit(dx, dy); }
                if middle_dragging { orbit_cam.pan(dx, dy); }
                if scroll != 0.0 { orbit_cam.zoom(scroll); }
            }

            // 3D 씬 렌더 → 오프스크린 텍스처
            render_scene(&mut encoder, &viewport_tex, &orbit_cam, &scene_data);

            // 텍스처를 UI에 표시
            viewport_renderer.render(&mut encoder, &surface_view, &viewport_tex, screen_w, screen_h, rect);
        }
        _ => { /* 다른 패널 위젯 */ }
    }
}

// UI 오버레이 렌더
ui_renderer.render(...);
```

순서: 3D 씬 렌더 → 뷰포트 텍스처 표시 → UI 오버레이 (탭 바 등은 위에 그려짐)

## File Structure

- `crates/voltex_editor/src/viewport_texture.rs` — ViewportTexture
- `crates/voltex_editor/src/viewport_renderer.rs` — ViewportRenderer
- `crates/voltex_editor/src/viewport_shader.wgsl` — 텍스처드 쿼드 셰이더
- `crates/voltex_editor/src/orbit_camera.rs` — OrbitCamera
- `crates/voltex_editor/src/lib.rs` — 모듈 추가
- `examples/editor_demo/src/main.rs` — 통합

## Testing

### OrbitCamera (순수 수학, GPU 불필요)
- position 계산: 다양한 yaw/pitch에서 카메라 위치 검증
- view_matrix: 알려진 위치에서 행렬 검증
- orbit: dx/dy 입력 후 yaw/pitch 변경 확인
- zoom: distance 변경 + min/max 클램프
- pan: target 이동 방향 검증
- pitch 클램프: +-90도 초과 방지

### ViewportTexture (GPU 필요 — 단위 테스트 불가, 빌드 검증만)
- ensure_size 로직은 크기 비교만이므로 간단한 상태 테스트 가능

### ViewportRenderer (GPU 의존 — 빌드 검증)
- NDC 변환 함수만 별도 추출하여 테스트 가능
