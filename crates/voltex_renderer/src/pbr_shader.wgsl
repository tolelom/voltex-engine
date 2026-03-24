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
@group(1) @binding(2) var t_normal: texture_2d<f32>;
@group(1) @binding(3) var s_normal: sampler;

@group(2) @binding(0) var<uniform> material: MaterialUniform;

struct ShadowUniform {
    light_view_proj: mat4x4<f32>,
    shadow_map_size: f32,
    shadow_bias: f32,
};

@group(3) @binding(0) var t_shadow: texture_depth_2d;
@group(3) @binding(1) var s_shadow: sampler_comparison;
@group(3) @binding(2) var<uniform> shadow: ShadowUniform;

@group(4) @binding(0) var t_brdf_lut: texture_2d<f32>;
@group(4) @binding(1) var s_brdf_lut: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
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
fn vs_main(model_v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = camera.model * vec4<f32>(model_v.position, 1.0);
    out.world_pos = world_pos.xyz;
    out.world_normal = (camera.model * vec4<f32>(model_v.normal, 0.0)).xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.uv = model_v.uv;
    out.light_space_pos = shadow.light_view_proj * world_pos;

    let T = normalize((camera.model * vec4<f32>(model_v.tangent.xyz, 0.0)).xyz);
    let N_out = normalize((camera.model * vec4<f32>(model_v.normal, 0.0)).xyz);
    let B = cross(N_out, T) * model_v.tangent.w;
    out.world_tangent = T;
    out.world_bitangent = B;

    return out;
}

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

fn calculate_shadow(light_space_pos: vec4<f32>) -> f32 {
    // If shadow_map_size == 0, shadow is disabled
    if shadow.shadow_map_size == 0.0 {
        return 1.0;
    }

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
    let t = direction.y * 0.5 + 0.5;
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    let albedo = material.base_color.rgb * tex_color.rgb;
    let metallic = material.metallic;
    let roughness = material.roughness;
    let ao = material.ao;

    // Normal mapping via TBN matrix
    let T = normalize(in.world_tangent);
    let B = normalize(in.world_bitangent);
    let N_geom = normalize(in.world_normal);

    // Sample normal map (tangent space normal)
    let normal_sample = textureSample(t_normal, s_normal, in.uv).rgb;
    let tangent_normal = normal_sample * 2.0 - 1.0;

    // TBN matrix transforms tangent space -> world space
    let TBN = mat3x3<f32>(T, B, N_geom);
    let N = normalize(TBN * tangent_normal);

    let V = normalize(camera.camera_pos - in.world_pos);

    // F0: base reflectivity; 0.04 for dielectrics, albedo for metals
    let F0 = mix(vec3<f32>(0.04, 0.04, 0.04), albedo, metallic);

    // Accumulate contribution from all active lights
    let shadow_factor = calculate_shadow(in.light_space_pos);
    var Lo = vec3<f32>(0.0);
    let light_count = min(lights_uniform.count, 16u);
    for (var i = 0u; i < light_count; i++) {
        var contribution = compute_light_contribution(
            lights_uniform.lights[i],
            N, V, in.world_pos, F0, albedo, metallic, roughness,
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

    let ambient = (diffuse_ibl + specular_ibl) * ao;

    var color = ambient + Lo;

    // Reinhard tone mapping
    color = color / (color + vec3<f32>(1.0));

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, material.base_color.a * tex_color.a);
}
