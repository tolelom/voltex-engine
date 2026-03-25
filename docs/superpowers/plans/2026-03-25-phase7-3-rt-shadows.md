# Phase 7-3: RT Shadows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** wgpu ray query로 하드웨어 레이트레이싱 기반 그림자 구현 — 정확한 픽셀-퍼펙트 그림자

**Architecture:** BLAS/TLAS acceleration structure를 구축하고, 컴퓨트 셰이더에서 G-Buffer position을 읽어 light 방향으로 ray query를 수행. 차폐 여부를 R8Unorm shadow 텍스처에 기록. Lighting Pass에서 이 텍스처를 읽어 기존 PCF shadow map 대체.

**Tech Stack:** Rust, wgpu 28.0 (EXPERIMENTAL_RAY_QUERY), WGSL (ray_query)

**Spec:** `docs/superpowers/specs/2026-03-25-phase7-3-rt-shadows.md`

---

## File Structure

### 새 파일
- `crates/voltex_renderer/src/rt_accel.rs` — BLAS/TLAS 생성 관리 (Create)
- `crates/voltex_renderer/src/rt_shadow.rs` — RT Shadow 리소스 + uniform (Create)
- `crates/voltex_renderer/src/rt_shadow_shader.wgsl` — RT shadow 컴퓨트 셰이더 (Create)

### 수정 파일
- `crates/voltex_renderer/src/deferred_pipeline.rs` — RT shadow 컴퓨트 파이프라인, lighting group에 RT shadow binding 추가 (Modify)
- `crates/voltex_renderer/src/deferred_lighting.wgsl` — RT shadow 텍스처 사용 (Modify)
- `crates/voltex_renderer/src/lib.rs` — 새 모듈 등록 (Modify)
- `examples/deferred_demo/src/main.rs` — RT shadow 통합 (Modify)

---

## Task 1: rt_accel.rs — BLAS/TLAS 관리

**Files:**
- Create: `crates/voltex_renderer/src/rt_accel.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

- [ ] **Step 1: rt_accel.rs 작성**

This module wraps wgpu's acceleration structure API.

```rust
// crates/voltex_renderer/src/rt_accel.rs
use crate::vertex::MeshVertex;

/// Mesh data needed to build a BLAS.
pub struct BlasMeshData<'a> {
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub vertex_count: u32,
    pub index_count: u32,
}

/// Manages BLAS/TLAS for ray tracing.
pub struct RtAccel {
    pub blas_list: Vec<wgpu::Blas>,
    pub tlas: wgpu::Tlas,
}

impl RtAccel {
    /// Create acceleration structures.
    /// `meshes` — one BLAS per unique mesh.
    /// `instances` — (mesh_index, transform [3x4 row-major f32; 12]).
    pub fn new(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        meshes: &[BlasMeshData],
        instances: &[(usize, [f32; 12])],
    ) -> Self {
        // 1. Create BLAS for each mesh
        let mut blas_list = Vec::new();
        let mut blas_sizes = Vec::new();

        for mesh in meshes {
            let size_desc = wgpu::BlasTriangleGeometrySizeDescriptor {
                vertex_format: wgpu::VertexFormat::Float32x3,
                vertex_count: mesh.vertex_count,
                index_format: Some(wgpu::IndexFormat::Uint16),
                index_count: Some(mesh.index_count),
                flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
            };
            blas_sizes.push(size_desc);
        }

        for (i, mesh) in meshes.iter().enumerate() {
            let blas = device.create_blas(
                &wgpu::CreateBlasDescriptor {
                    label: Some(&format!("BLAS {}", i)),
                    flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                    update_mode: wgpu::AccelerationStructureUpdateMode::Build,
                },
                wgpu::BlasGeometrySizeDescriptors::Triangles {
                    descriptors: vec![blas_sizes[i].clone()],
                },
            );
            blas_list.push(blas);
        }

        // Build all BLAS
        let blas_entries: Vec<wgpu::BlasBuildEntry> = meshes.iter().enumerate().map(|(i, mesh)| {
            wgpu::BlasBuildEntry {
                blas: &blas_list[i],
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![
                    wgpu::BlasTriangleGeometry {
                        size: &blas_sizes[i],
                        vertex_buffer: mesh.vertex_buffer,
                        first_vertex: 0,
                        vertex_stride: std::mem::size_of::<MeshVertex>() as u64,
                        index_buffer: Some(mesh.index_buffer),
                        first_index: Some(0),
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    },
                ]),
            }
        }).collect();

        // 2. Create TLAS
        let max_instances = instances.len().max(1) as u32;
        let mut tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("TLAS"),
            max_instances,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });

        // Fill TLAS instances
        for (i, (mesh_idx, transform)) in instances.iter().enumerate() {
            tlas[i] = Some(wgpu::TlasInstance::new(
                &blas_list[*mesh_idx],
                *transform,
                0, // custom_data
                0xFF, // mask
            ));
        }

        // 3. Build
        encoder.build_acceleration_structures(
            blas_entries.iter(),
            [&tlas],
        );

        RtAccel { blas_list, tlas }
    }

    /// Update TLAS instance transforms (BLAS stays the same).
    pub fn update_instances(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        instances: &[(usize, [f32; 12])],
    ) {
        for (i, (mesh_idx, transform)) in instances.iter().enumerate() {
            self.tlas[i] = Some(wgpu::TlasInstance::new(
                &self.blas_list[*mesh_idx],
                *transform,
                0,
                0xFF,
            ));
        }

        // Rebuild TLAS only (no BLAS rebuild)
        encoder.build_acceleration_structures(
            std::iter::empty(),
            [&self.tlas],
        );
    }
}

/// Convert a 4x4 column-major matrix to 3x4 row-major transform for TLAS instance.
pub fn mat4_to_tlas_transform(m: &[f32; 16]) -> [f32; 12] {
    // Column-major [c0r0, c0r1, c0r2, c0r3, c1r0, ...] to
    // Row-major 3x4 [r0c0, r0c1, r0c2, r0c3, r1c0, ...]
    [
        m[0], m[4], m[8],  m[12], // row 0
        m[1], m[5], m[9],  m[13], // row 1
        m[2], m[6], m[10], m[14], // row 2
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mat4_to_tlas_transform_identity() {
        let identity: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        let t = mat4_to_tlas_transform(&identity);
        assert_eq!(t, [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn test_mat4_to_tlas_transform_translation() {
        // Column-major translation (5, 10, 15)
        let m: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            5.0, 10.0, 15.0, 1.0,
        ];
        let t = mat4_to_tlas_transform(&m);
        // Row 0: [1, 0, 0, 5]
        assert_eq!(t[3], 5.0);
        assert_eq!(t[7], 10.0);
        assert_eq!(t[11], 15.0);
    }
}
```

- [ ] **Step 2: lib.rs에 모듈 등록**

```rust
pub mod rt_accel;
pub use rt_accel::{RtAccel, BlasMeshData, mat4_to_tlas_transform};
```

- [ ] **Step 3: 빌드 + 테스트**

Run: `cargo test -p voltex_renderer`
Expected: 기존 23 + 2 = 25 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_renderer/src/rt_accel.rs crates/voltex_renderer/src/lib.rs
git commit -m "feat(renderer): add BLAS/TLAS acceleration structure management for RT"
```

---

## Task 2: RT Shadow 리소스 + 컴퓨트 셰이더

**Files:**
- Create: `crates/voltex_renderer/src/rt_shadow.rs`
- Create: `crates/voltex_renderer/src/rt_shadow_shader.wgsl`
- Modify: `crates/voltex_renderer/src/lib.rs`

- [ ] **Step 1: rt_shadow.rs 작성**

```rust
// crates/voltex_renderer/src/rt_shadow.rs
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

pub const RT_SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct RtShadowUniform {
    pub light_direction: [f32; 3],
    pub _pad0: f32,
    pub width: u32,
    pub height: u32,
    pub _pad1: [u32; 2],
}

pub struct RtShadowResources {
    pub shadow_texture: wgpu::Texture,
    pub shadow_view: wgpu::TextureView,
    pub uniform_buffer: wgpu::Buffer,
    pub width: u32,
    pub height: u32,
}

impl RtShadowResources {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let (shadow_texture, shadow_view) = create_shadow_texture(device, width, height);
        let uniform = RtShadowUniform {
            light_direction: [0.0, -1.0, 0.0],
            _pad0: 0.0,
            width,
            height,
            _pad1: [0; 2],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("RT Shadow Uniform"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self { shadow_texture, shadow_view, uniform_buffer, width, height }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let (tex, view) = create_shadow_texture(device, width, height);
        self.shadow_texture = tex;
        self.shadow_view = view;
        self.width = width;
        self.height = height;
    }
}

fn create_shadow_texture(device: &wgpu::Device, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("RT Shadow Texture"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: RT_SHADOW_FORMAT,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rt_shadow_uniform_size() {
        assert_eq!(std::mem::size_of::<RtShadowUniform>(), 32);
    }
}
```

- [ ] **Step 2: rt_shadow_shader.wgsl 작성**

```wgsl
// RT Shadow compute shader
// Traces shadow rays from G-Buffer world positions toward the light

@group(0) @binding(0) var t_position: texture_2d<f32>;
@group(0) @binding(1) var t_normal: texture_2d<f32>;

struct RtShadowUniform {
    light_direction: vec3<f32>,
    _pad0: f32,
    width: u32,
    height: u32,
    _pad1: vec2<u32>,
};

@group(1) @binding(0) var tlas: acceleration_structure;
@group(1) @binding(1) var t_shadow_out: texture_storage_2d<r32float, write>;
@group(1) @binding(2) var<uniform> uniforms: RtShadowUniform;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= uniforms.width || id.y >= uniforms.height {
        return;
    }

    let world_pos = textureLoad(t_position, vec2<i32>(id.xy), 0).xyz;

    // Skip background pixels
    if dot(world_pos, world_pos) < 0.001 {
        textureStore(t_shadow_out, vec2<i32>(id.xy), vec4<f32>(1.0, 0.0, 0.0, 0.0));
        return;
    }

    let normal = normalize(textureLoad(t_normal, vec2<i32>(id.xy), 0).xyz * 2.0 - 1.0);

    // Ray from surface toward light (opposite of light direction)
    let ray_origin = world_pos + normal * 0.01; // bias off surface
    let ray_dir = normalize(-uniforms.light_direction);

    // Trace shadow ray
    var rq: ray_query;
    rayQueryInitialize(&rq, tlas,
        RAY_FLAG_TERMINATE_ON_FIRST_HIT | RAY_FLAG_SKIP_CLOSEST_HIT_SHADER,
        0xFFu, ray_origin, 0.001, ray_dir, 1000.0);
    rayQueryProceed(&rq);

    var shadow_val = 1.0; // lit by default
    if rayQueryGetCommittedIntersectionType(&rq) != RAY_QUERY_COMMITTED_INTERSECTION_NONE {
        shadow_val = 0.0; // in shadow
    }

    textureStore(t_shadow_out, vec2<i32>(id.xy), vec4<f32>(shadow_val, 0.0, 0.0, 0.0));
}
```

- [ ] **Step 3: lib.rs에 모듈 등록**

```rust
pub mod rt_shadow;
pub use rt_shadow::{RtShadowResources, RtShadowUniform, RT_SHADOW_FORMAT};
```

- [ ] **Step 4: 빌드 + 테스트**

Run: `cargo test -p voltex_renderer`
Expected: 26 PASS (25 + 1)

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_renderer/src/rt_shadow.rs crates/voltex_renderer/src/rt_shadow_shader.wgsl crates/voltex_renderer/src/lib.rs
git commit -m "feat(renderer): add RT shadow resources and compute shader"
```

---

## Task 3: RT Shadow 파이프라인 + Lighting 통합

**Files:**
- Modify: `crates/voltex_renderer/src/deferred_pipeline.rs`
- Modify: `crates/voltex_renderer/src/deferred_lighting.wgsl`

- [ ] **Step 1: deferred_pipeline.rs에 RT shadow 파이프라인 함수 추가**

Add import: `use crate::rt_shadow::RT_SHADOW_FORMAT;`

Add these functions:

```rust
/// Compute pipeline bind group layout for RT shadow G-Buffer input (group 0).
pub fn rt_shadow_gbuffer_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("RT Shadow GBuffer BGL"),
        entries: &[
            // position texture
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
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
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    })
}

/// Compute pipeline bind group layout for RT shadow data (group 1).
pub fn rt_shadow_data_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("RT Shadow Data BGL"),
        entries: &[
            // TLAS
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::AccelerationStructure,
                count: None,
            },
            // shadow output (storage texture, write)
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: RT_SHADOW_FORMAT,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            // uniform
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
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

/// Create the RT shadow compute pipeline.
pub fn create_rt_shadow_pipeline(
    device: &wgpu::Device,
    gbuffer_layout: &wgpu::BindGroupLayout,
    data_layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("RT Shadow Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("rt_shadow_shader.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("RT Shadow Pipeline Layout"),
        bind_group_layouts: &[gbuffer_layout, data_layout],
        immediate_size: 0,
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("RT Shadow Compute Pipeline"),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}
```

- [ ] **Step 2: lighting_shadow_bind_group_layout에 RT shadow binding 추가**

기존 8 bindings (0-6 shadow+IBL+SSGI) + 추가:
```rust
// binding 7: RT shadow texture (Float, filterable)
wgpu::BindGroupLayoutEntry {
    binding: 7,
    visibility: wgpu::ShaderStages::FRAGMENT,
    ty: wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    },
    count: None,
},
// binding 8: RT shadow sampler
wgpu::BindGroupLayoutEntry {
    binding: 8,
    visibility: wgpu::ShaderStages::FRAGMENT,
    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
    count: None,
},
```

- [ ] **Step 3: deferred_lighting.wgsl 수정**

Add bindings:
```wgsl
@group(2) @binding(7) var t_rt_shadow: texture_2d<f32>;
@group(2) @binding(8) var s_rt_shadow: sampler;
```

Replace shadow usage in fs_main:
```wgsl
// OLD: let shadow_factor = calculate_shadow(world_pos);
// NEW: Use RT shadow
let rt_shadow_val = textureSample(t_rt_shadow, s_rt_shadow, uv).r;
let shadow_factor = rt_shadow_val;
```

- [ ] **Step 4: 빌드 확인**

Run: `cargo build -p voltex_renderer`
Expected: 컴파일 성공

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_renderer/src/deferred_pipeline.rs crates/voltex_renderer/src/deferred_lighting.wgsl
git commit -m "feat(renderer): add RT shadow compute pipeline and integrate into lighting pass"
```

---

## Task 4: deferred_demo에 RT Shadow 통합

**Files:**
- Modify: `examples/deferred_demo/src/main.rs`

NOTE: 이 태스크가 가장 복잡합니다. GpuContext 대신 직접 device를 생성하여 EXPERIMENTAL_RAY_QUERY feature를 요청해야 합니다.

변경사항:
1. Device 생성 시 `Features::EXPERIMENTAL_RAY_QUERY` 요청
2. `RtAccel::new()` — 구체 메시의 BLAS 빌드, 25개 인스턴스의 TLAS 빌드
3. `RtShadowResources::new()` — RT shadow 텍스처 + uniform
4. RT shadow 컴퓨트 파이프라인 + 바인드 그룹 생성
5. 렌더 루프에 RT shadow 컴퓨트 디스패치 추가 (Pass 3)
6. Lighting shadow 바인드 그룹에 RT shadow 텍스처 추가 (binding 7, 8)
7. 매 프레임 RtShadowUniform 업데이트 (light direction)
8. 리사이즈 시 RT shadow 리소스 재생성

이 태스크는 opus 모델로 실행.

- [ ] **Step 1: deferred_demo 수정**

- [ ] **Step 2: 빌드 확인**

Run: `cargo build --bin deferred_demo`

- [ ] **Step 3: 커밋**

```bash
git add examples/deferred_demo/src/main.rs
git commit -m "feat(renderer): add hardware RT shadows to deferred_demo"
```

---

## Task 5: 문서 업데이트

**Files:**
- Modify: `docs/STATUS.md`
- Modify: `docs/DEFERRED.md`

- [ ] **Step 1: STATUS.md에 Phase 7-3 추가**

```markdown
### Phase 7-3: RT Shadows (Hardware Ray Tracing)
- voltex_renderer: RtAccel (BLAS/TLAS acceleration structure management)
- voltex_renderer: RT Shadow compute shader (ray query, directional light)
- voltex_renderer: RT shadow pipeline + bind group layouts
- voltex_renderer: Lighting pass RT shadow integration
- deferred_demo updated with hardware RT shadows (requires RTX/RDNA2+)
```

- [ ] **Step 2: DEFERRED.md에 Phase 7-3 미뤄진 항목**

```markdown
## Phase 7-3

- **RT Reflections** — 미구현. BLAS/TLAS 인프라 재사용 가능.
- **RT AO** — 미구현.
- **Point/Spot Light RT shadows** — Directional만 구현.
- **Soft RT shadows** — 단일 ray만. Multi-ray soft shadow 미구현.
- **BLAS 업데이트** — 정적 지오메트리만. 동적 메시 변경 시 BLAS 재빌드 필요.
- **Fallback** — RT 미지원 GPU에서 자동 PCF 폴백 미구현.
```

- [ ] **Step 3: 커밋**

```bash
git add docs/STATUS.md docs/DEFERRED.md
git commit -m "docs: add Phase 7-3 RT shadows status and deferred items"
```
