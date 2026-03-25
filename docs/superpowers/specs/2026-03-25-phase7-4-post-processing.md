# Phase 7-4: HDR + Bloom + ACES Tonemap — Design Spec

## Overview

디퍼드 파이프라인을 HDR로 전환하고, Bloom과 ACES 톤매핑 포스트 프로세싱을 추가한다.

## Scope

- HDR 렌더 타겟 (Rgba16Float)
- Lighting Pass HDR 출력 (Reinhard/감마 제거)
- Bloom (bright extract + downsample chain + upsample blend)
- ACES 톤매핑 + bloom 합성 + 감마 보정
- deferred_demo에 HDR+Bloom+Tonemap 통합

## Out of Scope

- TAA (Temporal Anti-Aliasing)
- SSR (Screen-Space Reflections)
- Motion Blur, Depth of Field
- Adaptive exposure / auto-exposure
- Filmic 등 다른 톤매핑 커브

## Render Pass Flow (최종)

```
Pass 1: G-Buffer (변경 없음)
Pass 2: SSGI (변경 없음)
Pass 3: RT Shadow (변경 없음)
Pass 4: Lighting → HDR texture (Rgba16Float) — 톤매핑/감마 제거
Pass 5: Bloom (NEW) — bright extract → downsample chain → upsample chain
Pass 6: Tonemap (NEW) — HDR + Bloom → ACES tonemap + gamma → surface
```

## Module Structure

### 새 파일
- `crates/voltex_renderer/src/hdr.rs` — HdrTarget (Create)
- `crates/voltex_renderer/src/bloom.rs` — BloomResources, mip chain 생성 (Create)
- `crates/voltex_renderer/src/bloom_shader.wgsl` — downsample + upsample 셰이더 (Create)
- `crates/voltex_renderer/src/tonemap.rs` — TonemapUniform (Create)
- `crates/voltex_renderer/src/tonemap_shader.wgsl` — ACES + bloom merge + gamma (Create)

### 수정 파일
- `crates/voltex_renderer/src/deferred_pipeline.rs` — bloom/tonemap 파이프라인 추가 (Modify)
- `crates/voltex_renderer/src/deferred_lighting.wgsl` — Reinhard+감마 제거 (Modify)
- `crates/voltex_renderer/src/lib.rs` — 새 모듈 등록 (Modify)
- `examples/deferred_demo/src/main.rs` — HDR+Bloom+Tonemap 통합 (Modify)

## Types

### HdrTarget

```rust
pub const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

pub struct HdrTarget {
    pub view: TextureView,
    pub width: u32,
    pub height: u32,
}
```
- `new(device, width, height)` — Rgba16Float, RENDER_ATTACHMENT | TEXTURE_BINDING
- `resize(device, width, height)`

### BloomResources

```rust
pub struct BloomResources {
    pub mip_views: Vec<TextureView>,  // 5 levels
    pub threshold: f32,
    pub intensity: f32,
    pub bloom_uniform: Buffer,
}
```

**Mip chain**: 5단계, 각 레벨은 이전의 1/2 크기.
- Level 0: width/2 x height/2
- Level 1: width/4 x height/4
- Level 2: width/8 x height/8
- Level 3: width/16 x height/16
- Level 4: width/32 x height/32

각 mip은 Rgba16Float, RENDER_ATTACHMENT | TEXTURE_BINDING.

### BloomUniform

```rust
#[repr(C)]
pub struct BloomUniform {
    pub threshold: f32,
    pub soft_threshold: f32, // threshold - knee
    pub _padding: [f32; 2],
}
```

### TonemapUniform

```rust
#[repr(C)]
pub struct TonemapUniform {
    pub bloom_intensity: f32,
    pub exposure: f32,
    pub _padding: [f32; 2],
}
```

## Bloom Algorithm

### Pass 5a: Bright Extract + First Downsample

입력: HDR 텍스처 (full res)
출력: Bloom mip 0 (half res)

```wgsl
let color = textureSample(hdr, sampler, uv);
let luminance = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));
let contribution = max(luminance - threshold, 0.0);
let bloom_color = color.rgb * (contribution / (luminance + 0.0001));
// 2x2 box filter (downsample)
output = bloom_color;
```

### Pass 5b: Downsample Chain (4회)

mip N → mip N+1: 텍스처 샘플링으로 자연스럽게 2x 다운샘플. 13-tap box filter로 품질 향상.

```wgsl
// 13-tap downsample (Karis average 스타일)
let a = textureSample(src, s, uv + vec2(-2, -2) * texel);
let b = textureSample(src, s, uv + vec2( 0, -2) * texel);
// ... (center + 4 corners + 4 edges + 4 diagonal)
output = weighted_average;
```

### Pass 5c: Upsample Chain (4회)

mip N+1 → mip N: bilinear 업샘플 + additive blend with current level.

```wgsl
let upsampled = textureSample(lower_mip, s, uv);
let current = textureSample(current_mip, s, uv);
output = current + upsampled;
```

### 최종 bloom = mip 0 (half res)

## Tonemap Pass (Pass 6)

입력: HDR 텍스처 + Bloom 텍스처 (mip 0)
출력: surface texture (sRGB)

```wgsl
let hdr_color = textureSample(t_hdr, s, uv).rgb;
let bloom_color = textureSample(t_bloom, s, uv).rgb;
let combined = hdr_color + bloom_color * bloom_intensity;
let exposed = combined * exposure;
let tonemapped = aces_tonemap(exposed);
let gamma = pow(tonemapped, vec3(1.0 / 2.2));
output = vec4(gamma, 1.0);
```

### ACES Filmic Tonemap

```wgsl
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3(0.0), vec3(1.0));
}
```

## Lighting Pass 수정

현재 `deferred_lighting.wgsl` 끝부분:
```wgsl
// Reinhard tone mapping
color = color / (color + vec3<f32>(1.0));
// Gamma correction
color = pow(color, vec3<f32>(1.0 / 2.2));
```

변경: 이 두 줄을 **제거**. raw HDR 값을 그대로 출력.

Lighting pipeline의 color target format을 surface format → HDR_FORMAT(Rgba16Float)으로 변경.

## Bind Group Details

### Bloom Downsample/Upsample (각 패스)
- Group 0: 입력 텍스처 + sampler + BloomUniform (downsample 시)
단일 파이프라인을 재사용, 입력/출력만 변경.

### Tonemap
- Group 0: HDR 텍스처 + bloom 텍스처 + sampler + TonemapUniform

## Simplified Bloom (구현 단순화)

복잡한 13-tap 대신, 첫 구현은:
1. Bright extract → mip 0 (half res, single fullscreen pass)
2. Downsample: 순차 4회 (bilinear sampler가 자동 2x2 평균)
3. Upsample: 순차 4회 (additive blend)
4. 총 9 fullscreen passes (1 extract + 4 down + 4 up)

단일 셰이더 + 다른 바인드 그룹으로 반복 실행.

## Test Plan

### bloom.rs
- mip_sizes: 입력 크기에서 5단계 크기 계산
- BloomResources 생성

### tonemap.rs
- ACES tonemap: 0→0, 1→~0.59, large→~1.0

### 통합 (수동)
- deferred_demo: bloom ON/OFF 비교 (밝은 라이트 주변 glow)
- exposure 조절 (어두움/밝음)
