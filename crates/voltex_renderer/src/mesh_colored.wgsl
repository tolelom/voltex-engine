// BasicRenderer's Blinn-Phong shader with a per-entity `base_color` tint.
// Identical to mesh_shader.wgsl except the camera uniform carries base_color
// (fed from each entity's MaterialUniform) and the fragment multiplies by it,
// so meshes can be colored without a per-object texture.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec3<f32>,
    base_color: vec4<f32>,
};

struct LightUniform {
    direction: vec3<f32>,
    color: vec3<f32>,
    ambient_strength: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> light: LightUniform;

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

// Two vec4 (pos+range, color+intensity) to avoid the std140 vec3 packing
// ambiguity — this matches the Rust `PointLight`'s contiguous [f32;3]+f32 fields.
struct PointLight {
    pos_range: vec4<f32>,        // xyz = position, w = range
    color_intensity: vec4<f32>,  // rgb = color, w = intensity
};
struct PointLights {
    lights: array<PointLight, 8>,
    count: u32,
};
@group(2) @binding(0) var<uniform> point_lights: PointLights;

struct ShadowMain {
    light_view_proj: mat4x4<f32>,
    shadow_map_size: f32,
    shadow_bias: f32,
};
@group(3) @binding(0) var t_shadow: texture_depth_2d;
@group(3) @binding(1) var s_shadow: sampler_comparison;
@group(3) @binding(2) var<uniform> shadow: ShadowMain;

// 1.0 = fully lit, 0.0 = fully shadowed. Returns 1.0 when shadows are disabled
// (shadow_map_size == 0) or the fragment is outside the shadow frustum.
fn shadow_factor(light_space_pos: vec4<f32>) -> f32 {
    if (shadow.shadow_map_size == 0.0) {
        return 1.0;
    }
    let proj = light_space_pos.xyz / light_space_pos.w;
    let uv = vec2<f32>(proj.x * 0.5 + 0.5, -proj.y * 0.5 + 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || proj.z > 1.0) {
        return 1.0;
    }
    let texel = 1.0 / shadow.shadow_map_size;
    var sum = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let off = vec2<f32>(f32(dx), f32(dy)) * texel;
            sum = sum + textureSampleCompare(t_shadow, s_shadow, uv + off, proj.z - shadow.shadow_bias);
        }
    }
    return sum / 9.0;
}

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
    @location(3) light_space_pos: vec4<f32>,
};

@vertex
fn vs_main(model_v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = camera.model * vec4<f32>(model_v.position, 1.0);
    out.world_pos = world_pos.xyz;
    out.world_normal = (camera.model * vec4<f32>(model_v.normal, 0.0)).xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.uv = model_v.uv;
    out.light_space_pos = shadow.light_view_proj * world_pos;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv) * camera.base_color;
    let normal = normalize(in.world_normal);
    let light_dir = normalize(-light.direction);
    let ambient = light.ambient_strength * light.color;
    let diff = max(dot(normal, light_dir), 0.0);
    let diffuse = diff * light.color;
    let view_dir = normalize(camera.camera_pos - in.world_pos);
    let half_dir = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, half_dir), 0.0), 32.0);
    let specular = spec * light.color * 0.5;

    // Additive point lights (smooth distance falloff).
    var point_contrib = vec3<f32>(0.0, 0.0, 0.0);
    for (var i: u32 = 0u; i < point_lights.count; i = i + 1u) {
        let pl = point_lights.lights[i];
        let range = pl.pos_range.w;
        let to_light = pl.pos_range.xyz - in.world_pos;
        let dist = length(to_light);
        if (range > 0.0 && dist < range) {
            let l = to_light / max(dist, 1e-4);
            let atten = 1.0 - dist / range;
            let d = max(dot(normal, l), 0.0) * atten * atten * pl.color_intensity.w;
            point_contrib = point_contrib + d * pl.color_intensity.xyz;
        }
    }

    // Directional light (diffuse + specular) is occluded by the shadow map;
    // ambient and point lights are not.
    let s = shadow_factor(in.light_space_pos);
    let result = (ambient + (diffuse + specular) * s + point_contrib) * tex_color.rgb;
    return vec4<f32>(result, tex_color.a);
}
