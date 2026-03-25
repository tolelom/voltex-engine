# Phase 7-1: Deferred Rendering — Design Spec

## Overview

`voltex_renderer`에 디퍼드 렌더링 파이프라인을 추가한다. 기존 포워드 PBR은 유지하고, G-Buffer + Lighting Pass 구조의 디퍼드 파이프라인을 새 모듈로 구현한다.

## Scope

- G-Buffer (4 MRT: Position, Normal, Albedo, Material + Depth)
- G-Buffer Pass 셰이더 (기하 데이터 기록)
- Lighting Pass 셰이더 (풀스크린 쿼드, Cook-Torrance BRDF, 멀티 라이트, 섀도우, IBL)
- 풀스크린 삼각형
- deferred_demo 예제

## Out of Scope

- 포워드 파이프라인 제거/변경
- 투명 오브젝트 (디퍼드에서 처리 어려움, 별도 포워드 패스 필요)
- G-Buffer 압축/최적화 (octahedral normal, depth-position 복원 등)
- Light volumes (sphere/cone 렌더링으로 라이트 컬링)
- Stencil 기반 최적화

## Render Pass Architecture

### Pass 1: G-Buffer Pass

MRT(Multiple Render Targets)로 기하 데이터 기록.

| RT | Format | Content |
|----|--------|---------|
| RT0 | Rgba32Float | World Position (xyz) |
| RT1 | Rgba16Float | World Normal (xyz, normalized) |
| RT2 | Rgba8UnormSrgb | Albedo (rgb) |
| RT3 | Rgba8Unorm | R=metallic, G=roughness, B=ao |
| Depth | Depth32Float | Depth (기존 공유) |

**Bind Groups:**
- Group 0 (dynamic): CameraUniform (view_proj, model)
- Group 1: PBR Textures (albedo + normal map)
- Group 2 (dynamic): MaterialUniform

**Shader:** 버텍스 → 월드 변환, 프래그먼트 → G-Buffer 기록. TBN 노멀맵 적용.

### Pass 2: Lighting Pass

풀스크린 삼각형 렌더, G-Buffer를 텍스처로 읽어 라이팅 계산.

**Bind Groups:**
- Group 0: G-Buffer textures (4개) + sampler
- Group 1: LightsUniform + CameraPosition
- Group 2: Shadow map + shadow sampler + ShadowUniform + BRDF LUT + BRDF sampler

**Shader:** 기존 pbr_shader.wgsl의 Cook-Torrance BRDF 로직을 재사용.
- G-Buffer에서 position, normal, albedo, metallic/roughness/ao 읽기
- 멀티 라이트 루프 (directional, point, spot)
- PCF 섀도우
- IBL ambient (procedural sky + BRDF LUT)
- Reinhard 톤매핑 + 감마 보정

## Module Structure

### 새 파일
- `crates/voltex_renderer/src/gbuffer.rs` — GBuffer 타입 (텍스처 생성/리사이즈)
- `crates/voltex_renderer/src/fullscreen_quad.rs` — 풀스크린 삼각형 정점
- `crates/voltex_renderer/src/deferred_pipeline.rs` — 파이프라인 생성 (gbuffer pass + lighting pass)
- `crates/voltex_renderer/src/deferred_gbuffer.wgsl` — G-Buffer pass 셰이더
- `crates/voltex_renderer/src/deferred_lighting.wgsl` — Lighting pass 셰이더

### 수정 파일
- `crates/voltex_renderer/src/lib.rs` — 새 모듈 등록

## Types

### GBuffer

```rust
pub struct GBuffer {
    pub position_view: TextureView,  // Rgba32Float
    pub normal_view: TextureView,    // Rgba16Float
    pub albedo_view: TextureView,    // Rgba8UnormSrgb
    pub material_view: TextureView,  // Rgba8Unorm
    pub depth_view: TextureView,     // Depth32Float
    pub width: u32,
    pub height: u32,
}
```

- `new(device, width, height) -> Self`
- `resize(device, width, height)` — 윈도우 리사이즈 시 재생성

### DeferredPipeline

```rust
pub struct DeferredPipeline {
    pub gbuffer_pipeline: RenderPipeline,
    pub lighting_pipeline: RenderPipeline,
    pub gbuffer_bind_group_layouts: [BindGroupLayout; 3],  // camera, texture, material
    pub lighting_bind_group_layouts: [BindGroupLayout; 3],  // gbuffer, lights, shadow+ibl
}
```

- `new(device, surface_format) -> Self`

### Fullscreen Triangle

```rust
pub struct FullscreenTriangle {
    pub vertex_buffer: Buffer,
}
```

3 정점: (-1,-1), (3,-1), (-1,3) — 클리핑으로 화면 커버. UV는 셰이더에서 position으로 계산.

## Bind Group Details

### G-Buffer Pass

**Group 0 — Camera (dynamic offset):**
- binding 0: CameraUniform (view_proj, model, camera_pos)

**Group 1 — Textures:**
- binding 0: albedo texture
- binding 1: albedo sampler
- binding 2: normal map texture
- binding 3: normal map sampler

**Group 2 — Material (dynamic offset):**
- binding 0: MaterialUniform (base_color, metallic, roughness, ao)

### Lighting Pass

**Group 0 — G-Buffer:**
- binding 0: position texture
- binding 1: normal texture
- binding 2: albedo texture
- binding 3: material texture
- binding 4: sampler (shared, nearest)

**Group 1 — Lights:**
- binding 0: LightsUniform
- binding 1: CameraPositionUniform (vec3 + padding)

**Group 2 — Shadow + IBL:**
- binding 0: shadow depth texture
- binding 1: shadow comparison sampler
- binding 2: ShadowUniform
- binding 3: BRDF LUT texture
- binding 4: BRDF LUT sampler

## Shader Summary

### deferred_gbuffer.wgsl

Vertex: position → world (model * pos), normal → world (model * normal), TBN 계산, UV 전달.

Fragment outputs (4 targets):
```wgsl
struct GBufferOutput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) albedo: vec4<f32>,
    @location(3) material: vec4<f32>,
}
```
- position.xyz = world position
- normal.xyz = TBN-mapped world normal
- albedo.rgb = texture sample * base_color
- material = vec4(metallic, roughness, ao, 1.0)

### deferred_lighting.wgsl

Vertex: 풀스크린 삼각형, UV 계산.

Fragment:
1. G-Buffer 샘플링
2. Cook-Torrance BRDF (기존 pbr_shader.wgsl 로직)
3. 멀티 라이트 루프
4. PCF 섀도우
5. IBL ambient
6. Reinhard 톤매핑 + 감마

## Test Plan

### gbuffer.rs
- GBuffer 생성: 텍스처 크기 확인
- 리사이즈: 새 크기로 재생성

### fullscreen_quad.rs
- 정점 데이터: 3개 정점, 올바른 좌표

### 통합 (수동)
- deferred_demo 예제: 다수 포인트 라이트 + 디퍼드 렌더링
- G-Buffer 시각화 (디버그용: position/normal/albedo 각각 출력)

## Constraints

- max_bind_groups=4: G-Buffer pass 3개, Lighting pass 3개 사용 → 제약 내
- MRT: wgpu는 최대 8개 color attachment 지원. 4개 사용.
- Rgba32Float: Position에 32-bit float 사용 (정밀도 우선, 최적화는 추후)
