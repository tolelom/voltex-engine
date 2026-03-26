# SSGI Quality Improvement Design (Bilateral Blur + Temporal Accumulation)

## Overview

SSGI 출력의 노이즈를 줄이기 위해 bilateral blur와 temporal accumulation 필터를 추가한다.

## Scope

- Bilateral blur 셰이더: depth/normal 기반 edge-aware blur
- Temporal accumulation: 이전 프레임 SSGI와 현재 프레임을 블렌딩
- 각각 독립적 풀스크린 패스

## Bilateral Blur

Depth와 normal 차이를 가중치로 사용하여 edge를 보존하면서 노이즈를 줄이는 블러.

### 셰이더 (bilateral_blur.wgsl)

```wgsl
@group(0) @binding(0) var input_tex: texture_2d<f32>;     // SSGI 출력
@group(0) @binding(1) var depth_tex: texture_2d<f32>;     // G-Buffer depth
@group(0) @binding(2) var normal_tex: texture_2d<f32>;    // G-Buffer normal
@group(0) @binding(3) var output_tex: texture_storage_2d<rgba16float, write>;

// 5x5 bilateral kernel
// weight = gaussian(spatial_dist) * exp(-|depth_diff|/sigma_d) * max(dot(n1,n2),0)^sigma_n
```

컴퓨트 셰이더로 구현. 워크그룹 16x16.

### BilateralBlur 구조체

```rust
pub struct BilateralBlur {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}
```

- `new(device)` — 파이프라인 생성
- `dispatch(encoder, ssgi_view, depth_view, normal_view, output_view, width, height)` — 실행

### 파라미터
- kernel_size: 5 (하드코딩)
- sigma_spatial: 2.0
- sigma_depth: 0.1
- sigma_normal: 16.0

## Temporal Accumulation

이전 프레임의 결과와 현재 프레임을 블렌딩하여 시간에 따라 노이즈를 줄임.

### 셰이더 (temporal_accum.wgsl)

```wgsl
@group(0) @binding(0) var current_tex: texture_2d<f32>;   // 현재 프레임 (blurred SSGI)
@group(0) @binding(1) var history_tex: texture_2d<f32>;   // 이전 프레임 결과
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba16float, write>;

// blend_factor = 0.1 (10% 새 프레임, 90% 히스토리)
// output = mix(history, current, blend_factor)
```

### TemporalAccumulation 구조체

```rust
pub struct TemporalAccumulation {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    history_texture: wgpu::Texture,  // 이전 프레임 저장
    history_view: wgpu::TextureView,
    pub blend_factor: f32,           // 0.1
}
```

- `new(device, width, height)` — 히스토리 텍스처 + 파이프라인
- `dispatch(encoder, current_view, output_view, width, height)` — 블렌딩 실행
- `swap_history(new_view)` — 출력을 히스토리로 복사 (또는 ping-pong)

### Ping-Pong 패턴
텍스처 2개를 교대로 사용:
- Frame N: read history_A, write history_B → output = history_B
- Frame N+1: read history_B, write history_A → output = history_A

## 적용 순서

```
SSGI Pass → [Bilateral Blur] → [Temporal Accumulation] → Lighting Pass (기존 SSGI 텍스처 대체)
```

## File Structure

- `crates/voltex_renderer/src/bilateral_blur.rs` — BilateralBlur
- `crates/voltex_renderer/src/bilateral_blur.wgsl` — 컴퓨트 셰이더
- `crates/voltex_renderer/src/temporal_accum.rs` — TemporalAccumulation
- `crates/voltex_renderer/src/temporal_accum.wgsl` — 컴퓨트 셰이더
- `crates/voltex_renderer/src/lib.rs` — 모듈 추가

## Testing

### Bilateral Blur (순수 수학)
- gaussian weight 계산 검증
- bilateral weight: depth 차이 클수록 weight 작아지는지

### Temporal Accumulation (순수 수학)
- blend: factor=0.0 → history 그대로, factor=1.0 → current 그대로
- factor=0.1 → 90% history + 10% current
