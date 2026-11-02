// Skinned PBR shader: GPU vertex skinning + the full Cook-Torrance fragment
// from pbr_shader.wgsl. The bone palette is folded into group 0 (alongside
// camera + lights) so the pipeline stays within max_bind_groups=4:
//   group(0): camera (dyn) + lights (static) + bone palette (dyn)
//   group(1): albedo/normal/ORM/emissive textures
//   group(2): material
//   group(3): shadow depth + comparison sampler + ShadowUniform + BRDF LUT
// The fragment is identical to the non-skinned PBR shader.

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

struct BonePalette {
    bones: array<mat4x4<f32>, 128>,
};

struct MaterialUniform {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> lights_uniform: LightsUniform;
@group(0) @binding(2) var<uniform> palette: BonePalette;

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;
@group(1) @binding(2) var t_normal: texture_2d<f32>;
@group(1) @binding(3) var s_normal: sampler;
@group(1) @binding(4) var t_orm: texture_2d<f32>;
@group(1) @binding(5) var s_orm: sampler;
@group(1) @binding(6) var t_emissive: texture_2d<f32>;
@group(1) @binding(7) var s_emissive: sampler;

@group(2) @binding(0) var<uniform> material: MaterialUniform;

struct ShadowUniform {
    light_view_proj: mat4x4<f32>,
    shadow_map_size: f32,
    shadow_bias: f32,
    _padding: vec2<f32>,
    sun_direction: vec3<f32>,
    turbidity: f32,
    sh_coefficients: array<vec4<f32>, 7>,
};

@group(3) @binding(0) var t_shadow: texture_depth_2d;
@group(3) @binding(1) var s_shadow: sampler_comparison;
@group(3) @binding(2) var<uniform> shadow: ShadowUniform;
@group(3) @binding(3) var t_brdf_lut: texture_2d<f32>;
@group(3) @binding(4) var s_brdf_lut: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
    @location(4) joints: vec4<u32>,
    @location(5) weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) light_space_pos: vec4<f32>,
    @location(4) world_tangent: vec3<f32>,
    @location(5) world_bitangent: vec3<f32>,
};

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    // Weighted blend of the four influencing bone matrices (CPU palette).
    let skin = palette.bones[v.joints.x] * v.weights.x
             + palette.bones[v.joints.y] * v.weights.y
             + palette.bones[v.joints.z] * v.weights.z
             + palette.bones[v.joints.w] * v.weights.w;

    let skinned_pos = skin * vec4<f32>(v.position, 1.0);
    let world_pos = camera.model * skinned_pos;

    var out: VertexOutput;
    out.world_pos = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.uv = v.uv;
    out.light_space_pos = shadow.light_view_proj * world_pos;

    // Skin + model the normal/tangent for correct TBN under deformation.
    let model_skin = camera.model * skin;
    let N_out = normalize((model_skin * vec4<f32>(v.normal, 0.0)).xyz);
    let T = normalize((model_skin * vec4<f32>(v.tangent.xyz, 0.0)).xyz);
    let B = cross(N_out, T) * v.tangent.w;
    out.world_normal = N_out;
    out.world_tangent = T;
    out.world_bitangent = B;
    return out;
}

// ── Cook-Torrance helpers (identical to pbr_shader.wgsl) ────────────────────

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
    let ggx1 = geometry_schlick_ggx(NdotV, roughness);
    let ggx2 = geometry_schlick_ggx(NdotL, roughness);
    return ggx1 * ggx2;
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
        0.0,
        1.0,
    );
}

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
        L = normalize(-light.direction);
        radiance = light.color * light.intensity;
    } else if light.light_type == 1u {
        let to_light = light.position - world_pos;
        let dist = length(to_light);
        L = normalize(to_light);
        let att = attenuation_point(dist, light.range);
        radiance = light.color * light.intensity * att;
    } else {
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

fn calculate_shadow(light_space_pos: vec4<f32>) -> f32 {
    if shadow.shadow_map_size == 0.0 {
        return 1.0;
    }
    let proj_coords = light_space_pos.xyz / light_space_pos.w;
    let shadow_uv = vec2<f32>(proj_coords.x * 0.5 + 0.5, -proj_coords.y * 0.5 + 0.5);
    let current_depth = proj_coords.z;
    if shadow_uv.x < 0.0 || shadow_uv.x > 1.0 || shadow_uv.y < 0.0 || shadow_uv.y > 1.0 {
        return 1.0;
    }
    if current_depth > 1.0 || current_depth < 0.0 {
        return 1.0;
    }
    let texel_size = 1.0 / shadow.shadow_map_size;
    var shadow_val = 0.0;
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow_val += textureSampleCompare(
                t_shadow, s_shadow, shadow_uv + offset, current_depth - shadow.shadow_bias,
            );
        }
    }
    return shadow_val / 9.0;
}

fn sample_environment(direction: vec3<f32>, roughness: f32) -> vec3<f32> {
    let sun_dir = normalize(shadow.sun_direction);
    let turb = clamp(shadow.turbidity, 1.5, 10.0);
    var env: vec3<f32>;
    if direction.y > 0.0 {
        let zenith_color = vec3<f32>(0.15, 0.3, 0.8) * (1.0 / (turb * 0.15 + 0.5));
        let horizon_color = vec3<f32>(0.7, 0.6, 0.5) * (1.0 + turb * 0.04);
        let elevation = direction.y;
        let sky_gradient = mix(horizon_color, zenith_color, pow(elevation, 0.4));
        let cos_sun = max(dot(direction, sun_dir), 0.0);
        let mie_strength = turb * 0.02;
        let mie = mie_strength * pow(cos_sun, 8.0) * vec3<f32>(1.0, 0.9, 0.7);
        let sun_disk = pow(max(cos_sun, 0.0), 2048.0) * vec3<f32>(10.0, 9.0, 7.0);
        env = sky_gradient + mie + sun_disk;
    } else {
        let horizon_color = vec3<f32>(0.6, 0.55, 0.45);
        let ground_color = vec3<f32>(0.1, 0.08, 0.06);
        env = mix(horizon_color, ground_color, pow(-direction.y, 0.4));
    }
    let avg = vec3<f32>(0.3, 0.35, 0.4);
    return mix(env, avg, roughness * roughness);
}

fn evaluate_sh(normal: vec3<f32>, coeffs: array<vec4<f32>, 7>) -> vec3<f32> {
    let x = normal.x;
    let y = normal.y;
    let z = normal.z;
    let Y00  = 0.282095;
    let Y1n1 = 0.488603 * y;
    let Y10  = 0.488603 * z;
    let Y1p1 = 0.488603 * x;
    let Y2n2 = 1.092548 * x * y;
    let Y2n1 = 1.092548 * y * z;
    let Y20  = 0.315392 * (3.0 * z * z - 1.0);
    let Y2p1 = 1.092548 * x * z;
    let Y2p2 = 0.546274 * (x * x - y * y);
    let c0 = vec3<f32>(coeffs[0].x, coeffs[0].y, coeffs[0].z);
    let c1 = vec3<f32>(coeffs[0].w, coeffs[1].x, coeffs[1].y);
    let c2 = vec3<f32>(coeffs[1].z, coeffs[1].w, coeffs[2].x);
    let c3 = vec3<f32>(coeffs[2].y, coeffs[2].z, coeffs[2].w);
    let c4 = vec3<f32>(coeffs[3].x, coeffs[3].y, coeffs[3].z);
    let c5 = vec3<f32>(coeffs[3].w, coeffs[4].x, coeffs[4].y);
    let c6 = vec3<f32>(coeffs[4].z, coeffs[4].w, coeffs[5].x);
    let c7 = vec3<f32>(coeffs[5].y, coeffs[5].z, coeffs[5].w);
    let c8 = vec3<f32>(coeffs[6].x, coeffs[6].y, coeffs[6].z);
    return max(
        c0 * Y00 + c1 * Y1n1 + c2 * Y10 + c3 * Y1p1 +
        c4 * Y2n2 + c5 * Y2n1 + c6 * Y20 + c7 * Y2p1 + c8 * Y2p2,
        vec3<f32>(0.0)
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    let albedo = material.base_color.rgb * tex_color.rgb;

    let orm_sample = textureSample(t_orm, s_orm, in.uv);
    let ao = orm_sample.r * material.ao;
    let roughness = orm_sample.g * material.roughness;
    let metallic = orm_sample.b * material.metallic;

    let emissive = textureSample(t_emissive, s_emissive, in.uv).rgb;

    let T = normalize(in.world_tangent);
    let B = normalize(in.world_bitangent);
    let N_geom = normalize(in.world_normal);
    let normal_sample = textureSample(t_normal, s_normal, in.uv).rgb;
    let tangent_normal = normal_sample * 2.0 - 1.0;
    let TBN = mat3x3<f32>(T, B, N_geom);
    let N = normalize(TBN * tangent_normal);

    let V = normalize(camera.camera_pos - in.world_pos);
    let F0 = mix(vec3<f32>(0.04, 0.04, 0.04), albedo, metallic);

    let shadow_factor = calculate_shadow(in.light_space_pos);
    var Lo = vec3<f32>(0.0);
    let light_count = min(lights_uniform.count, 16u);
    for (var i = 0u; i < light_count; i++) {
        var contribution = compute_light_contribution(
            lights_uniform.lights[i], N, V, in.world_pos, F0, albedo, metallic, roughness,
        );
        if lights_uniform.lights[i].light_type == 0u {
            contribution = contribution * shadow_factor;
        }
        Lo += contribution;
    }

    let NdotV_ibl = max(dot(N, V), 0.0);
    let R = reflect(-V, N);
    var irradiance: vec3<f32>;
    let sh_test = shadow.sh_coefficients[0].x + shadow.sh_coefficients[0].y + shadow.sh_coefficients[0].z;
    if abs(sh_test) > 0.0001 {
        irradiance = evaluate_sh(N, shadow.sh_coefficients);
    } else {
        irradiance = sample_environment(N, 1.0);
    }
    let F_env = fresnel_schlick(NdotV_ibl, F0);
    let kd_ibl = (vec3<f32>(1.0) - F_env) * (1.0 - metallic);
    let diffuse_ibl = kd_ibl * albedo * irradiance;

    let prefiltered = sample_environment(R, roughness);
    let brdf_val = textureSample(t_brdf_lut, s_brdf_lut, vec2<f32>(NdotV_ibl, roughness));
    let specular_ibl = prefiltered * (F0 * brdf_val.r + vec3<f32>(brdf_val.g));

    let ambient = (diffuse_ibl + specular_ibl) * ao;
    var color = ambient + Lo + emissive;
    color = color / (color + vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(color, material.base_color.a * tex_color.a);
}
