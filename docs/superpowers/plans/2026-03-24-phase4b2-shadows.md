# Phase 4b-2: Directional Light Shadow Map Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Directional Light에서 단일 Shadow Map을 생성하고, PBR 셰이더에서 그림자를 샘플링하여 PCF 소프트 섀도우를 렌더링한다.

**Architecture:** 2-pass 렌더링: (1) Shadow pass — 라이트 시점의 orthographic 투영으로 씬을 depth-only 텍스처에 렌더링, (2) Color pass — 기존 PBR 렌더링 + shadow map 샘플링. ShadowMap 구조체가 depth 텍스처와 라이트 VP 행렬을 관리. PBR 셰이더의 새 bind group(3)으로 shadow map + shadow uniform을 전달. 3x3 PCF로 소프트 섀도우.

**Tech Stack:** Rust 1.94, wgpu 28.0, WGSL

---

## File Structure

```
crates/voltex_renderer/src/
├── shadow.rs           # ShadowMap, ShadowUniform, shadow depth texture (NEW)
├── shadow_shader.wgsl  # Depth-only vertex shader for shadow pass (NEW)
├── shadow_pipeline.rs  # Depth-only render pipeline (NEW)
├── pbr_shader.wgsl     # Shadow sampling + PCF 추가 (MODIFY)
├── pbr_pipeline.rs     # group(3) shadow bind group 추가 (MODIFY)
├── lib.rs              # re-export 업데이트 (MODIFY)
examples/
└── shadow_demo/        # 섀도우 데모 (NEW)
    ├── Cargo.toml
    └── src/
        └── main.rs
```

---

## Task 1: ShadowMap + Shadow Depth Shader + Shadow Pipeline

**Files:**
- Create: `crates/voltex_renderer/src/shadow.rs`
- Create: `crates/voltex_renderer/src/shadow_shader.wgsl`
- Create: `crates/voltex_renderer/src/shadow_pipeline.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

### shadow.rs

```rust
// crates/voltex_renderer/src/shadow.rs
use bytemuck::{Pod, Zeroable};

pub const SHADOW_MAP_SIZE: u32 = 2048;
pub const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Shadow map에 필요한 GPU 리소스
pub struct ShadowMap {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl ShadowMap {
    pub fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Map"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Comparison sampler for hardware-assisted shadow comparison
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Self { texture, view, sampler }
    }

    /// Shadow bind group layout (group 3)
    /// binding 0: shadow depth texture (comparison)
    /// binding 1: shadow comparison sampler
    /// binding 2: ShadowUniform (light VP + params)
    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Depth,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
    }

    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        shadow_uniform_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: shadow_uniform_buffer.as_entire_binding(),
                },
            ],
        })
    }
}

/// Shadow pass에 필요한 uniform (light view-projection 행렬)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ShadowUniform {
    pub light_view_proj: [[f32; 4]; 4],
    pub shadow_map_size: f32,
    pub shadow_bias: f32,
    pub _padding: [f32; 2],
}

impl ShadowUniform {
    pub fn new() -> Self {
        Self {
            light_view_proj: [
                [1.0,0.0,0.0,0.0],
                [0.0,1.0,0.0,0.0],
                [0.0,0.0,1.0,0.0],
                [0.0,0.0,0.0,1.0],
            ],
            shadow_map_size: SHADOW_MAP_SIZE as f32,
            shadow_bias: 0.005,
            _padding: [0.0; 2],
        }
    }
}

/// Shadow pass용 per-object uniform (light VP * model)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ShadowPassUniform {
    pub light_vp_model: [[f32; 4]; 4],
}
```

### shadow_shader.wgsl

Shadow pass에서 사용하는 depth-only 셰이더. Fragment output 없음 — depth만 기록.

```wgsl
// crates/voltex_renderer/src/shadow_shader.wgsl

struct ShadowPassUniform {
    light_vp_model: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> shadow_pass: ShadowPassUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(model_v: VertexInput) -> @builtin(position) vec4<f32> {
    return shadow_pass.light_vp_model * vec4<f32>(model_v.position, 1.0);
}
```

### shadow_pipeline.rs

Depth-only 렌더 파이프라인. Fragment 스테이지 없음.

```rust
// crates/voltex_renderer/src/shadow_pipeline.rs
use crate::vertex::MeshVertex;
use crate::shadow::SHADOW_FORMAT;

pub fn create_shadow_pipeline(
    device: &wgpu::Device,
    shadow_pass_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shadow Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shadow_shader.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Shadow Pipeline Layout"),
        bind_group_layouts: &[shadow_pass_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Shadow Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[MeshVertex::LAYOUT],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: None, // depth-only
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Front), // front-face culling reduces peter-panning
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: SHADOW_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,     // depth bias 로 shadow acne 방지
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Shadow pass용 bind group layout (group 0: ShadowPassUniform)
pub fn shadow_pass_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Shadow Pass BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<crate::shadow::ShadowPassUniform>() as u64,
                    ),
                },
                count: None,
            },
        ],
    })
}
```

### lib.rs 업데이트

```rust
pub mod shadow;
pub mod shadow_pipeline;
pub use shadow::{ShadowMap, ShadowUniform, ShadowPassUniform, SHADOW_MAP_SIZE, SHADOW_FORMAT};
pub use shadow_pipeline::{create_shadow_pipeline, shadow_pass_bind_group_layout};
```

- [ ] **Step 1: 위 파일들 모두 작성**
- [ ] **Step 2: 빌드 확인** — `cargo build -p voltex_renderer`
- [ ] **Step 3: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): add ShadowMap, shadow depth shader, and shadow pipeline"
```

---

## Task 2: PBR 셰이더 + 파이프라인에 Shadow 통합

**Files:**
- Modify: `crates/voltex_renderer/src/pbr_shader.wgsl`
- Modify: `crates/voltex_renderer/src/pbr_pipeline.rs`

PBR 셰이더에 group(3) shadow bind group을 추가하고, directional light의 그림자를 PCF로 샘플링.

### pbr_shader.wgsl 변경

기존 코드에 다음을 추가:

**Uniforms (group 3):**
```wgsl
struct ShadowUniform {
    light_view_proj: mat4x4<f32>,
    shadow_map_size: f32,
    shadow_bias: f32,
};

@group(3) @binding(0) var t_shadow: texture_depth_2d;
@group(3) @binding(1) var s_shadow: sampler_comparison;
@group(3) @binding(2) var<uniform> shadow: ShadowUniform;
```

**VertexOutput에 추가:**
```wgsl
@location(3) light_space_pos: vec4<f32>,
```

**Vertex shader에서 light space position 계산:**
```wgsl
out.light_space_pos = shadow.light_view_proj * world_pos;
```

**Shadow sampling 함수:**
```wgsl
fn calculate_shadow(light_space_pos: vec4<f32>) -> f32 {
    // Perspective divide
    let proj_coords = light_space_pos.xyz / light_space_pos.w;

    // NDC → shadow map UV: x [-1,1]→[0,1], y [-1,1]→[0,1] (flip y)
    let shadow_uv = vec2<f32>(
        proj_coords.x * 0.5 + 0.5,
        -proj_coords.y * 0.5 + 0.5,
    );
    let current_depth = proj_coords.z;

    // Out of shadow map bounds → no shadow
    if shadow_uv.x < 0.0 || shadow_uv.x > 1.0 || shadow_uv.y < 0.0 || shadow_uv.y > 1.0 {
        return 1.0;
    }
    if current_depth > 1.0 || current_depth < 0.0 {
        return 1.0;
    }

    // 3x3 PCF
    let texel_size = 1.0 / shadow.shadow_map_size;
    var shadow_val = 0.0;
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow_val += textureSampleCompare(
                t_shadow, s_shadow,
                shadow_uv + offset,
                current_depth - shadow.shadow_bias,
            );
        }
    }
    return shadow_val / 9.0;
}
```

**Fragment shader에서 directional light에 shadow 적용:**
```wgsl
// compute_light_contribution 안에서, directional light일 때만:
// radiance *= shadow_factor
```

실제로는 `compute_light_contribution` 함수에 shadow_factor 파라미터를 추가하거나, fragment shader에서 directional light(첫 번째 라이트)에만 shadow를 적용.

가장 간단한 접근: fs_main에서 라이트 루프 전에 shadow를 계산하고, directional light(type==0)의 contribution에만 곱함.

### pbr_pipeline.rs 변경

`create_pbr_pipeline`의 bind_group_layouts에 shadow layout 추가:
```rust
bind_group_layouts: &[camera_light_layout, texture_layout, material_layout, shadow_layout],
```

함수 시그니처에 `shadow_layout: &wgpu::BindGroupLayout` 파라미터 추가.

**주의:** 이 변경은 기존 pbr_demo, multi_light_demo 예제에 영향을 줌. 해당 예제들은 Task 3에서 shadow bind group을 추가해야 함. 또는 "no shadow" 용 dummy bind group을 사용.

- [ ] **Step 1: pbr_shader.wgsl 수정** — shadow uniforms, vertex output, sampling, PCF
- [ ] **Step 2: pbr_pipeline.rs 수정** — shadow bind group layout 추가
- [ ] **Step 3: 빌드 확인** — `cargo build -p voltex_renderer`
- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): integrate shadow map sampling with PCF into PBR shader"
```

---

## Task 3: 기존 예제 수정 + Shadow Demo

**Files:**
- Modify: `examples/pbr_demo/src/main.rs` — dummy shadow bind group 추가
- Modify: `examples/multi_light_demo/src/main.rs` — dummy shadow bind group 추가
- Create: `examples/shadow_demo/Cargo.toml`
- Create: `examples/shadow_demo/src/main.rs`
- Modify: `Cargo.toml` (워크스페이스에 shadow_demo 추가)

### pbr_demo, multi_light_demo 수정

PBR pipeline이 이제 shadow bind group(group 3)을 요구하므로, shadow가 필요 없는 예제에는 dummy shadow bind group을 제공:
- 1x1 depth texture (값 1.0 = 그림자 없음)
- 또는 ShadowMap::new()로 생성 후 모든 depth = 1.0인 상태로 둠 (cleared가 0이면 모두 그림자 → 안 됨)

더 간단한 접근: ShadowMap을 생성하되, shadow pass를 실행하지 않음. shadow map은 초기값(cleared) 상태. PBR 셰이더의 `calculate_shadow`에서 `current_depth > 1.0`이면 shadow=1.0(그림자 없음)을 반환하므로, clear된 상태면 그림자 없는 것과 동일.

실제로는 depth texture clear value가 0.0이므로 비교 시 모든 곳이 그림자가 됨. 이를 방지하려면:
- shadow uniform의 light_view_proj를 identity로 두면 모든 점의 z가 양수(~원래 위치)가 되어 depth=0과 비교 시 항상 "밝음"이 됨

또는 더 간단하게: shadow bias를 매우 크게 설정 (10.0) → 항상 밝게.

가장 깔끔한 해결: shader에서 `shadow.shadow_map_size == 0.0`이면 shadow 비활성(return 1.0). Dummy에서 shadow_map_size=0으로 설정.

### shadow_demo

장면:
- 바닥 평면: 큰 큐브 (scale 15x0.1x15), y=-0.5, roughness 0.8
- 구체 3개 + 큐브 2개: 바닥 위에 배치
- Directional Light: 방향 (-1, -2, -1) normalized, 위에서 비스듬히

렌더링 루프:
1. Shadow pass:
   - 라이트 VP 행렬 계산: `Mat4::look_at(light_pos, target, up) * Mat4::orthographic(...)`
   - 필요: orthographic 투영 함수 (Mat4에 추가하거나 inline으로)
   - Shadow pipeline으로 모든 오브젝트를 shadow map에 렌더링
   - per-object: ShadowPassUniform { light_vp * model }

2. Color pass:
   - ShadowUniform { light_view_proj, shadow_map_size, shadow_bias } write
   - PBR pipeline으로 렌더링 (shadow bind group 포함)

카메라: (5, 8, 12), pitch=-0.4

**필요: Mat4::orthographic 추가**

voltex_math의 Mat4에 orthographic 투영 함수가 필요:
```rust
pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self
```
wgpu NDC (z: [0,1]):
```
col0: [2/(r-l), 0, 0, 0]
col1: [0, 2/(t-b), 0, 0]
col2: [0, 0, 1/(near-far), 0]  // note: reversed for wgpu z[0,1]
col3: [-(r+l)/(r-l), -(t+b)/(t-b), near/(near-far), 1]
```

이것은 shadow_demo에서 inline으로 구현하거나, voltex_math의 Mat4에 추가할 수 있다. Mat4에 추가하는 것이 재사용성이 좋다.

- [ ] **Step 1: voltex_math/mat4.rs에 orthographic 추가 + 테스트**
- [ ] **Step 2: pbr_demo, multi_light_demo에 dummy shadow bind group 추가**
- [ ] **Step 3: shadow_demo 작성**
- [ ] **Step 4: 빌드 + 테스트**
- [ ] **Step 5: 실행 확인** — `cargo run -p shadow_demo`
- [ ] **Step 6: 커밋**

```bash
git add Cargo.toml crates/voltex_math/ examples/
git commit -m "feat: add shadow demo with directional light shadow mapping and PCF"
```

---

## Phase 4b-2 완료 기준 체크리스트

- [ ] `cargo build --workspace` 성공
- [ ] `cargo test --workspace` — 모든 테스트 통과
- [ ] Shadow map: 2048x2048 depth texture, comparison sampler
- [ ] Shadow pass: depth-only pipeline, front-face culling, depth bias
- [ ] PBR 셰이더: shadow map 샘플링 + 3x3 PCF
- [ ] `cargo run -p shadow_demo` — 바닥에 오브젝트 그림자 보임
- [ ] `cargo run -p pbr_demo` — 그림자 없이 동작 (dummy shadow)
- [ ] `cargo run -p multi_light_demo` — 그림자 없이 동작
- [ ] 기존 예제 (mesh_shader 사용하는 것들) 영향 없음
