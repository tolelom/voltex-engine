struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec3<f32>,
};

struct LightUniform {
    direction: vec3<f32>,
    color: vec3<f32>,
    ambient_strength: f32,
};

struct MaterialUniform {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> light: LightUniform;

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
    out.world_normal = (camera.model * vec4<f32>(model_v.normal, 0.0)).xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.uv = model_v.uv;
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    let albedo = material.base_color.rgb * tex_color.rgb;
    let metallic = material.metallic;
    let roughness = material.roughness;
    let ao = material.ao;

    let N = normalize(in.world_normal);
    let V = normalize(camera.camera_pos - in.world_pos);

    // F0: base reflectivity; 0.04 for dielectrics, albedo for metals
    let F0 = mix(vec3<f32>(0.04, 0.04, 0.04), albedo, metallic);

    // Single directional light
    let L = normalize(-light.direction);
    let H = normalize(V + L);
    let radiance = light.color;

    // Cook-Torrance BRDF components
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

    let Lo = (kd * albedo / 3.14159265358979 + specular) * radiance * NdotL;

    // Ambient term
    let ambient = light.ambient_strength * light.color * albedo * ao;

    var color = ambient + Lo;

    // Reinhard tone mapping
    color = color / (color + vec3<f32>(1.0));

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, material.base_color.a * tex_color.a);
}
