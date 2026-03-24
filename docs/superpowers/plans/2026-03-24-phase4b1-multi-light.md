# Phase 4b-1: Multi-Light System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Directional, Point, Spot 3종류의 라이트를 최대 16개 동시 지원하여 PBR 셰이더에서 다중 광원 라이팅을 렌더링한다.

**Architecture:** 기존 단일 `LightUniform`을 `LightsUniform`으로 교체. GPU에 고정 크기 라이트 배열(MAX_LIGHTS=16)과 활성 라이트 수를 전달. PBR 셰이더에서 라이트 루프를 돌며 각 라이트 타입별로 radiance를 계산. Point Light는 거리 감쇠(inverse square + range clamp), Spot Light는 원뿔 각도 감쇠를 적용.

**Tech Stack:** Rust 1.94, wgpu 28.0, WGSL

---

## File Structure

```
crates/voltex_renderer/src/
├── light.rs            # LightData, LightsUniform 교체 (MODIFY)
├── pbr_shader.wgsl     # 다중 라이트 루프 (MODIFY)
├── pbr_pipeline.rs     # 기존 유지 (bind group 변경 없음 — light는 group(0) binding(1) 그대로)
├── lib.rs              # re-export 업데이트 (MODIFY)
examples/
└── multi_light_demo/   # 다중 라이트 데모 (NEW)
    ├── Cargo.toml
    └── src/
        └── main.rs
```

---

## Task 1: LightData + LightsUniform

**Files:**
- Modify: `crates/voltex_renderer/src/light.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

기존 `LightUniform`(단일 directional)을 유지하면서(하위 호환), 새 `LightData` + `LightsUniform`을 추가.

- [ ] **Step 1: light.rs에 새 타입 추가**

기존 CameraUniform, LightUniform은 유지 (기존 예제 호환). 새 타입 추가:

```rust
// crates/voltex_renderer/src/light.rs — 기존 코드 아래에 추가

pub const MAX_LIGHTS: usize = 16;

/// 라이트 타입 상수
pub const LIGHT_DIRECTIONAL: u32 = 0;
pub const LIGHT_POINT: u32 = 1;
pub const LIGHT_SPOT: u32 = 2;

/// 개별 라이트 데이터 (GPU 전달용)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightData {
    pub position: [f32; 3],
    pub light_type: u32,        // 0=directional, 1=point, 2=spot
    pub direction: [f32; 3],
    pub range: f32,             // point/spot: 최대 영향 거리
    pub color: [f32; 3],
    pub intensity: f32,
    pub inner_cone: f32,        // spot: 내부 원뿔 각도 (cos)
    pub outer_cone: f32,        // spot: 외부 원뿔 각도 (cos)
    pub _padding: [f32; 2],
}

impl LightData {
    pub fn directional(direction: [f32; 3], color: [f32; 3], intensity: f32) -> Self {
        Self {
            position: [0.0; 3],
            light_type: LIGHT_DIRECTIONAL,
            direction,
            range: 0.0,
            color,
            intensity,
            inner_cone: 0.0,
            outer_cone: 0.0,
            _padding: [0.0; 2],
        }
    }

    pub fn point(position: [f32; 3], color: [f32; 3], intensity: f32, range: f32) -> Self {
        Self {
            position,
            light_type: LIGHT_POINT,
            direction: [0.0; 3],
            range,
            color,
            intensity,
            inner_cone: 0.0,
            outer_cone: 0.0,
            _padding: [0.0; 2],
        }
    }

    pub fn spot(
        position: [f32; 3],
        direction: [f32; 3],
        color: [f32; 3],
        intensity: f32,
        range: f32,
        inner_angle_deg: f32,
        outer_angle_deg: f32,
    ) -> Self {
        Self {
            position,
            light_type: LIGHT_SPOT,
            direction,
            range,
            color,
            intensity,
            inner_cone: inner_angle_deg.to_radians().cos(),
            outer_cone: outer_angle_deg.to_radians().cos(),
            _padding: [0.0; 2],
        }
    }
}

/// 다중 라이트 uniform (고정 크기 배열)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightsUniform {
    pub lights: [LightData; MAX_LIGHTS],
    pub count: u32,
    pub ambient_color: [f32; 3],
}

impl LightsUniform {
    pub fn new() -> Self {
        Self {
            lights: [LightData::directional([0.0, -1.0, 0.0], [1.0; 3], 1.0); MAX_LIGHTS],
            count: 0,
            ambient_color: [0.03, 0.03, 0.03],
        }
    }

    pub fn add_light(&mut self, light: LightData) {
        if (self.count as usize) < MAX_LIGHTS {
            self.lights[self.count as usize] = light;
            self.count += 1;
        }
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_data_size() {
        // LightData must be 16-byte aligned for WGSL array
        assert_eq!(std::mem::size_of::<LightData>() % 16, 0);
    }

    #[test]
    fn test_lights_uniform_add() {
        let mut lights = LightsUniform::new();
        lights.add_light(LightData::point([0.0, 5.0, 0.0], [1.0, 0.0, 0.0], 10.0, 20.0));
        lights.add_light(LightData::directional([0.0, -1.0, 0.0], [1.0, 1.0, 1.0], 1.0));
        assert_eq!(lights.count, 2);
    }

    #[test]
    fn test_lights_uniform_max() {
        let mut lights = LightsUniform::new();
        for i in 0..20 {
            lights.add_light(LightData::point([i as f32, 0.0, 0.0], [1.0; 3], 1.0, 10.0));
        }
        assert_eq!(lights.count, MAX_LIGHTS as u32); // capped at 16
    }

    #[test]
    fn test_spot_light_cone() {
        let spot = LightData::spot(
            [0.0; 3], [0.0, -1.0, 0.0],
            [1.0; 3], 10.0, 20.0,
            15.0, 30.0,
        );
        // cos(15°) ≈ 0.9659, cos(30°) ≈ 0.8660
        assert!((spot.inner_cone - 15.0_f32.to_radians().cos()).abs() < 1e-4);
        assert!((spot.outer_cone - 30.0_f32.to_radians().cos()).abs() < 1e-4);
    }
}
```

- [ ] **Step 2: lib.rs 업데이트**

기존 re-export에 추가:

```rust
pub use light::{LightData, LightsUniform, MAX_LIGHTS, LIGHT_DIRECTIONAL, LIGHT_POINT, LIGHT_SPOT};
```

- [ ] **Step 3: 테스트 통과 확인**

Run: `cargo test -p voltex_renderer`
Expected: 기존 13 + light 4 = 17개 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): add LightData and LightsUniform for multi-light support"
```

---

## Task 2: PBR 셰이더 다중 라이트

**Files:**
- Modify: `crates/voltex_renderer/src/pbr_shader.wgsl`

기존 단일 directional light 로직을 다중 라이트 루프로 교체. 라이트 타입별 radiance 계산.

- [ ] **Step 1: pbr_shader.wgsl 교체**

```wgsl
// crates/voltex_renderer/src/pbr_shader.wgsl
const PI: f32 = 3.14159265358979;
const MAX_LIGHTS: u32 = 16u;
const LIGHT_DIRECTIONAL: u32 = 0u;
const LIGHT_POINT: u32 = 1u;
const LIGHT_SPOT: u32 = 2u;

struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec3<f32>,
};

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

struct MaterialUniform {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> lights_uniform: LightsUniform;

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

@group(2) @binding(0) var<uniform> material: MaterialUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(model_v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = camera.model * vec4<f32>(model_v.position, 1.0);
    out.world_pos = world_pos.xyz;
    out.world_normal = normalize((camera.model * vec4<f32>(model_v.normal, 0.0)).xyz);
    out.clip_position = camera.view_proj * world_pos;
    out.uv = model_v.uv;
    return out;
}

// --- PBR Functions ---

fn distribution_ggx(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;
    let denom_inner = NdotH2 * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom_inner * denom_inner);
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

// --- Light attenuation ---

fn attenuation_point(distance: f32, range: f32) -> f32 {
    let att = 1.0 / (distance * distance + 0.0001);
    // Smooth range falloff
    let falloff = clamp(1.0 - pow(distance / range, 4.0), 0.0, 1.0);
    return att * falloff * falloff;
}

fn attenuation_spot(light: LightData, L: vec3<f32>) -> f32 {
    let theta = dot(normalize(light.direction), -L);
    let epsilon = light.inner_cone - light.outer_cone;
    return clamp((theta - light.outer_cone) / epsilon, 0.0, 1.0);
}

// --- Per-light radiance ---

fn compute_light_contribution(
    light: LightData,
    N: vec3<f32>,
    V: vec3<f32>,
    world_pos: vec3<f32>,
    F0: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
) -> vec3<f32> {
    var L: vec3<f32>;
    var radiance: vec3<f32>;

    if light.light_type == LIGHT_DIRECTIONAL {
        L = normalize(-light.direction);
        radiance = light.color * light.intensity;
    } else {
        // Point or Spot
        let to_light = light.position - world_pos;
        let distance = length(to_light);
        L = to_light / distance;
        let att = attenuation_point(distance, light.range);
        radiance = light.color * light.intensity * att;

        if light.light_type == LIGHT_SPOT {
            radiance = radiance * attenuation_spot(light, L);
        }
    }

    let H = normalize(V + L);
    let NdotL = max(dot(N, L), 0.0);

    if NdotL <= 0.0 {
        return vec3<f32>(0.0);
    }

    let NDF = distribution_ggx(N, H, roughness);
    let G = geometry_smith(N, V, L, roughness);
    let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

    let ks = F;
    let kd = (vec3<f32>(1.0) - ks) * (1.0 - metallic);

    let numerator = NDF * G * F;
    let NdotV = max(dot(N, V), 0.0);
    let denominator = 4.0 * NdotV * NdotL + 0.0001;
    let specular = numerator / denominator;

    return (kd * albedo / PI + specular) * radiance * NdotL;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    let albedo = material.base_color.rgb * tex_color.rgb;
    let metallic = material.metallic;
    let roughness = material.roughness;
    let ao = material.ao;

    let N = normalize(in.world_normal);
    let V = normalize(camera.camera_pos - in.world_pos);

    let F0 = mix(vec3<f32>(0.04, 0.04, 0.04), albedo, metallic);

    // Accumulate light contributions
    var Lo = vec3<f32>(0.0);
    let light_count = min(lights_uniform.count, MAX_LIGHTS);
    for (var i = 0u; i < light_count; i = i + 1u) {
        Lo = Lo + compute_light_contribution(
            lights_uniform.lights[i], N, V, in.world_pos,
            F0, albedo, metallic, roughness,
        );
    }

    // Ambient
    let ambient = lights_uniform.ambient_color * albedo * ao;

    var color = ambient + Lo;

    // Reinhard tone mapping
    color = color / (color + vec3<f32>(1.0));

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, material.base_color.a * tex_color.a);
}
```

- [ ] **Step 2: 빌드 확인**

Run: `cargo build -p voltex_renderer`
Expected: 빌드 성공

중요: PBR 셰이더의 light uniform이 바뀌었으므로, 기존 pbr_demo는 **컴파일은 되지만** 런타임에 bind group 크기가 맞지 않아 크래시할 수 있다. pbr_demo는 Task 3에서 업데이트.

- [ ] **Step 3: 커밋**

```bash
git add crates/voltex_renderer/src/pbr_shader.wgsl
git commit -m "feat(renderer): update PBR shader for multi-light with point and spot support"
```

---

## Task 3: multi_light_demo + pbr_demo 수정

**Files:**
- Create: `examples/multi_light_demo/Cargo.toml`
- Create: `examples/multi_light_demo/src/main.rs`
- Modify: `examples/pbr_demo/src/main.rs` (LightsUniform 사용으로 업데이트)
- Modify: `Cargo.toml` (워크스페이스에 multi_light_demo 추가)

### pbr_demo 수정

기존 pbr_demo가 `LightUniform`을 사용하고 있으므로 `LightsUniform`으로 교체해야 한다. 변경 최소화:
- `LightUniform::new()` → `LightsUniform::new()` + `add_light(LightData::directional(...))`
- light_buffer 크기가 `LightsUniform` 크기로 변경
- 나머지 로직 동일

### multi_light_demo

PBR 구체 여러 개 + 다양한 색상의 Point/Spot Light로 다중 라이트를 데모.

장면 구성:
- 바닥: 큰 큐브(scale 10x0.1x10)를 y=-0.5에 배치 (roughness 0.8, 비금속)
- 구체 5개: 일렬 배치, 다양한 metallic/roughness
- Point Light 4개: 빨강, 초록, 파랑, 노랑 — 구체 위에서 원형으로 공전
- Directional Light 1개: 약한 하얀빛 (전체 조명)
- Spot Light 1개: 바닥 중앙을 비추는 흰색

카메라: (0, 5, 10) pitch=-0.3

동적 라이트: 매 프레임 Point Light 위치를 time 기반으로 원형 궤도에서 업데이트.

dynamic UBO 패턴 사용 (many_cubes 기반). LightsUniform은 매 프레임 write_buffer로 갱신 (static, dynamic offset 없음).

파일을 작성하기 전에 반드시 읽어야 할 파일:
1. `examples/pbr_demo/src/main.rs` — 수정 대상
2. `examples/many_cubes/src/main.rs` — dynamic UBO 패턴
3. `crates/voltex_renderer/src/light.rs` — LightData, LightsUniform API
4. `crates/voltex_renderer/src/material.rs` — MaterialUniform API
5. `crates/voltex_renderer/src/sphere.rs` — generate_sphere

- [ ] **Step 1: pbr_demo/main.rs 수정**

핵심 변경:
- `use voltex_renderer::LightUniform` → `use voltex_renderer::{LightsUniform, LightData}`
- light_uniform 초기화: `LightsUniform::new()` + `add_light(LightData::directional([-1.0, -1.0, -1.0], [1.0,1.0,1.0], 1.0))`
- light_buffer 크기: `std::mem::size_of::<LightsUniform>()`
- write_buffer에 `bytemuck::cast_slice(&[lights_uniform])`

- [ ] **Step 2: multi_light_demo 작성**

워크스페이스에 추가, Cargo.toml 작성, main.rs 작성.

구체 + 바닥 = 6개 엔티티 (간단하므로 ECS 없이 직접 관리). dynamic UBO for camera (per-entity) + material (per-entity). LightsUniform은 static (per-frame update).

- [ ] **Step 3: 빌드 + 테스트**

Run: `cargo build --workspace`
Run: `cargo test --workspace`

- [ ] **Step 4: 실행 확인**

Run: `cargo run -p pbr_demo` — 여전히 동작 (단일 directional light)
Run: `cargo run -p multi_light_demo` — 다중 색상 라이트가 구체들을 비추며 공전

- [ ] **Step 5: 커밋**

```bash
git add Cargo.toml examples/pbr_demo/ examples/multi_light_demo/
git commit -m "feat: add multi-light demo with point/spot lights, update pbr_demo for LightsUniform"
```

---

## Phase 4b-1 완료 기준 체크리스트

- [ ] `cargo build --workspace` 성공
- [ ] `cargo test --workspace` — 모든 테스트 통과
- [ ] LightData: Directional, Point, Spot 3가지 타입 생성자
- [ ] LightsUniform: 최대 16개 라이트 배열, add/clear
- [ ] PBR 셰이더: 라이트 루프, Point attenuation, Spot cone falloff
- [ ] `cargo run -p pbr_demo` — 기존 기능 유지 (단일 directional)
- [ ] `cargo run -p multi_light_demo` — 다중 색상 Point Light 공전, Spot Light
- [ ] 기존 예제 모두 동작 (mesh_shader.wgsl 사용하는 것들은 변경 없음)
