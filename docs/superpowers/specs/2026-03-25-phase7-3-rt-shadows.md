# Phase 7-3: RT Shadows — Design Spec

## Overview

wgpu의 EXPERIMENTAL_RAY_QUERY를 활용하여 하드웨어 레이트레이싱 기반 그림자를 구현한다. 기존 PCF shadow map을 대체하는 정확한 그림자.

## Hardware Requirements

- GPU: RTX 20xx+ / RDNA2+ (ray query 지원)
- wgpu Features: EXPERIMENTAL_RAY_QUERY
- 검증 완료: RTX 4050 Laptop GPU, Vulkan backend

## Scope

- BLAS/TLAS acceleration structure 생성 관리
- RT Shadow 컴퓨트 셰이더 (ray query로 directional light shadow)
- RT Shadow 출력 텍스처 (R8Unorm)
- Lighting Pass에 RT shadow 통합
- deferred_demo에 RT shadow 적용

## Out of Scope

- RT Reflections
- RT AO
- Point/Spot light RT shadows
- Soft RT shadows (multi-ray)
- BLAS 재빌드 (정적 지오메트리만)

## Render Pass Flow (디퍼드 확장)

```
Pass 1: G-Buffer (변경 없음)
Pass 2: SSGI (변경 없음)
Pass 3: RT Shadow (NEW) — 컴퓨트 셰이더, ray query로 shadow 텍스처 출력
Pass 4: Lighting (수정) — RT shadow 텍스처 사용
```

## Module Structure

### 새 파일
- `crates/voltex_renderer/src/rt_accel.rs` — RtAccel (BLAS/TLAS 관리)
- `crates/voltex_renderer/src/rt_shadow.rs` — RtShadowResources + 컴퓨트 파이프라인
- `crates/voltex_renderer/src/rt_shadow_shader.wgsl` — RT shadow 컴퓨트 셰이더

### 수정 파일
- `crates/voltex_renderer/src/deferred_pipeline.rs` — lighting shadow bind group에 RT shadow 텍스처 추가
- `crates/voltex_renderer/src/deferred_lighting.wgsl` — RT shadow 사용
- `crates/voltex_renderer/src/lib.rs` — 새 모듈 등록
- `examples/deferred_demo/src/main.rs` — RT shadow 통합

## Types

### RtAccel

```rust
pub struct RtAccel {
    pub blas_list: Vec<wgpu::Blas>,
    pub tlas_package: wgpu::TlasPackage,
}
```

**Methods:**
- `new(device, meshes: &[(vertex_buffer, index_buffer, vertex_count, index_count)], transforms: &[[f32; 12]])` — BLAS 빌드, TLAS 구성
- BLAS: 메시별 삼각형 지오메트리 (BlasTriangleGeometry)
- TLAS: 인스턴스 배열 (TlasInstance with transform, blas index)

**BLAS 생성:**
1. BlasTriangleGeometrySizeDescriptor (vertex_count, index_count, vertex_format: Float32x3)
2. device.create_blas(size, flags: PREFER_FAST_TRACE)
3. encoder.build_acceleration_structures with BlasBuildEntry (vertex_buffer, index_buffer, geometry)

**TLAS 생성:**
1. device.create_tlas(max_instances: transform_count)
2. TlasPackage에 TlasInstance 채움 (transform [3x4 row-major], blas_index, mask: 0xFF)
3. encoder.build_acceleration_structures with tlas_package

### RtShadowResources

```rust
pub struct RtShadowResources {
    pub shadow_view: TextureView,    // R8Unorm, STORAGE_BINDING
    pub shadow_texture: Texture,
    pub uniform_buffer: Buffer,       // RtShadowUniform
    pub width: u32,
    pub height: u32,
}
```

### RtShadowUniform

```rust
#[repr(C)]
pub struct RtShadowUniform {
    pub light_direction: [f32; 3],
    pub _pad0: f32,
    pub width: u32,
    pub height: u32,
    pub _pad1: [u32; 2],
}
```

## RT Shadow Compute Shader

### 바인드 그룹

**Group 0: G-Buffer**
- binding 0: position texture (Float, non-filterable)
- binding 1: normal texture (Float, filterable)

**Group 1: RT Data**
- binding 0: TLAS (acceleration_structure)
- binding 1: RT shadow output (storage texture, r32float, write)
- binding 2: RtShadowUniform

### 셰이더 로직

```wgsl
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= uniforms.width || id.y >= uniforms.height { return; }

    let world_pos = textureLoad(t_position, id.xy, 0).xyz;

    // Skip background
    if dot(world_pos, world_pos) < 0.001 {
        textureStore(t_shadow_out, id.xy, vec4(1.0));
        return;
    }

    let normal = normalize(textureLoad(t_normal, id.xy, 0).xyz * 2.0 - 1.0);
    let ray_origin = world_pos + normal * 0.01;  // bias off surface
    let ray_dir = normalize(-uniforms.light_direction);

    var rq: ray_query;
    rayQueryInitialize(&rq, tlas, RAY_FLAG_TERMINATE_ON_FIRST_HIT,
        0xFFu, ray_origin, 0.001, ray_dir, 1000.0);
    rayQueryProceed(&rq);

    var shadow = 1.0;  // lit by default
    if rayQueryGetCommittedIntersectionType(&rq) != RAY_QUERY_COMMITTED_INTERSECTION_NONE {
        shadow = 0.0;  // occluded
    }

    textureStore(t_shadow_out, id.xy, vec4(shadow, 0.0, 0.0, 0.0));
}
```

## Lighting Pass 수정

RT shadow 텍스처를 기존 shadow_factor 대신 사용:

```wgsl
// 기존: let shadow_factor = calculate_shadow(world_pos);
// 변경: RT shadow map에서 직접 읽기
let rt_shadow = textureSample(t_rt_shadow, s_rt_shadow, uv).r;
let shadow_factor = rt_shadow;
```

기존 PCF shadow map 관련 바인딩은 유지하되 사용하지 않음 (호환성).
RT shadow 텍스처를 Group 2의 추가 바인딩(7, 8)으로 추가.

## Device Creation 변경

RT feature를 요청해야 함:
```rust
let (device, queue) = adapter.request_device(&DeviceDescriptor {
    required_features: Features::EXPERIMENTAL_RAY_QUERY,
    ..
}).await;
```

기존 GpuContext::new()는 features를 요청하지 않으므로, deferred_demo에서 직접 device를 생성하거나 GpuContext에 optional features 파라미터를 추가.

## Bind Group Details

### RT Shadow Compute

**Group 0:**
- binding 0: position texture (texture_2d<f32>)
- binding 1: normal texture (texture_2d<f32>)

**Group 1:**
- binding 0: acceleration_structure (TLAS)
- binding 1: storage texture (r32float, write)
- binding 2: uniform buffer (RtShadowUniform)

### Lighting Pass Group 2 (확장)

기존 7 bindings (0-6: shadow+IBL+SSGI) + 추가:
- binding 7: RT shadow texture (Float, filterable)
- binding 8: RT shadow sampler (Filtering)

## Test Plan

- rt_accel.rs: 빌드 확인만 (GPU 의존)
- rt_shadow.rs: RtShadowUniform 크기, 리소스 생성
- 통합: deferred_demo에서 RT shadow ON, 기존 PCF OFF → 날카로운 그림자 확인
