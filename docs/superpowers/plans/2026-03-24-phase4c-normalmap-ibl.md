# Phase 4c: Normal Mapping + Simple IBL Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normal map으로 표면 디테일을 추가하고, 프로시저럴 환경광(간이 IBL) + BRDF LUT로 roughness에 따른 반사 차이를 시각적으로 확인할 수 있게 한다.

**Architecture:** MeshVertex에 tangent 벡터를 추가하여 TBN 행렬을 구성하고, PBR 셰이더에서 normal map을 샘플링한다. IBL은 큐브맵 없이 프로시저럴 sky 함수로 환경광을 계산하고, CPU에서 생성한 BRDF LUT로 split-sum 근사를 수행한다. 나중에 프로시저럴 sky를 실제 HDR 큐브맵으로 교체하면 full IBL이 된다.

**Tech Stack:** Rust 1.94, wgpu 28.0, WGSL

---

## File Structure

```
crates/voltex_renderer/src/
├── vertex.rs           # MeshVertex에 tangent 추가 (MODIFY)
├── obj.rs              # tangent 계산 추가 (MODIFY)
├── sphere.rs           # tangent 계산 추가 (MODIFY)
├── brdf_lut.rs         # CPU BRDF LUT 생성 (NEW)
├── ibl.rs              # IBL bind group + dummy resources (NEW)
├── pbr_shader.wgsl     # normal mapping + IBL (MODIFY)
├── pbr_pipeline.rs     # group(4) IBL bind group (MODIFY)
├── shadow_shader.wgsl  # vertex layout 변경 반영 (MODIFY)
├── lib.rs              # re-export 업데이트 (MODIFY)
examples/
└── ibl_demo/           # Normal map + IBL 데모 (NEW)
    ├── Cargo.toml
    └── src/
        └── main.rs
```

---

## Task 1: MeshVertex tangent + 계산

**Files:**
- Modify: `crates/voltex_renderer/src/vertex.rs`
- Modify: `crates/voltex_renderer/src/obj.rs`
- Modify: `crates/voltex_renderer/src/sphere.rs`
- Modify: `crates/voltex_renderer/src/shadow_shader.wgsl`

MeshVertex에 `tangent: [f32; 4]`를 추가 (w=handedness, +1 or -1). OBJ 파서와 sphere 생성기에서 tangent를 계산.

- [ ] **Step 1: vertex.rs — MeshVertex에 tangent 추가**

```rust
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub tangent: [f32; 4],  // xyz = tangent direction, w = handedness (+1 or -1)
}
```

LAYOUT에 tangent attribute 추가:
```rust
// location 3, Float32x4, offset after uv
wgpu::VertexAttribute {
    offset: (std::mem::size_of::<[f32; 3]>() * 2 + std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress,
    shader_location: 3,
    format: wgpu::VertexFormat::Float32x4,
},
```

- [ ] **Step 2: obj.rs — tangent 계산 추가**

OBJ 파서 후처리로 Mikktspace-like tangent 계산. `parse_obj` 끝에서 삼각형별로 tangent를 계산하고 정점에 누적:

```rust
/// 인덱스 배열의 삼각형들로부터 tangent 벡터를 계산.
/// UV 기반으로 tangent/bitangent를 구하고, 정점에 누적 후 정규화.
pub fn compute_tangents(vertices: &mut [MeshVertex], indices: &[u32]) {
    // 각 삼각형에 대해:
    // edge1 = v1.pos - v0.pos, edge2 = v2.pos - v0.pos
    // duv1 = v1.uv - v0.uv, duv2 = v2.uv - v0.uv
    // f = 1.0 / (duv1.x * duv2.y - duv2.x * duv1.y)
    // tangent = f * (duv2.y * edge1 - duv1.y * edge2)
    // bitangent = f * (-duv2.x * edge1 + duv1.x * edge2)
    // 누적 후 정규화, handedness = sign(dot(cross(N, T), B))
}
```

`parse_obj` 끝에서 `compute_tangents(&mut vertices, &indices)` 호출.

- [ ] **Step 3: sphere.rs — tangent 계산 추가**

UV sphere에서 tangent는 해석적으로 계산 가능:
- tangent 방향 = longitude 방향의 접선 (sector angle의 미분)
- `tx = -sin(sector_angle), tz = cos(sector_angle)` (Y-up에서)
- handedness w = 1.0

`generate_sphere`에서 각 정점 생성 시 tangent를 직접 계산.

- [ ] **Step 4: shadow_shader.wgsl — vertex input에 tangent 추가**

shadow shader도 MeshVertex를 사용하므로 VertexInput에 tangent를 추가해야 빌드가 됨:

```wgsl
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};
```

vertex shader는 tangent를 사용하지 않고 position만 변환 — 기존과 동일.

- [ ] **Step 5: 빌드 + 테스트**

Run: `cargo build -p voltex_renderer`
Run: `cargo test -p voltex_renderer`
Expected: 기존 테스트 통과. OBJ 테스트에서 tangent 필드가 추가된 MeshVertex를 확인.

참고: 기존 OBJ 테스트는 position/normal/uv만 검증하므로 tangent 추가로 깨지지 않음. sphere 테스트는 vertex count/index count만 확인하므로 OK.

- [ ] **Step 6: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): add tangent to MeshVertex with computation in OBJ parser and sphere generator"
```

---

## Task 2: BRDF LUT + IBL 리소스

**Files:**
- Create: `crates/voltex_renderer/src/brdf_lut.rs`
- Create: `crates/voltex_renderer/src/ibl.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

BRDF LUT는 split-sum 근사의 핵심. NdotV(x축)와 roughness(y축)에 대한 scale+bias를 2D 텍스처로 저장.

- [ ] **Step 1: brdf_lut.rs — CPU에서 BRDF LUT 생성**

```rust
// crates/voltex_renderer/src/brdf_lut.rs

/// BRDF LUT 생성 (256x256, RG16Float or RGBA8 근사)
/// x축 = NdotV (0..1), y축 = roughness (0..1)
/// 출력: (scale, bias) per texel → R=scale, G=bias
pub fn generate_brdf_lut(size: u32) -> Vec<[f32; 2]> {
    let mut data = vec![[0.0f32; 2]; (size * size) as usize];

    for y in 0..size {
        let roughness = (y as f32 + 0.5) / size as f32;
        for x in 0..size {
            let n_dot_v = (x as f32 + 0.5) / size as f32;
            let (scale, bias) = integrate_brdf(n_dot_v.max(0.001), roughness.max(0.001));
            data[(y * size + x) as usize] = [scale, bias];
        }
    }

    data
}

/// Hammersley sequence (low-discrepancy)
fn radical_inverse_vdc(mut bits: u32) -> f32 {
    bits = (bits << 16) | (bits >> 16);
    bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1);
    bits = ((bits & 0x33333333) << 2) | ((bits & 0xCCCCCCCC) >> 2);
    bits = ((bits & 0x0F0F0F0F) << 4) | ((bits & 0xF0F0F0F0) >> 4);
    bits = ((bits & 0x00FF00FF) << 8) | ((bits & 0xFF00FF00) >> 8);
    bits as f32 * 2.3283064365386963e-10 // 1/2^32
}

fn hammersley(i: u32, n: u32) -> [f32; 2] {
    [i as f32 / n as f32, radical_inverse_vdc(i)]
}

/// GGX importance sampling
fn importance_sample_ggx(xi: [f32; 2], roughness: f32) -> [f32; 3] {
    let a = roughness * roughness;
    let phi = 2.0 * std::f32::consts::PI * xi[0];
    let cos_theta = ((1.0 - xi[1]) / (1.0 + (a * a - 1.0) * xi[1])).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

    [phi.cos() * sin_theta, phi.sin() * sin_theta, cos_theta]
}

/// Numerical integration of BRDF for given NdotV and roughness
fn integrate_brdf(n_dot_v: f32, roughness: f32) -> (f32, f32) {
    let v = [
        (1.0 - n_dot_v * n_dot_v).sqrt(), // sin
        0.0,
        n_dot_v, // cos
    ];
    let n = [0.0f32, 0.0, 1.0];

    let mut scale = 0.0f32;
    let mut bias = 0.0f32;
    let sample_count = 1024u32;

    for i in 0..sample_count {
        let xi = hammersley(i, sample_count);
        let h = importance_sample_ggx(xi, roughness);

        // L = 2 * dot(V, H) * H - V
        let v_dot_h = (v[0] * h[0] + v[1] * h[1] + v[2] * h[2]).max(0.0);
        let l = [
            2.0 * v_dot_h * h[0] - v[0],
            2.0 * v_dot_h * h[1] - v[1],
            2.0 * v_dot_h * h[2] - v[2],
        ];

        let n_dot_l = l[2].max(0.0); // dot(N, L) where N = (0,0,1)
        let n_dot_h = h[2].max(0.0);

        if n_dot_l > 0.0 {
            let g = geometry_smith_ibl(n_dot_v, n_dot_l, roughness);
            let g_vis = (g * v_dot_h) / (n_dot_h * n_dot_v).max(0.001);
            let fc = (1.0 - v_dot_h).powi(5);

            scale += g_vis * (1.0 - fc);
            bias += g_vis * fc;
        }
    }

    (scale / sample_count as f32, bias / sample_count as f32)
}

fn geometry_smith_ibl(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let a = roughness;
    let k = (a * a) / 2.0; // IBL uses k = a^2/2 (not (a+1)^2/8)

    let g1 = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let g2 = n_dot_l / (n_dot_l * (1.0 - k) + k);
    g1 * g2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brdf_lut_dimensions() {
        let lut = generate_brdf_lut(64);
        assert_eq!(lut.len(), 64 * 64);
    }

    #[test]
    fn test_brdf_lut_values_in_range() {
        let lut = generate_brdf_lut(32);
        for [scale, bias] in &lut {
            assert!(*scale >= 0.0 && *scale <= 1.5, "scale out of range: {}", scale);
            assert!(*bias >= 0.0 && *bias <= 1.5, "bias out of range: {}", bias);
        }
    }

    #[test]
    fn test_hammersley() {
        let h = hammersley(0, 16);
        assert_eq!(h[0], 0.0);
    }
}
```

- [ ] **Step 2: ibl.rs — IBL 리소스 관리**

```rust
// crates/voltex_renderer/src/ibl.rs
use crate::brdf_lut::generate_brdf_lut;

pub const BRDF_LUT_SIZE: u32 = 256;

/// IBL 리소스 (BRDF LUT 텍스처)
pub struct IblResources {
    pub brdf_lut_texture: wgpu::Texture,
    pub brdf_lut_view: wgpu::TextureView,
    pub brdf_lut_sampler: wgpu::Sampler,
}

impl IblResources {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        // Generate BRDF LUT on CPU
        let lut_data = generate_brdf_lut(BRDF_LUT_SIZE);

        // Convert [f32; 2] to RGBA8 (R=scale*255, G=bias*255, B=0, A=255)
        let mut pixels = vec![0u8; (BRDF_LUT_SIZE * BRDF_LUT_SIZE * 4) as usize];
        for (i, [scale, bias]) in lut_data.iter().enumerate() {
            pixels[i * 4] = (scale.clamp(0.0, 1.0) * 255.0) as u8;
            pixels[i * 4 + 1] = (bias.clamp(0.0, 1.0) * 255.0) as u8;
            pixels[i * 4 + 2] = 0;
            pixels[i * 4 + 3] = 255;
        }

        let size = wgpu::Extent3d {
            width: BRDF_LUT_SIZE,
            height: BRDF_LUT_SIZE,
            depth_or_array_layers: 1,
        };

        let brdf_lut_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BRDF LUT"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm, // NOT sRGB
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &brdf_lut_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * BRDF_LUT_SIZE),
                rows_per_image: Some(BRDF_LUT_SIZE),
            },
            size,
        );

        let brdf_lut_view = brdf_lut_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let brdf_lut_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("BRDF LUT Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            brdf_lut_texture,
            brdf_lut_view,
            brdf_lut_sampler,
        }
    }

    /// IBL bind group layout (group 4)
    /// binding 0: BRDF LUT texture
    /// binding 1: BRDF LUT sampler
    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("IBL Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("IBL Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.brdf_lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.brdf_lut_sampler),
                },
            ],
        })
    }
}
```

- [ ] **Step 3: lib.rs 업데이트**

```rust
pub mod brdf_lut;
pub mod ibl;
pub use ibl::IblResources;
```

- [ ] **Step 4: 테스트 통과**

Run: `cargo test -p voltex_renderer`
Expected: 기존 + brdf_lut 3개 PASS

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): add BRDF LUT generator and IBL resources"
```

---

## Task 3: PBR 셰이더 Normal Map + IBL 통합

**Files:**
- Modify: `crates/voltex_renderer/src/pbr_shader.wgsl`
- Modify: `crates/voltex_renderer/src/pbr_pipeline.rs`

### PBR 셰이더 변경

1. **VertexInput에 tangent 추가:**
```wgsl
@location(3) tangent: vec4<f32>,
```

2. **VertexOutput에 tangent/bitangent 추가:**
```wgsl
@location(4) world_tangent: vec3<f32>,
@location(5) world_bitangent: vec3<f32>,
```

3. **vs_main에서 TBN 계산:**
```wgsl
let T = normalize((camera.model * vec4<f32>(model_v.tangent.xyz, 0.0)).xyz);
let B = cross(out.world_normal, T) * model_v.tangent.w;
out.world_tangent = T;
out.world_bitangent = B;
```

4. **group(1) 확장 — normal map texture 추가:**
```wgsl
@group(1) @binding(2) var t_normal: texture_2d<f32>;
@group(1) @binding(3) var s_normal: sampler;
```

기존 group(1) bind group layout도 normal map 바인딩을 추가해야 함. 하지만 이것은 기존 GpuTexture의 layout을 변경하는 것이라 영향이 큼.

**대안:** normal map을 material bind group(group 2)에 추가하거나, 별도 bind group 사용.

**가장 간단한 접근:** group(1)에 normal map 추가. texture bind group layout을 확장. 기존 예제에서는 normal map에 1x1 "flat blue" 텍스처 ((128, 128, 255, 255) = (0,0,1) normal) 사용.

5. **group(4) IBL 바인딩:**
```wgsl
@group(4) @binding(0) var t_brdf_lut: texture_2d<f32>;
@group(4) @binding(1) var s_brdf_lut: sampler;
```

6. **프로시저럴 환경 함수:**
```wgsl
fn sample_environment(direction: vec3<f32>, roughness: f32) -> vec3<f32> {
    let t = direction.y * 0.5 + 0.5;
    let sky = mix(vec3<f32>(0.05, 0.05, 0.08), vec3<f32>(0.3, 0.5, 0.9), t);
    let horizon = vec3<f32>(0.6, 0.6, 0.5);
    let ground = vec3<f32>(0.1, 0.08, 0.06);

    var env: vec3<f32>;
    if direction.y > 0.0 {
        env = mix(horizon, sky, pow(direction.y, 0.4));
    } else {
        env = mix(horizon, ground, pow(-direction.y, 0.4));
    }

    // Roughness → blur (lerp toward average)
    let avg = (sky + horizon + ground) / 3.0;
    return mix(env, avg, roughness * roughness);
}
```

7. **fs_main에서 IBL ambient 교체:**

기존:
```wgsl
let ambient = lights_uniform.ambient_color * albedo * ao;
```

새:
```wgsl
let NdotV = max(dot(N, V), 0.0);
let R = reflect(-V, N);

// Diffuse IBL
let irradiance = sample_environment(N, 1.0);
let diffuse_ibl = kd_ambient * albedo * irradiance;

// Specular IBL
let prefiltered = sample_environment(R, roughness);
let brdf_sample = textureSample(t_brdf_lut, s_brdf_lut, vec2<f32>(NdotV, roughness));
let F_env = F0 * brdf_sample.r + vec3<f32>(brdf_sample.g);
let specular_ibl = prefiltered * F_env;

let ambient = (diffuse_ibl + specular_ibl) * ao;
```

여기서 `kd_ambient = (1.0 - F_env_avg) * (1.0 - metallic)` — 에너지 보존.

### PBR 파이프라인 변경

`create_pbr_pipeline`에 `ibl_layout: &wgpu::BindGroupLayout` 파라미터 추가:

```rust
pub fn create_pbr_pipeline(
    device, format,
    camera_light_layout,
    texture_layout,    // group(1): now includes normal map
    material_layout,
    shadow_layout,
    ibl_layout,        // NEW: group(4)
) -> wgpu::RenderPipeline
```

bind_group_layouts: `&[camera_light, texture, material, shadow, ibl]`

### Texture bind group layout 확장

GpuTexture::bind_group_layout을 수정하거나 새 함수를 만들어 normal map도 포함하도록:

```rust
// 기존 (bindings 0-1): albedo texture + sampler
// 새로 추가 (bindings 2-3): normal map texture + sampler
```

기존 예제 호환을 위해 `texture_with_normal_bind_group_layout(device)` 새 함수를 만들고, 기존 `bind_group_layout`은 유지.

- [ ] **Step 1: pbr_shader.wgsl 수정**
- [ ] **Step 2: pbr_pipeline.rs 수정**
- [ ] **Step 3: texture.rs에 normal map 포함 bind group layout 추가**
- [ ] **Step 4: 빌드 확인** — `cargo build -p voltex_renderer`
- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): add normal mapping and procedural IBL to PBR shader"
```

---

## Task 4: 기존 예제 수정 + IBL Demo

**Files:**
- Modify: `examples/pbr_demo/src/main.rs`
- Modify: `examples/multi_light_demo/src/main.rs`
- Modify: `examples/shadow_demo/src/main.rs`
- Create: `examples/ibl_demo/Cargo.toml`
- Create: `examples/ibl_demo/src/main.rs`
- Modify: `Cargo.toml`

### 기존 예제 수정

모든 PBR 예제에:
1. `create_pbr_pipeline`에 `ibl_layout` 파라미터 추가
2. IblResources 생성, IBL bind group 생성
3. Normal map: "flat blue" 1x1 텍스처 (128, 128, 255, 255) 사용
4. texture bind group에 normal map 추가
5. render pass에 IBL bind group (group 4) 설정

### ibl_demo

7x7 구체 그리드 (pbr_demo와 유사하지만 IBL 효과가 보임):
- metallic X축, roughness Y축
- IBL이 켜져 있으므로 roughness 차이가 확연히 보임
- smooth metallic 구체는 환경을 반사, rough 구체는 blurry 반사
- Camera: (0, 0, 12)

반드시 읽어야 할 파일: pbr_demo/src/main.rs (기반), shadow_demo/src/main.rs (shadow bind group 패턴)

- [ ] **Step 1: 기존 예제 수정 (pbr_demo, multi_light_demo, shadow_demo)**
- [ ] **Step 2: ibl_demo 작성**
- [ ] **Step 3: 빌드 + 테스트**
- [ ] **Step 4: 실행 확인** — `cargo run -p ibl_demo`
- [ ] **Step 5: 커밋**

```bash
git add Cargo.toml examples/
git commit -m "feat: add IBL demo with normal mapping and procedural environment lighting"
```

---

## Phase 4c 완료 기준 체크리스트

- [ ] `cargo build --workspace` 성공
- [ ] `cargo test --workspace` — 모든 테스트 통과
- [ ] MeshVertex에 tangent 포함, OBJ/sphere에서 자동 계산
- [ ] PBR 셰이더: TBN 행렬로 normal map 샘플링
- [ ] BRDF LUT: CPU 생성, 256x256 텍스처
- [ ] 프로시저럴 IBL: sky gradient + roughness-based blur
- [ ] `cargo run -p ibl_demo` — roughness 차이가 시각적으로 확연히 보임
- [ ] 기존 예제 모두 동작 (flat normal map + IBL)
