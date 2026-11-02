# Phase 7-4: HDR + Bloom + ACES Tonemap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** HDR 파이프라인으로 전환하고 Bloom + ACES 톤매핑으로 AAA 비주얼 품질 달성

**Architecture:** Lighting Pass 출력을 Rgba16Float HDR 텍스처로 변경. Bloom 패스(bright extract + 5-level downsample/upsample chain). Tonemap 패스(HDR + Bloom → ACES + 감마 → surface). 총 9 bloom sub-passes + 1 tonemap pass 추가.

**Tech Stack:** Rust, wgpu 28.0, WGSL

**Spec:** `docs/superpowers/specs/2026-03-25-phase7-4-post-processing.md`

---

## File Structure

### 새 파일
- `crates/voltex_renderer/src/hdr.rs` — HdrTarget (Create)
- `crates/voltex_renderer/src/bloom.rs` — BloomResources, BloomUniform, mip chain (Create)
- `crates/voltex_renderer/src/bloom_shader.wgsl` — downsample (bright extract) + upsample (Create)
- `crates/voltex_renderer/src/tonemap.rs` — TonemapUniform (Create)
- `crates/voltex_renderer/src/tonemap_shader.wgsl` — ACES + bloom merge + gamma (Create)

### 수정 파일
- `crates/voltex_renderer/src/deferred_pipeline.rs` — bloom/tonemap 파이프라인 추가 (Modify)
- `crates/voltex_renderer/src/deferred_lighting.wgsl` — Reinhard+감마 제거 (Modify)
- `crates/voltex_renderer/src/lib.rs` — 새 모듈 등록 (Modify)
- `examples/deferred_demo/src/main.rs` — HDR+Bloom+Tonemap 통합 (Modify)

---

## Task 1: HdrTarget + Bloom/Tonemap 리소스

**Files:**
- Create: `crates/voltex_renderer/src/hdr.rs`
- Create: `crates/voltex_renderer/src/bloom.rs`
- Create: `crates/voltex_renderer/src/tonemap.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

- [ ] **Step 1: hdr.rs 작성**

```rust
// crates/voltex_renderer/src/hdr.rs
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub struct HdrTarget {
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl HdrTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let view = create_hdr_texture(device, width, height);
        Self { view, width, height }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.view = create_hdr_texture(device, width, height);
        self.width = width;
        self.height = height;
    }
}

fn create_hdr_texture(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("HDR Target"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}
```

- [ ] **Step 2: bloom.rs 작성**

```rust
// crates/voltex_renderer/src/bloom.rs
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

pub const BLOOM_MIP_COUNT: usize = 5;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct BloomUniform {
    pub threshold: f32,
    pub soft_threshold: f32,
    pub _padding: [f32; 2],
}

impl Default for BloomUniform {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            soft_threshold: 0.5,
            _padding: [0.0; 2],
        }
    }
}

pub struct BloomResources {
    pub mip_views: Vec<wgpu::TextureView>,
    pub uniform_buffer: wgpu::Buffer,
    pub intensity: f32,
}

impl BloomResources {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let mip_views = create_mip_chain(device, width, height);
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bloom Uniform"),
            contents: bytemuck::bytes_of(&BloomUniform::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self { mip_views, uniform_buffer, intensity: 0.5 }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.mip_views = create_mip_chain(device, width, height);
    }
}

fn create_mip_chain(device: &wgpu::Device, width: u32, height: u32) -> Vec<wgpu::TextureView> {
    let sizes = mip_sizes(width, height);
    sizes.iter().enumerate().map(|(i, (w, h))| {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Bloom Mip {}", i)),
            size: wgpu::Extent3d { width: *w, height: *h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: crate::hdr::HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }).collect()
}

/// Calculate mip chain sizes (5 levels, each half of previous).
pub fn mip_sizes(width: u32, height: u32) -> Vec<(u32, u32)> {
    let mut sizes = Vec::with_capacity(BLOOM_MIP_COUNT);
    let mut w = (width / 2).max(1);
    let mut h = (height / 2).max(1);
    for _ in 0..BLOOM_MIP_COUNT {
        sizes.push((w, h));
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    sizes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mip_sizes() {
        let sizes = mip_sizes(1920, 1080);
        assert_eq!(sizes.len(), 5);
        assert_eq!(sizes[0], (960, 540));
        assert_eq!(sizes[1], (480, 270));
        assert_eq!(sizes[2], (240, 135));
        assert_eq!(sizes[3], (120, 67));
        assert_eq!(sizes[4], (60, 33));
    }

    #[test]
    fn test_mip_sizes_small() {
        let sizes = mip_sizes(64, 64);
        assert_eq!(sizes[0], (32, 32));
        assert_eq!(sizes[4], (2, 2));
    }

    #[test]
    fn test_bloom_uniform_default() {
        let u = BloomUniform::default();
        assert!((u.threshold - 1.0).abs() < 1e-5);
    }
}
```

- [ ] **Step 3: tonemap.rs 작성**

```rust
// crates/voltex_renderer/src/tonemap.rs
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TonemapUniform {
    pub bloom_intensity: f32,
    pub exposure: f32,
    pub _padding: [f32; 2],
}

impl Default for TonemapUniform {
    fn default() -> Self {
        Self {
            bloom_intensity: 0.5,
            exposure: 1.0,
            _padding: [0.0; 2],
        }
    }
}

/// ACES tonemap (CPU version for testing).
pub fn aces_tonemap(x: f32) -> f32 {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    ((x * (a * x + b)) / (x * (c * x + d) + e)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aces_zero() {
        assert!((aces_tonemap(0.0)).abs() < 1e-5);
    }

    #[test]
    fn test_aces_one() {
        let v = aces_tonemap(1.0);
        assert!(v > 0.5 && v < 0.7, "aces(1.0) = {}, expected ~0.59", v);
    }

    #[test]
    fn test_aces_high() {
        let v = aces_tonemap(10.0);
        assert!(v > 0.95, "aces(10.0) = {}, expected ~1.0", v);
    }

    #[test]
    fn test_tonemap_uniform_default() {
        let u = TonemapUniform::default();
        assert!((u.exposure - 1.0).abs() < 1e-5);
        assert!((u.bloom_intensity - 0.5).abs() < 1e-5);
    }
}
```

- [ ] **Step 4: lib.rs에 모듈 등록**

```rust
pub mod hdr;
pub mod bloom;
pub mod tonemap;

pub use hdr::{HdrTarget, HDR_FORMAT};
pub use bloom::{BloomResources, BloomUniform, mip_sizes, BLOOM_MIP_COUNT};
pub use tonemap::{TonemapUniform, aces_tonemap};
```

- [ ] **Step 5: 빌드 + 테스트**

Run: `cargo test -p voltex_renderer`
Expected: 기존 26 + 7 = 33 PASS

- [ ] **Step 6: 커밋**

```bash
git add crates/voltex_renderer/src/hdr.rs crates/voltex_renderer/src/bloom.rs crates/voltex_renderer/src/tonemap.rs crates/voltex_renderer/src/lib.rs
git commit -m "feat(renderer): add HDR target, Bloom resources, and ACES tonemap"
```

---

## Task 2: Bloom + Tonemap 셰이더

**Files:**
- Create: `crates/voltex_renderer/src/bloom_shader.wgsl`
- Create: `crates/voltex_renderer/src/tonemap_shader.wgsl`

- [ ] **Step 1: bloom_shader.wgsl 작성**

Single shader with two entry points: downsample (bright extract on first pass) and upsample.

```wgsl
// Bloom shader: downsample with bright extract + upsample with additive blend

struct BloomUniform {
    threshold: f32,
    soft_threshold: f32,
};

@group(0) @binding(0) var t_input: texture_2d<f32>;
@group(0) @binding(1) var s_input: sampler;
@group(0) @binding(2) var<uniform> bloom: BloomUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    out.uv = vec2<f32>(position.x * 0.5 + 0.5, 1.0 - (position.y * 0.5 + 0.5));
    return out;
}

// Downsample with optional bright extract (first pass uses threshold, subsequent passes threshold=0)
@fragment
fn fs_downsample(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel_size = 1.0 / vec2<f32>(textureDimensions(t_input));

    // 4-sample box filter with offset for better quality
    let a = textureSample(t_input, s_input, in.uv + texel_size * vec2(-1.0, -1.0)).rgb;
    let b = textureSample(t_input, s_input, in.uv + texel_size * vec2( 1.0, -1.0)).rgb;
    let c = textureSample(t_input, s_input, in.uv + texel_size * vec2(-1.0,  1.0)).rgb;
    let d = textureSample(t_input, s_input, in.uv + texel_size * vec2( 1.0,  1.0)).rgb;
    let center = textureSample(t_input, s_input, in.uv).rgb;

    var color = (a + b + c + d + center * 4.0) / 8.0;

    // Bright extract (only effective when threshold > 0)
    if bloom.threshold > 0.0 {
        let luminance = dot(color, vec3(0.2126, 0.7152, 0.0722));
        let contribution = max(luminance - bloom.soft_threshold, 0.0) / max(luminance, 0.0001);
        color = color * contribution;
    }

    return vec4(color, 1.0);
}

// Upsample with bilinear + additive blend (reads from lower mip)
@fragment
fn fs_upsample(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel_size = 1.0 / vec2<f32>(textureDimensions(t_input));

    // 9-tap tent filter for smooth upsample
    var color = textureSample(t_input, s_input, in.uv).rgb * 4.0;
    color += textureSample(t_input, s_input, in.uv + vec2( texel_size.x, 0.0)).rgb * 2.0;
    color += textureSample(t_input, s_input, in.uv + vec2(-texel_size.x, 0.0)).rgb * 2.0;
    color += textureSample(t_input, s_input, in.uv + vec2(0.0,  texel_size.y)).rgb * 2.0;
    color += textureSample(t_input, s_input, in.uv + vec2(0.0, -texel_size.y)).rgb * 2.0;
    color += textureSample(t_input, s_input, in.uv + vec2( texel_size.x,  texel_size.y)).rgb;
    color += textureSample(t_input, s_input, in.uv + vec2(-texel_size.x,  texel_size.y)).rgb;
    color += textureSample(t_input, s_input, in.uv + vec2( texel_size.x, -texel_size.y)).rgb;
    color += textureSample(t_input, s_input, in.uv + vec2(-texel_size.x, -texel_size.y)).rgb;
    color /= 16.0;

    return vec4(color, 1.0);
}
```

- [ ] **Step 2: tonemap_shader.wgsl 작성**

```wgsl
// Tonemap pass: combine HDR + Bloom, apply ACES tonemap + gamma

struct TonemapUniform {
    bloom_intensity: f32,
    exposure: f32,
};

@group(0) @binding(0) var t_hdr: texture_2d<f32>;
@group(0) @binding(1) var t_bloom: texture_2d<f32>;
@group(0) @binding(2) var s_sampler: sampler;
@group(0) @binding(3) var<uniform> tonemap: TonemapUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    out.uv = vec2<f32>(position.x * 0.5 + 0.5, 1.0 - (position.y * 0.5 + 0.5));
    return out;
}

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3(0.0), vec3(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr_color = textureSample(t_hdr, s_sampler, in.uv).rgb;
    let bloom_color = textureSample(t_bloom, s_sampler, in.uv).rgb;

    // Combine HDR + Bloom
    let combined = hdr_color + bloom_color * tonemap.bloom_intensity;

    // Apply exposure
    let exposed = combined * tonemap.exposure;

    // ACES tonemap
    let tonemapped = aces_tonemap(exposed);

    // Gamma correction (linear → sRGB)
    let gamma = pow(tonemapped, vec3(1.0 / 2.2));

    return vec4(gamma, 1.0);
}
```

- [ ] **Step 3: 커밋**

```bash
git add crates/voltex_renderer/src/bloom_shader.wgsl crates/voltex_renderer/src/tonemap_shader.wgsl
git commit -m "feat(renderer): add bloom and tonemap shaders"
```

---

## Task 3: Bloom + Tonemap 파이프라인 + Lighting HDR 전환

**Files:**
- Modify: `crates/voltex_renderer/src/deferred_pipeline.rs`
- Modify: `crates/voltex_renderer/src/deferred_lighting.wgsl`

- [ ] **Step 1: deferred_pipeline.rs에 bloom/tonemap 파이프라인 함수 추가**

```rust
use crate::hdr::HDR_FORMAT;
use crate::fullscreen_quad::FullscreenVertex;

/// Bloom pass bind group layout (shared for downsample and upsample).
/// Group 0: input texture + sampler + BloomUniform
pub fn bloom_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bloom BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
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

/// Create bloom downsample pipeline (entry point: fs_downsample).
pub fn create_bloom_downsample_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Bloom Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("bloom_shader.wgsl").into()),
    });
    let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Bloom Downsample Layout"),
        bind_group_layouts: &[layout],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Bloom Downsample"),
        layout: Some(&pipe_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[FullscreenVertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_downsample"),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Create bloom upsample pipeline (entry point: fs_upsample).
/// Output blends additively with the render target.
pub fn create_bloom_upsample_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Bloom Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("bloom_shader.wgsl").into()),
    });
    let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Bloom Upsample Layout"),
        bind_group_layouts: &[layout],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Bloom Upsample"),
        layout: Some(&pipe_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[FullscreenVertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_upsample"),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Tonemap pass bind group layout.
pub fn tonemap_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Tonemap BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
            wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
        ],
    })
}

/// Create tonemap pipeline.
pub fn create_tonemap_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Tonemap Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("tonemap_shader.wgsl").into()),
    });
    let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Tonemap Layout"),
        bind_group_layouts: &[layout],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Tonemap Pipeline"),
        layout: Some(&pipe_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[FullscreenVertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}
```

- [ ] **Step 2: Lighting pipeline의 target format을 HDR로 변경**

In `create_lighting_pipeline`, change the `surface_format` parameter usage: the function should accept `HDR_FORMAT` instead for the color target. The caller passes HDR_FORMAT.

Actually, the function already takes `surface_format` as parameter. The caller (deferred_demo) will just pass `HDR_FORMAT` instead of the surface format.

- [ ] **Step 3: deferred_lighting.wgsl에서 Reinhard + 감마 제거**

Remove the last 4 lines before `return`:
```wgsl
// REMOVE these lines:
// color = color / (color + vec3<f32>(1.0));  // Reinhard
// color = pow(color, vec3<f32>(1.0 / 2.2)); // Gamma
```

The output is now raw HDR linear color.

- [ ] **Step 4: 빌드 확인**

Run: `cargo build -p voltex_renderer`
Expected: 컴파일 성공

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_renderer/src/deferred_pipeline.rs crates/voltex_renderer/src/deferred_lighting.wgsl
git commit -m "feat(renderer): add bloom/tonemap pipelines and convert lighting to HDR output"
```

---

## Task 4: deferred_demo에 HDR + Bloom + Tonemap 통합

**Files:**
- Modify: `examples/deferred_demo/src/main.rs`

이 태스크는 opus 모델로 실행.

변경사항:
1. HdrTarget 생성 (full resolution)
2. BloomResources 생성
3. Bloom 파이프라인 2개 (downsample + upsample) + bind group layout
4. Tonemap 파이프라인 + bind group layout + TonemapUniform buffer
5. Lighting Pass를 HDR texture에 렌더 (surface 대신)
6. Bloom Pass: 9 sub-passes (1 bright extract downsample + 4 downsample + 4 upsample)
7. Tonemap Pass: HDR + bloom mip[0] → ACES + gamma → surface
8. 리사이즈 시 HDR + Bloom 재생성
9. Bloom/Tonemap bind groups 생성 (각 sub-pass마다 다른 입출력)

- [ ] **Step 1: deferred_demo 수정**

- [ ] **Step 2: 빌드 확인**

Run: `cargo build --bin deferred_demo`

- [ ] **Step 3: 커밋**

```bash
git add examples/deferred_demo/src/main.rs
git commit -m "feat(renderer): add HDR + Bloom + ACES tonemap to deferred_demo"
```

---

## Task 5: 문서 업데이트

**Files:**
- Modify: `docs/dev/STATUS.md`
- Modify: `docs/dev/DEFERRED.md`

- [ ] **Step 1: STATUS.md에 Phase 7-4 추가**

```markdown
### Phase 7-4: Post Processing (HDR + Bloom + ACES)
- voltex_renderer: HdrTarget (Rgba16Float), Lighting → HDR output
- voltex_renderer: BloomResources (5-level mip chain, downsample/upsample)
- voltex_renderer: Bloom shader (bright extract + tent filter upsample)
- voltex_renderer: Tonemap shader (ACES filmic + bloom merge + gamma)
- deferred_demo updated with full post-processing pipeline (6 passes)
```

- [ ] **Step 2: DEFERRED.md에 Phase 7-4 미뤄진 항목**

```markdown
## Phase 7-4

- **TAA** — Temporal Anti-Aliasing 미구현. Motion vector 필요.
- **SSR** — Screen-Space Reflections 미구현.
- **Motion Blur, DOF** — 미구현.
- **Auto Exposure** — 고정 exposure만. 적응형 노출 미구현.
- **Bilateral Bloom Blur** — 단순 box/tent filter. Kawase blur 미적용.
```

- [ ] **Step 3: 커밋**

```bash
git add docs/dev/STATUS.md docs/dev/DEFERRED.md
git commit -m "docs: add Phase 7-4 post processing status and deferred items"
```
