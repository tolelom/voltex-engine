# Auto Exposure Design

## Overview

컴퓨트 셰이더로 HDR 타겟의 평균 log-luminance를 계산하고, 시간 적응형 노출 조절을 톤맵에 적용한다.

## Scope

- 컴퓨트 셰이더: HDR 텍스처 → log-luminance 평균 (2-pass 다운샘플)
- AutoExposure 구조체: 노출 계산 + 시간 보간
- tonemap_shader 수정: exposure uniform 적용

## AutoExposure

```rust
pub struct AutoExposure {
    reduce_pipeline: wgpu::ComputePipeline,
    reduce_bind_group_layout: wgpu::BindGroupLayout,
    luminance_buffer: wgpu::Buffer,       // 최종 평균 luminance (f32 1개)
    staging_buffer: wgpu::Buffer,         // GPU→CPU readback
    pub exposure: f32,
    pub min_exposure: f32,
    pub max_exposure: f32,
    pub adaptation_speed: f32,
    pub key_value: f32,                   // 목표 밝기 (0.18 기본)
}
```

## 알고리즘

### Pass 1: Log-Luminance 합산 (Compute)

HDR 텍스처의 모든 픽셀에 대해:
```
luminance = 0.2126*R + 0.7152*G + 0.0722*B
log_lum = log(max(luminance, 0.0001))
```

워크그룹 내 shared memory로 합산, atomicAdd로 글로벌 버퍼에 누적.

단순화: **단일 컴퓨트 디스패치**로 전체 텍스처를 16x16 워크그룹으로 처리. 각 워크그룹이 자기 영역의 합을 atomic으로 글로벌 버퍼에 더함.

### Pass 2: CPU에서 평균 계산 + 노출 결정

```rust
let avg_log_lum = total_log_lum / pixel_count;
let avg_lum = exp(avg_log_lum);
let target_exposure = key_value / avg_lum;
let clamped = target_exposure.clamp(min_exposure, max_exposure);
// 시간 적응 (부드러운 전환)
exposure = lerp(exposure, clamped, 1.0 - exp(-dt * adaptation_speed));
```

### Tonemap 수정

기존 tonemap_shader에 exposure 곱하기:
```wgsl
// 기존: let color = hdr_color;
// 변경:
let exposed = hdr_color * exposure;
// 그 다음 ACES tonemap
```

tonemap uniform에 `exposure: f32` 필드 추가.

## Compute Shader (auto_exposure.wgsl)

```wgsl
@group(0) @binding(0) var hdr_texture: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> result: array<atomic<u32>>;
// result[0] = sum of log_lum * 1000 (fixed point)
// result[1] = pixel count

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(hdr_texture);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let color = textureLoad(hdr_texture, vec2<i32>(gid.xy), 0);
    let lum = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
    let log_lum = log(max(lum, 0.0001));

    // Fixed-point: multiply by 1000 and cast to i32, then atomic add
    let fixed = i32(log_lum * 1000.0);
    atomicAdd(&result[0], u32(bitcast<u32>(fixed)));
    atomicAdd(&result[1], 1u);
}
```

CPU에서 readback 후:
```rust
let sum_fixed = i32::from_le_bytes(data[0..4]);
let count = u32::from_le_bytes(data[4..8]);
let avg_log_lum = (sum_fixed as f32 / 1000.0) / count as f32;
```

## GPU→CPU Readback

- luminance_buffer: STORAGE | COPY_SRC
- staging_buffer: MAP_READ | COPY_DST
- encoder.copy_buffer_to_buffer → staging
- staging.slice(..).map_async(MapMode::Read) → 다음 프레임에 읽기 (1프레임 지연 허용)

1프레임 지연은 auto exposure에서 자연스러움 (사람 눈도 적응에 시간 걸림).

## File Structure

- `crates/voltex_renderer/src/auto_exposure.rs` — AutoExposure 구조체
- `crates/voltex_renderer/src/auto_exposure.wgsl` — 컴퓨트 셰이더
- `crates/voltex_renderer/src/lib.rs` — 모듈 추가

## Testing

- exposure 계산 로직 (순수 수학): avg_lum → target_exposure → clamp → lerp
- min/max clamp 검증
- adaptation lerp 검증 (dt=0 → 변화 없음, dt=큰값 → 목표에 수렴)
