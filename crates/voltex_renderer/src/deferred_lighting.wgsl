// Deferred lighting pass shader.
// Reads G-Buffer textures and computes Cook-Torrance PBR shading.

// ── G-Buffer inputs ──────────────────────────────────────────────────────────

@group(0) @binding(0) var t_position: texture_2d<f32>;
@group(0) @binding(1) var t_normal: texture_2d<f32>;
@group(0) @binding(2) var t_albedo: texture_2d<f32>;
@group(0) @binding(3) var t_material: texture_2d<f32>;
@group(0) @binding(4) var s_gbuffer: sampler;

// ── Lights ───────────────────────────────────────────────────────────────────

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
@group(1) @binding(1) var<uniform> camera_uniform: CameraPositionUniform;

// ── Shadow + IBL ─────────────────────────────────────────────────────────────

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
@group(2) @binding(5) var t_ssgi: texture_2d<f32>;
@group(2) @binding(6) var s_ssgi: sampler;
@group(2) @binding(7) var t_rt_shadow: texture_2d<f32>;
@group(2) @binding(8) var s_rt_shadow: sampler;

// ── Vertex / Fragment structs ─────────────────────────────────────────────────

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(v.position, 0.0, 1.0);
    out.uv = vec2<f32>(v.position.x * 0.5 + 0.5, 1.0 - (v.position.y * 0.5 + 0.5));
    return out;
}

// ── BRDF functions (identical to pbr_shader.wgsl) ────────────────────────────

// GGX Normal Distribution Function
fn distribution_ggx(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;
    let denom_inner = NdotH2 * (a2 - 1.0) + 1.0;
    let denom = 3.14159265358979 * denom_inner * denom_inner;
    return a2 / denom;
}

// Schlick-GGX geometry function (single direction)
fn geometry_schlick_ggx(NdotV: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return NdotV / (NdotV * (1.0 - k) + k);
}

// Smith geometry function (both directions)
fn geometry_smith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx1 = geometry_schlick_ggx(NdotV, roughness);
    let ggx2 = geometry_schlick_ggx(NdotL, roughness);
    return ggx1 * ggx2;
}

// Fresnel-Schlick approximation
fn fresnel_schlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

// Point light distance attenuation: inverse-square with smooth falloff at range boundary
fn attenuation_point(distance: f32, range: f32) -> f32 {
    let d_over_r = distance / range;
    let d_over_r4 = d_over_r * d_over_r * d_over_r * d_over_r;
    let falloff = clamp(1.0 - d_over_r4, 0.0, 1.0);
    return (falloff * falloff) / (distance * distance + 0.0001);
}

// Spot light angular attenuation
fn attenuation_spot(light: LightData, L: vec3<f32>) -> f32 {
    let spot_dir = normalize(light.direction);
    let theta = dot(spot_dir, -L);
    return clamp(
        (theta - light.outer_cone) / (light.inner_cone - light.outer_cone + 0.0001),
        0.0,
        1.0,
    );
}

// Cook-Torrance BRDF contribution for one light
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

    if light.light_type == 0u {
        // Directional
        L = normalize(-light.direction);
        radiance = light.color * light.intensity;
    } else if light.light_type == 1u {
        // Point
        let to_light = light.position - world_pos;
        let dist = length(to_light);
        L = normalize(to_light);
        let att = attenuation_point(dist, light.range);
        radiance = light.color * light.intensity * att;
    } else {
        // Spot
        let to_light = light.position - world_pos;
        let dist = length(to_light);
        L = normalize(to_light);
        let att_dist = attenuation_point(dist, light.range);
        let att_ang = attenuation_spot(light, L);
        radiance = light.color * light.intensity * att_dist * att_ang;
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
    // If shadow_map_size == 0, shadow is disabled
    if shadow.shadow_map_size == 0.0 {
        return 1.0;
    }

    let light_space_pos = shadow.light_view_proj * vec4<f32>(world_pos, 1.0);
    let proj_coords = light_space_pos.xyz / light_space_pos.w;

    // wgpu NDC: x,y [-1,1], z [0,1]
    let shadow_uv = vec2<f32>(
        proj_coords.x * 0.5 + 0.5,
        -proj_coords.y * 0.5 + 0.5,
    );
    let current_depth = proj_coords.z;

    if shadow_uv.x < 0.0 || shadow_uv.x > 1.0 || shadow_uv.y < 0.0 || shadow_uv.y > 1.0 {
        return 1.0;
    }
    if current_depth > 1.0 || current_depth < 0.0 {
        return 1.0;
    }

    // 3x3 PCF
    let texel_size = 1.0 / shadow.shadow_map_size;
    var shadow_val = 0.0;
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow_val += textureSampleCompare(
                t_shadow, s_shadow,
                shadow_uv + offset,
                current_depth - shadow.shadow_bias,
            );
        }
    }
    return shadow_val / 9.0;
}

// Procedural environment sampling for IBL
fn sample_environment(direction: vec3<f32>, roughness: f32) -> vec3<f32> {
    var env: vec3<f32>;
    if direction.y > 0.0 {
        let horizon = vec3<f32>(0.6, 0.6, 0.5);
        let sky = vec3<f32>(0.3, 0.5, 0.9);
        env = mix(horizon, sky, pow(direction.y, 0.4));
    } else {
        let horizon = vec3<f32>(0.6, 0.6, 0.5);
        let ground = vec3<f32>(0.1, 0.08, 0.06);
        env = mix(horizon, ground, pow(-direction.y, 0.4));
    }
    let avg = vec3<f32>(0.3, 0.35, 0.4);
    return mix(env, avg, roughness * roughness);
}

// ── Fragment shader ──────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Read G-Buffer
    let position_sample = textureSample(t_position, s_gbuffer, uv);
    let world_pos = position_sample.xyz;

    // Background pixel: skip shading
    if length(world_pos) < 0.001 {
        return vec4<f32>(0.01, 0.01, 0.01, 1.0);
    }

    let normal_sample = textureSample(t_normal, s_gbuffer, uv).rgb;
    let N = normalize(normal_sample * 2.0 - 1.0);

    let albedo_sample = textureSample(t_albedo, s_gbuffer, uv);
    let albedo = albedo_sample.rgb;
    let alpha = albedo_sample.a;

    let mat_sample = textureSample(t_material, s_gbuffer, uv);
    let metallic = mat_sample.r;
    let roughness = mat_sample.g;
    let ao = mat_sample.b;

    let V = normalize(camera_uniform.camera_pos - world_pos);

    // F0: base reflectivity; 0.04 for dielectrics, albedo for metals
    let F0 = mix(vec3<f32>(0.04, 0.04, 0.04), albedo, metallic);

    // Shadow: sample RT shadow texture (1.0 = lit, 0.0 = shadowed)
    let rt_dims = textureDimensions(t_rt_shadow);
    let rt_coord = vec2<i32>(vec2<f32>(uv.x * f32(rt_dims.x), uv.y * f32(rt_dims.y)));
    let shadow_factor = textureLoad(t_rt_shadow, rt_coord, 0).r;

    // Accumulate contribution from all active lights
    var Lo = vec3<f32>(0.0);
    let light_count = min(lights_uniform.count, 16u);
    for (var i = 0u; i < light_count; i++) {
        var contribution = compute_light_contribution(
            lights_uniform.lights[i],
            N, V, world_pos, F0, albedo, metallic, roughness,
        );
        if lights_uniform.lights[i].light_type == 0u {
            contribution = contribution * shadow_factor;
        }
        Lo += contribution;
    }

    // IBL ambient term
    let NdotV_ibl = max(dot(N, V), 0.0);
    let R = reflect(-V, N);

    // Diffuse IBL
    let irradiance = sample_environment(N, 1.0);
    let F_env = fresnel_schlick(NdotV_ibl, F0);
    let kd_ibl = (vec3<f32>(1.0) - F_env) * (1.0 - metallic);
    let diffuse_ibl = kd_ibl * albedo * irradiance;

    // Specular IBL
    let prefiltered = sample_environment(R, roughness);
    let brdf_val = textureSample(t_brdf_lut, s_brdf_lut, vec2<f32>(NdotV_ibl, roughness));
    let specular_ibl = prefiltered * (F0 * brdf_val.r + vec3<f32>(brdf_val.g));

    let ssgi_data = textureSample(t_ssgi, s_ssgi, uv);
    let ssgi_ao = ssgi_data.r;
    let ssgi_indirect = ssgi_data.gba;
    let ambient = (diffuse_ibl + specular_ibl) * ao * ssgi_ao + ssgi_indirect;

    // Output raw HDR linear colour; tonemap is applied in a separate tonemap pass.
    let color = ambient + Lo;

    return vec4<f32>(color, alpha);
}
