# Transparent Objects Design

## Overview

디퍼드 라이팅 후 HDR 타겟에 알파 블렌딩으로 투명 오브젝트를 포워드 렌더링한다.

## Scope

- ForwardPass 구조체 + 파이프라인 (알파 블렌딩, HDR 타겟)
- 포워드 셰이더 (Blinn-Phong + alpha)
- 카메라 거리 기준 back-to-front 정렬
- deferred_demo에 반투명 큐브 추가

## Render Pass Order

```
G-Buffer → SSGI → RT Shadow → Lighting → [Forward Transparency] → Bloom → Tonemap
```

## ForwardPass

```rust
pub struct ForwardPass {
    pipeline: wgpu::RenderPipeline,
}
```

### 파이프라인 설정
- target: Rgba16Float (HDR_FORMAT) — LoadOp::Load
- depth: Depth32Float (G-Buffer depth) — LoadOp::Load, depth_write: false, compare: LessEqual
- blend: SrcAlpha / OneMinusSrcAlpha (standard alpha blending)
- vertex: MeshVertex layout (기존 동일)
- bind groups: group(0) camera+light, group(1) texture — 기존 레이아웃 재사용
- immediate_size: 0 (wgpu 28.0)

### 생성
```rust
impl ForwardPass {
    pub fn new(
        device: &wgpu::Device,
        camera_light_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
    ) -> Self;
}
```

### 렌더
```rust
pub fn render(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    hdr_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
    camera_light_bg: &wgpu::BindGroup,
    texture_bg: &wgpu::BindGroup,
    meshes: &[(Mesh, f32)],  // (mesh, alpha) — back-to-front 정렬 완료 상태
);
```

## Forward Shader (forward_shader.wgsl)

기존 mesh_shader.wgsl과 거의 동일하나:
- CameraUniform에 alpha uniform 추가, 또는 별도 uniform
- fragment output에서 `color.a = alpha`

가장 간단한 방법: CameraUniform의 `_padding` 필드를 `alpha`로 재활용:
```wgsl
struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec3<f32>,
    alpha: f32,  // 기존 _padding → alpha로 사용
};
```

Fragment shader:
```wgsl
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Blinn-Phong lighting calculation (동일)
    let lit_color = ambient + diffuse + specular;
    return vec4<f32>(lit_color, camera.alpha);
}
```

## 투명 메시 정렬

```rust
pub fn sort_transparent_back_to_front(
    meshes: &mut Vec<(usize, Vec3)>,  // (mesh_index, center_position)
    camera_pos: Vec3,
) {
    meshes.sort_by(|a, b| {
        let da = (a.1 - camera_pos).length();
        let db = (b.1 - camera_pos).length();
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });
}
```

이 함수는 순수 수학이므로 단위 테스트 가능.

## deferred_demo 통합

1. ForwardPass 생성 (기존 camera_light_layout, texture_layout 재사용)
2. 반투명 큐브 2개 생성 (alpha=0.4, alpha=0.6)
3. 매 프레임: 투명 메시를 카메라 거리로 정렬
4. Lighting pass 후, Bloom 전에 forward_pass.render() 호출

## File Structure

- `crates/voltex_renderer/src/forward_pass.rs` — ForwardPass
- `crates/voltex_renderer/src/forward_shader.wgsl` — 포워드 셰이더
- `crates/voltex_renderer/src/lib.rs` — 모듈 추가
- `examples/deferred_demo/src/main.rs` — 투명 메시 + 포워드 패스 통합

## Testing

### sort_transparent_back_to_front (순수 수학)
- 3개 위치를 카메라 기준 정렬 → 먼 것부터 순서 검증
- 동일 거리 → 크래시 없음

### ForwardPass (GPU 의존 → 빌드 검증)
- 컴파일 성공 확인
