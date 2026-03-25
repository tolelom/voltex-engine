# PBR Texture Maps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add metallic/roughness/AO (ORM) and emissive texture map sampling to PBR shaders, extending bind group 1 from 4 to 8 bindings.

**Architecture:** Extend `pbr_texture_bind_group_layout` with 4 new bindings (ORM texture+sampler, emissive texture+sampler). Update forward PBR shader and deferred G-Buffer shader. Default 1x1 white/black textures when maps not provided. MaterialUniform values become multipliers for texture values.

**Tech Stack:** wgpu 28.0, WGSL shaders. No new Rust crates.

---

### Task 1: Texture Utility Additions

**Files:**
- Modify: `crates/voltex_renderer/src/texture.rs`

- [ ] **Step 1: Write test for black_1x1**

```rust
#[test]
fn test_black_1x1_exists() {
    // This is a compile/API test — GPU tests need device
    // We verify the function signature exists and the module compiles
}
```

- [ ] **Step 2: Add black_1x1 and extended layout functions**

Add to `texture.rs`:

```rust
/// 1x1 black texture for emissive default (no emission).
pub fn black_1x1(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> GpuTexture {
    Self::from_rgba(device, queue, 1, 1, &[0, 0, 0, 255], layout)
}

/// Extended PBR texture layout: albedo + normal + ORM + emissive (8 bindings).
pub fn pbr_full_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("PBR Full Texture Bind Group Layout"),
        entries: &[
            // 0-1: albedo (existing)
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
            // 2-3: normal map (existing)
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // 4-5: ORM (AO/Roughness/Metallic) — NEW
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // 6-7: Emissive — NEW
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 7,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Create bind group for full PBR textures (albedo + normal + ORM + emissive).
pub fn create_pbr_full_texture_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    albedo_view: &wgpu::TextureView,
    albedo_sampler: &wgpu::Sampler,
    normal_view: &wgpu::TextureView,
    normal_sampler: &wgpu::Sampler,
    orm_view: &wgpu::TextureView,
    orm_sampler: &wgpu::Sampler,
    emissive_view: &wgpu::TextureView,
    emissive_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    // ... 8 entries at bindings 0-7
}
```

- [ ] **Step 3: Add exports to lib.rs**

```rust
pub use texture::{pbr_full_texture_bind_group_layout, create_pbr_full_texture_bind_group};
```

- [ ] **Step 4: Commit**

```bash
git add crates/voltex_renderer/src/texture.rs crates/voltex_renderer/src/lib.rs
git commit -m "feat(renderer): add ORM and emissive texture bind group layout"
```

---

### Task 2: Update PBR Forward Shader

**Files:**
- Modify: `crates/voltex_renderer/src/pbr_shader.wgsl`

- [ ] **Step 1: Add ORM and emissive bindings**

Add after existing normal map bindings:

```wgsl
@group(1) @binding(4) var t_orm: texture_2d<f32>;
@group(1) @binding(5) var s_orm: sampler;
@group(1) @binding(6) var t_emissive: texture_2d<f32>;
@group(1) @binding(7) var s_emissive: sampler;
```

- [ ] **Step 2: Update fragment shader to sample ORM**

Replace lines that read material params directly:

```wgsl
// Before:
// let metallic = material.metallic;
// let roughness = material.roughness;
// let ao = material.ao;

// After:
let orm_sample = textureSample(t_orm, s_orm, in.uv);
let ao = orm_sample.r * material.ao;
let roughness = orm_sample.g * material.roughness;
let metallic = orm_sample.b * material.metallic;
```

- [ ] **Step 3: Add emissive to final color**

```wgsl
let emissive = textureSample(t_emissive, s_emissive, in.uv).rgb;

// Before tone mapping:
var color = ambient + Lo + emissive;
```

- [ ] **Step 4: Verify existing examples still compile**

The existing examples use `pbr_texture_bind_group_layout` (4 bindings). They should continue to work with the OLD layout — the new `pbr_full_texture_bind_group_layout` is a separate function. The shader needs to match whichever layout is used.

**Strategy:** Create a NEW shader variant `pbr_shader_full.wgsl` with ORM+emissive support, keeping the old shader untouched for backward compatibility. OR add a preprocessor approach.

**Simpler approach:** Just update `pbr_shader.wgsl` to include the new bindings AND update ALL examples that use it to use the full layout with default white/black textures. This avoids shader duplication.

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_renderer/src/pbr_shader.wgsl
git commit -m "feat(renderer): add ORM and emissive texture sampling to PBR shader"
```

---

### Task 3: Update Deferred G-Buffer Shader

**Files:**
- Modify: `crates/voltex_renderer/src/deferred_gbuffer.wgsl`
- Modify: `crates/voltex_renderer/src/deferred_lighting.wgsl`

- [ ] **Step 1: Add ORM/emissive bindings to G-Buffer shader**

Same pattern as forward shader — add bindings 4-7, sample ORM for metallic/roughness/ao, store in material_data output.

- [ ] **Step 2: Store emissive in G-Buffer**

Option: Pack emissive into existing G-Buffer outputs or add a 5th MRT.

**Recommended:** Store emissive in material_data.a (was padding=1.0). Simple, no extra MRT.

```wgsl
// G-Buffer output location(3): [metallic, roughness, ao, emissive_intensity]
// Emissive color stored as luminance for simplicity, or use location(2).a
out.material_data = vec4<f32>(metallic, roughness, ao, emissive_luminance);
```

Actually simpler: add emissive directly in the lighting pass as a texture sample pass-through. But that requires the emissive texture in the lighting pass too.

**Simplest approach:** Write emissive to albedo output additively, since emissive bypasses lighting.

```wgsl
// In deferred_gbuffer.wgsl:
let emissive = textureSample(t_emissive, s_emissive, in.uv).rgb;
// Store emissive luminance in material_data.w (was 1.0 padding)
let emissive_lum = dot(emissive, vec3<f32>(0.299, 0.587, 0.114));
out.material_data = vec4<f32>(metallic, roughness, ao, emissive_lum);
```

Then in `deferred_lighting.wgsl`, read `material_data.w` as emissive intensity and add `albedo * emissive_lum` to final color.

- [ ] **Step 3: Update deferred_lighting.wgsl**

```wgsl
let emissive_lum = textureSample(g_material, s_gbuffer, uv).w;
// After lighting calculation:
color += albedo * emissive_lum;
```

- [ ] **Step 4: Commit**

```bash
git add crates/voltex_renderer/src/deferred_gbuffer.wgsl crates/voltex_renderer/src/deferred_lighting.wgsl
git commit -m "feat(renderer): add ORM and emissive to deferred rendering pipeline"
```

---

### Task 4: Update Examples + Pipeline Creation

**Files:**
- Modify: `crates/voltex_renderer/src/pbr_pipeline.rs`
- Modify: `examples/pbr_demo/src/main.rs`
- Modify: `examples/ibl_demo/src/main.rs`
- Modify: `crates/voltex_renderer/src/deferred_pipeline.rs`

- [ ] **Step 1: Update examples to use full texture layout**

In each example that uses `pbr_texture_bind_group_layout`:
- Switch to `pbr_full_texture_bind_group_layout`
- Create default ORM texture (white_1x1 = ao=1, roughness=1, metallic=1 — but material multipliers control actual values)
- Create default emissive texture (black_1x1 = no emission)
- Pass all 4 texture pairs to `create_pbr_full_texture_bind_group`

- [ ] **Step 2: Update deferred_pipeline.rs**

Update gbuffer pipeline layout to use the full texture layout.

- [ ] **Step 3: Build and test**

```bash
cargo build --workspace
cargo test --workspace
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(renderer): update examples and pipelines for full PBR texture maps"
```
