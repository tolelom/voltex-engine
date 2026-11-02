# Phase 7-1: Deferred Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** G-Buffer + Lighting Pass 디퍼드 렌더링 파이프라인으로 다수의 라이트를 효율적으로 처리

**Architecture:** voltex_renderer에 새 모듈 추가. G-Buffer pass(MRT 4개)가 기하 데이터를 기록하고, Lighting pass(풀스크린 삼각형)가 G-Buffer를 읽어 Cook-Torrance BRDF + 섀도우 + IBL 라이팅을 수행. 기존 포워드 PBR은 유지.

**Tech Stack:** Rust, wgpu 28.0, WGSL

**Spec:** `docs/superpowers/specs/2026-03-25-phase7-1-deferred-rendering.md`

---

## File Structure

### voltex_renderer (추가)
- `crates/voltex_renderer/src/gbuffer.rs` — GBuffer 텍스처 생성/리사이즈 (Create)
- `crates/voltex_renderer/src/fullscreen_quad.rs` — 풀스크린 삼각형 (Create)
- `crates/voltex_renderer/src/deferred_gbuffer.wgsl` — G-Buffer pass 셰이더 (Create)
- `crates/voltex_renderer/src/deferred_lighting.wgsl` — Lighting pass 셰이더 (Create)
- `crates/voltex_renderer/src/deferred_pipeline.rs` — 파이프라인 생성 함수들 (Create)
- `crates/voltex_renderer/src/lib.rs` — 새 모듈 등록 (Modify)

### Example (추가)
- `examples/deferred_demo/Cargo.toml` (Create)
- `examples/deferred_demo/src/main.rs` (Create)
- `Cargo.toml` — workspace members (Modify)

---

## Task 1: GBuffer + Fullscreen Triangle

**Files:**
- Create: `crates/voltex_renderer/src/gbuffer.rs`
- Create: `crates/voltex_renderer/src/fullscreen_quad.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

- [ ] **Step 1: gbuffer.rs 작성**

```rust
// crates/voltex_renderer/src/gbuffer.rs

pub const GBUFFER_POSITION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
pub const GBUFFER_NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const GBUFFER_ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
pub const GBUFFER_MATERIAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct GBuffer {
    pub position_view: wgpu::TextureView,
    pub normal_view: wgpu::TextureView,
    pub albedo_view: wgpu::TextureView,
    pub material_view: wgpu::TextureView,
    pub depth_view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl GBuffer {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let position_view = create_rt(device, width, height, GBUFFER_POSITION_FORMAT, "GBuffer Position");
        let normal_view = create_rt(device, width, height, GBUFFER_NORMAL_FORMAT, "GBuffer Normal");
        let albedo_view = create_rt(device, width, height, GBUFFER_ALBEDO_FORMAT, "GBuffer Albedo");
        let material_view = create_rt(device, width, height, GBUFFER_MATERIAL_FORMAT, "GBuffer Material");
        let depth_view = create_depth(device, width, height);
        Self { position_view, normal_view, albedo_view, material_view, depth_view, width, height }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = Self::new(device, width, height);
    }
}

fn create_rt(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat, label: &str) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("GBuffer Depth"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::gpu::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}
```

- [ ] **Step 2: fullscreen_quad.rs 작성**

```rust
// crates/voltex_renderer/src/fullscreen_quad.rs
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct FullscreenVertex {
    pub position: [f32; 2],
}

impl FullscreenVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<FullscreenVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
        ],
    };
}

/// Oversized triangle that covers the entire screen after clipping.
pub const FULLSCREEN_VERTICES: [FullscreenVertex; 3] = [
    FullscreenVertex { position: [-1.0, -1.0] },
    FullscreenVertex { position: [ 3.0, -1.0] },
    FullscreenVertex { position: [-1.0,  3.0] },
];

pub fn create_fullscreen_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Fullscreen Vertex Buffer"),
        contents: bytemuck::cast_slice(&FULLSCREEN_VERTICES),
        usage: wgpu::BufferUsages::VERTEX,
    })
}
```

- [ ] **Step 3: lib.rs에 모듈 등록**

```rust
pub mod gbuffer;
pub mod fullscreen_quad;
```

And add re-exports:
```rust
pub use gbuffer::GBuffer;
pub use fullscreen_quad::{create_fullscreen_vertex_buffer, FullscreenVertex};
```

- [ ] **Step 4: 빌드 확인**

Run: `cargo build -p voltex_renderer`
Expected: 컴파일 성공

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_renderer/src/gbuffer.rs crates/voltex_renderer/src/fullscreen_quad.rs crates/voltex_renderer/src/lib.rs
git commit -m "feat(renderer): add GBuffer and fullscreen triangle for deferred rendering"
```

---

## Task 2: G-Buffer Pass 셰이더

**Files:**
- Create: `crates/voltex_renderer/src/deferred_gbuffer.wgsl`

- [ ] **Step 1: deferred_gbuffer.wgsl 작성**

```wgsl
// G-Buffer pass: writes geometry data to multiple render targets

struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec3<f32>,
};

struct MaterialUniform {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;
@group(1) @binding(2) var t_normal: texture_2d<f32>;
@group(1) @binding(3) var s_normal: sampler;

@group(2) @binding(0) var<uniform> material: MaterialUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) world_bitangent: vec3<f32>,
};

struct GBufferOutput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) albedo: vec4<f32>,
    @location(3) material_out: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = camera.model * vec4<f32>(in.position, 1.0);
    out.world_pos = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.world_normal = normalize((camera.model * vec4<f32>(in.normal, 0.0)).xyz);
    out.uv = in.uv;

    let T = normalize((camera.model * vec4<f32>(in.tangent.xyz, 0.0)).xyz);
    let N = out.world_normal;
    let B = cross(N, T) * in.tangent.w;
    out.world_tangent = T;
    out.world_bitangent = B;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
    var out: GBufferOutput;

    // World position
    out.position = vec4<f32>(in.world_pos, 1.0);

    // Normal mapping
    let T = normalize(in.world_tangent);
    let B = normalize(in.world_bitangent);
    let N_geom = normalize(in.world_normal);
    let normal_sample = textureSample(t_normal, s_normal, in.uv).rgb;
    let tangent_normal = normal_sample * 2.0 - 1.0;
    let TBN = mat3x3<f32>(T, B, N_geom);
    let N = normalize(TBN * tangent_normal);
    out.normal = vec4<f32>(N, 0.0);

    // Albedo
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    out.albedo = vec4<f32>(material.base_color.rgb * tex_color.rgb, 1.0);

    // Material: R=metallic, G=roughness, B=ao
    out.material_out = vec4<f32>(material.metallic, material.roughness, material.ao, 1.0);

    return out;
}
```

- [ ] **Step 2: 커밋**

```bash
git add crates/voltex_renderer/src/deferred_gbuffer.wgsl
git commit -m "feat(renderer): add G-Buffer pass shader for deferred rendering"
```

---

## Task 3: Lighting Pass 셰이더

**Files:**
- Create: `crates/voltex_renderer/src/deferred_lighting.wgsl`

- [ ] **Step 1: deferred_lighting.wgsl 작성**

This shader reuses the Cook-Torrance BRDF functions from pbr_shader.wgsl but reads from G-Buffer instead of vertex attributes.

```wgsl
// Deferred Lighting Pass: reads G-Buffer, applies full PBR lighting

// Group 0: G-Buffer textures
@group(0) @binding(0) var t_position: texture_2d<f32>;
@group(0) @binding(1) var t_normal: texture_2d<f32>;
@group(0) @binding(2) var t_albedo: texture_2d<f32>;
@group(0) @binding(3) var t_material: texture_2d<f32>;
@group(0) @binding(4) var s_gbuffer: sampler;

// Group 1: Lights + Camera
struct LightData {
    position: vec3<f32>,
    light_type: u32,
    direction: vec3<f32>,
    range: f32,
    color: vec3<f32>,
    intensity: f32,
    inner_cone: f32,
    outer_cone: f32,
    _padding: vec2<f32>,
};

struct LightsUniform {
    lights: array<LightData, 16>,
    count: u32,
    ambient_color: vec3<f32>,
};

struct CameraPositionUniform {
    camera_pos: vec3<f32>,
};

@group(1) @binding(0) var<uniform> lights_uniform: LightsUniform;
@group(1) @binding(1) var<uniform> camera_data: CameraPositionUniform;

// Group 2: Shadow + IBL
struct ShadowUniform {
    light_view_proj: mat4x4<f32>,
    shadow_map_size: f32,
    shadow_bias: f32,
};

@group(2) @binding(0) var t_shadow: texture_depth_2d;
@group(2) @binding(1) var s_shadow: sampler_comparison;
@group(2) @binding(2) var<uniform> shadow: ShadowUniform;
@group(2) @binding(3) var t_brdf_lut: texture_2d<f32>;
@group(2) @binding(4) var s_brdf_lut: sampler;

// Fullscreen vertex
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    // Convert clip space [-1,1] to UV [0,1]
    out.uv = vec2<f32>(position.x * 0.5 + 0.5, 1.0 - (position.y * 0.5 + 0.5));
    return out;
}

// === BRDF functions (same as pbr_shader.wgsl) ===

fn distribution_ggx(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;
    let denom_inner = NdotH2 * (a2 - 1.0) + 1.0;
    let denom = 3.14159265358979 * denom_inner * denom_inner;
    return a2 / denom;
}

fn geometry_schlick_ggx(NdotV: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return NdotV / (NdotV * (1.0 - k) + k);
}

fn geometry_smith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    return geometry_schlick_ggx(NdotV, roughness) * geometry_schlick_ggx(NdotL, roughness);
}

fn fresnel_schlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

fn attenuation_point(distance: f32, range: f32) -> f32 {
    let d_over_r = distance / range;
    let d_over_r4 = d_over_r * d_over_r * d_over_r * d_over_r;
    let falloff = clamp(1.0 - d_over_r4, 0.0, 1.0);
    return (falloff * falloff) / (distance * distance + 0.0001);
}

fn attenuation_spot(light: LightData, L: vec3<f32>) -> f32 {
    let spot_dir = normalize(light.direction);
    let theta = dot(spot_dir, -L);
    return clamp(
        (theta - light.outer_cone) / (light.inner_cone - light.outer_cone + 0.0001),
        0.0, 1.0,
    );
}

fn compute_light_contribution(
    light: LightData, N: vec3<f32>, V: vec3<f32>, world_pos: vec3<f32>,
    F0: vec3<f32>, albedo: vec3<f32>, metallic: f32, roughness: f32,
) -> vec3<f32> {
    var L: vec3<f32>;
    var radiance: vec3<f32>;

    if light.light_type == 0u {
        L = normalize(-light.direction);
        radiance = light.color * light.intensity;
    } else if light.light_type == 1u {
        let to_light = light.position - world_pos;
        let dist = length(to_light);
        L = normalize(to_light);
        radiance = light.color * light.intensity * attenuation_point(dist, light.range);
    } else {
        let to_light = light.position - world_pos;
        let dist = length(to_light);
        L = normalize(to_light);
        radiance = light.color * light.intensity * attenuation_point(dist, light.range) * attenuation_spot(light, L);
    }

    let H = normalize(V + L);
    let NDF = distribution_ggx(N, H, roughness);
    let G = geometry_smith(N, V, L, roughness);
    let F = fresnel_schlick(max(dot(H, V), 0.0), F0);
    let ks = F;
    let kd = (vec3<f32>(1.0) - ks) * (1.0 - metallic);
    let numerator = NDF * G * F;
    let NdotL = max(dot(N, L), 0.0);
    let NdotV = max(dot(N, V), 0.0);
    let denominator = 4.0 * NdotV * NdotL + 0.0001;
    let specular = numerator / denominator;

    return (kd * albedo / 3.14159265358979 + specular) * radiance * NdotL;
}

fn calculate_shadow(world_pos: vec3<f32>) -> f32 {
    if shadow.shadow_map_size == 0.0 { return 1.0; }
    let light_space_pos = shadow.light_view_proj * vec4<f32>(world_pos, 1.0);
    let proj_coords = light_space_pos.xyz / light_space_pos.w;
    let shadow_uv = vec2<f32>(proj_coords.x * 0.5 + 0.5, -proj_coords.y * 0.5 + 0.5);
    let current_depth = proj_coords.z;
    if shadow_uv.x < 0.0 || shadow_uv.x > 1.0 || shadow_uv.y < 0.0 || shadow_uv.y > 1.0 { return 1.0; }
    if current_depth > 1.0 || current_depth < 0.0 { return 1.0; }
    let texel_size = 1.0 / shadow.shadow_map_size;
    var shadow_val = 0.0;
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            shadow_val += textureSampleCompare(t_shadow, s_shadow, shadow_uv + vec2<f32>(f32(x), f32(y)) * texel_size, current_depth - shadow.shadow_bias);
        }
    }
    return shadow_val / 9.0;
}

fn sample_environment(direction: vec3<f32>, roughness: f32) -> vec3<f32> {
    var env: vec3<f32>;
    if direction.y > 0.0 {
        env = mix(vec3<f32>(0.6, 0.6, 0.5), vec3<f32>(0.3, 0.5, 0.9), pow(direction.y, 0.4));
    } else {
        env = mix(vec3<f32>(0.6, 0.6, 0.5), vec3<f32>(0.1, 0.08, 0.06), pow(-direction.y, 0.4));
    }
    return mix(env, vec3<f32>(0.3, 0.35, 0.4), roughness * roughness);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = textureSample(t_position, s_gbuffer, in.uv).xyz;
    let N = normalize(textureSample(t_normal, s_gbuffer, in.uv).xyz);
    let albedo = textureSample(t_albedo, s_gbuffer, in.uv).rgb;
    let mat_data = textureSample(t_material, s_gbuffer, in.uv);
    let metallic = mat_data.r;
    let roughness = mat_data.g;
    let ao = mat_data.b;

    // Skip background pixels (position = 0,0,0 means no geometry)
    if length(textureSample(t_position, s_gbuffer, in.uv).xyz) < 0.001 {
        return vec4<f32>(0.05, 0.05, 0.08, 1.0); // background color
    }

    let V = normalize(camera_data.camera_pos - world_pos);
    let F0 = mix(vec3<f32>(0.04), albedo, metallic);

    let shadow_factor = calculate_shadow(world_pos);
    var Lo = vec3<f32>(0.0);
    let light_count = min(lights_uniform.count, 16u);
    for (var i = 0u; i < light_count; i++) {
        var contribution = compute_light_contribution(
            lights_uniform.lights[i], N, V, world_pos, F0, albedo, metallic, roughness,
        );
        if lights_uniform.lights[i].light_type == 0u {
            contribution = contribution * shadow_factor;
        }
        Lo += contribution;
    }

    // IBL
    let NdotV_ibl = max(dot(N, V), 0.0);
    let R = reflect(-V, N);
    let irradiance = sample_environment(N, 1.0);
    let F_env = fresnel_schlick(NdotV_ibl, F0);
    let kd_ibl = (vec3<f32>(1.0) - F_env) * (1.0 - metallic);
    let diffuse_ibl = kd_ibl * albedo * irradiance;
    let prefiltered = sample_environment(R, roughness);
    let brdf_val = textureSample(t_brdf_lut, s_brdf_lut, vec2<f32>(NdotV_ibl, roughness));
    let specular_ibl = prefiltered * (F0 * brdf_val.r + vec3<f32>(brdf_val.g));
    let ambient = (diffuse_ibl + specular_ibl) * ao;

    var color = ambient + Lo;
    color = color / (color + vec3<f32>(1.0)); // Reinhard
    color = pow(color, vec3<f32>(1.0 / 2.2)); // Gamma

    return vec4<f32>(color, 1.0);
}
```

- [ ] **Step 2: 커밋**

```bash
git add crates/voltex_renderer/src/deferred_lighting.wgsl
git commit -m "feat(renderer): add deferred lighting pass shader with Cook-Torrance BRDF"
```

---

## Task 4: Deferred Pipeline (Rust)

**Files:**
- Create: `crates/voltex_renderer/src/deferred_pipeline.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

- [ ] **Step 1: deferred_pipeline.rs 작성**

This file creates both G-Buffer pass and Lighting pass pipelines, plus their bind group layouts.

```rust
// crates/voltex_renderer/src/deferred_pipeline.rs
use crate::vertex::MeshVertex;
use crate::fullscreen_quad::FullscreenVertex;
use crate::gbuffer::*;
use crate::gpu::DEPTH_FORMAT;

// === G-Buffer Pass ===

pub fn gbuffer_camera_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("GBuffer Camera BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

pub fn create_gbuffer_pipeline(
    device: &wgpu::Device,
    camera_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
    material_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Deferred GBuffer Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("deferred_gbuffer.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("GBuffer Pipeline Layout"),
        bind_group_layouts: &[camera_layout, texture_layout, material_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("GBuffer Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[MeshVertex::LAYOUT],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[
                Some(wgpu::ColorTargetState {
                    format: GBUFFER_POSITION_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: GBUFFER_NORMAL_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: GBUFFER_ALBEDO_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: GBUFFER_MATERIAL_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
            ],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

// === Lighting Pass ===

pub fn lighting_gbuffer_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Lighting GBuffer BGL"),
        entries: &[
            // position texture
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // normal texture
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // albedo texture
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // material texture
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // sampler
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    })
}

pub fn lighting_lights_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Lighting Lights BGL"),
        entries: &[
            // LightsUniform
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // CameraPositionUniform
            wgpu::BindGroupLayoutEntry {
                binding: 1,
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

pub fn lighting_shadow_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Lighting Shadow+IBL BGL"),
        entries: &[
            // shadow depth texture
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // shadow comparison sampler
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            // ShadowUniform
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
            // BRDF LUT texture
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // BRDF LUT sampler
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

pub fn create_lighting_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    gbuffer_layout: &wgpu::BindGroupLayout,
    lights_layout: &wgpu::BindGroupLayout,
    shadow_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Deferred Lighting Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("deferred_lighting.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Lighting Pipeline Layout"),
        bind_group_layouts: &[gbuffer_layout, lights_layout, shadow_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Lighting Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[FullscreenVertex::LAYOUT],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None, // No depth for fullscreen pass
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
```

- [ ] **Step 2: lib.rs에 deferred_pipeline 등록**

```rust
pub mod deferred_pipeline;
```

And re-exports:
```rust
pub use deferred_pipeline::{
    create_gbuffer_pipeline, create_lighting_pipeline,
    gbuffer_camera_bind_group_layout,
    lighting_gbuffer_bind_group_layout, lighting_lights_bind_group_layout, lighting_shadow_bind_group_layout,
};
```

- [ ] **Step 3: 빌드 확인**

Run: `cargo build -p voltex_renderer`
Expected: 컴파일 성공

Run: `cargo test --workspace`
Expected: all pass (기존 200개)

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_renderer/src/deferred_pipeline.rs crates/voltex_renderer/src/lib.rs
git commit -m "feat(renderer): add deferred rendering pipeline (G-Buffer + Lighting pass)"
```

---

## Task 5: deferred_demo 예제

**Files:**
- Create: `examples/deferred_demo/Cargo.toml`
- Create: `examples/deferred_demo/src/main.rs`
- Modify: `Cargo.toml` (workspace members)

NOTE: 이 예제는 복잡합니다 (GPU 리소스 설정, 바인드 그룹 생성, 2-pass 렌더). 기존 pbr_demo 패턴을 따르되 디퍼드로 변경. 구체 그리드 + 다수 포인트 라이트 씬.

이 태스크는 가장 큰 구현이며, 더 capable한 모델로 실행해야 합니다.

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "deferred_demo"
version = "0.1.0"
edition = "2021"

[dependencies]
voltex_math.workspace = true
voltex_platform.workspace = true
voltex_renderer.workspace = true
bytemuck.workspace = true
pollster.workspace = true
wgpu.workspace = true
```

- [ ] **Step 2: main.rs 작성**

The example should:
1. Create window + GpuContext
2. Create GBuffer
3. Create G-Buffer pipeline + Lighting pipeline with proper bind group layouts
4. Generate sphere meshes (5x5 grid of metallic/roughness variations)
5. Set up 8 point lights orbiting the scene (to show deferred advantage)
6. Create all uniform buffers, textures, bind groups
7. Main loop:
   - Update camera (FPS controller)
   - Update light positions (orbit animation)
   - Pass 1: G-Buffer pass (render all objects to MRT)
   - Pass 2: Lighting pass (fullscreen quad, reads G-Buffer)
   - Present

Key: must create CameraPositionUniform buffer (vec3 + padding = 16 bytes) for the lighting pass.

- [ ] **Step 3: workspace에 추가**

`Cargo.toml` members에 `"examples/deferred_demo"` 추가.

- [ ] **Step 4: 빌드 + 실행 확인**

Run: `cargo build --bin deferred_demo`
Run: `cargo run --bin deferred_demo` (수동 확인)

- [ ] **Step 5: 커밋**

```bash
git add examples/deferred_demo/ Cargo.toml
git commit -m "feat(renderer): add deferred_demo example with multi-light deferred rendering"
```

---

## Task 6: 문서 업데이트

**Files:**
- Modify: `docs/dev/STATUS.md`
- Modify: `docs/dev/DEFERRED.md`

- [ ] **Step 1: STATUS.md에 Phase 7-1 추가**

Phase 6-3 아래에:
```markdown
### Phase 7-1: Deferred Rendering
- voltex_renderer: GBuffer (4 MRT: Position/Normal/Albedo/Material + Depth)
- voltex_renderer: G-Buffer pass shader (MRT output, TBN normal mapping)
- voltex_renderer: Lighting pass shader (fullscreen quad, Cook-Torrance BRDF, multi-light, shadow, IBL)
- voltex_renderer: Deferred pipeline (gbuffer + lighting bind group layouts)
- examples/deferred_demo (5x5 sphere grid + 8 orbiting point lights)
```

예제 수 11로 업데이트.

- [ ] **Step 2: DEFERRED.md에 Phase 7-1 미뤄진 항목 추가**

```markdown
## Phase 7-1

- **투명 오브젝트** — 디퍼드에서 처리 불가. 별도 포워드 패스 필요.
- **G-Buffer 압축** — Position을 depth에서 복원, Normal을 octahedral 인코딩 등 미적용.
- **Light Volumes** — 풀스크린 라이팅만. 라이트별 sphere/cone 렌더 미구현.
- **Stencil 최적화** — 미구현.
```

- [ ] **Step 3: 커밋**

```bash
git add docs/dev/STATUS.md docs/dev/DEFERRED.md
git commit -m "docs: add Phase 7-1 deferred rendering status and deferred items"
```
