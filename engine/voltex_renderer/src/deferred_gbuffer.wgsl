// G-Buffer pass shader for deferred rendering.
// Writes geometry data to multiple render targets.

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

@group(1) @binding(0) var t_albedo: texture_2d<f32>;
@group(1) @binding(1) var s_albedo: sampler;
@group(1) @binding(2) var t_normal: texture_2d<f32>;
@group(1) @binding(3) var s_normal: sampler;
@group(1) @binding(4) var t_orm: texture_2d<f32>;
@group(1) @binding(5) var s_orm: sampler;
@group(1) @binding(6) var t_emissive: texture_2d<f32>;
@group(1) @binding(7) var s_emissive: sampler;

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

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos4 = camera.model * vec4<f32>(v.position, 1.0);
    out.world_pos = world_pos4.xyz;
    out.clip_position = camera.view_proj * world_pos4;
    out.uv = v.uv;

    let N = normalize((camera.model * vec4<f32>(v.normal, 0.0)).xyz);
    let T = normalize((camera.model * vec4<f32>(v.tangent.xyz, 0.0)).xyz);
    let B = cross(N, T) * v.tangent.w;

    out.world_normal = N;
    out.world_tangent = T;
    out.world_bitangent = B;

    return out;
}

struct GBufferOutput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) albedo: vec4<f32>,
    @location(3) material_data: vec4<f32>,
};

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
    // Sample albedo texture
    let tex_color = textureSample(t_albedo, s_albedo, in.uv);
    let albedo = material.base_color.rgb * tex_color.rgb;

    // Normal mapping via TBN matrix
    let T = normalize(in.world_tangent);
    let B = normalize(in.world_bitangent);
    let N_geom = normalize(in.world_normal);

    let normal_sample = textureSample(t_normal, s_normal, in.uv).rgb;
    let tangent_normal = normal_sample * 2.0 - 1.0;

    // TBN: tangent space → world space
    let TBN = mat3x3<f32>(T, B, N_geom);
    let N = normalize(TBN * tangent_normal);

    // Sample ORM texture: R=AO, G=Roughness, B=Metallic; multiply with material params
    let orm_sample = textureSample(t_orm, s_orm, in.uv);
    let ao = orm_sample.r * material.ao;
    let roughness = orm_sample.g * material.roughness;
    let metallic = orm_sample.b * material.metallic;

    // Sample emissive texture and compute luminance
    let emissive = textureSample(t_emissive, s_emissive, in.uv).rgb;
    let emissive_lum = dot(emissive, vec3<f32>(0.299, 0.587, 0.114));

    var out: GBufferOutput;
    out.position = vec4<f32>(in.world_pos, 1.0);
    out.normal = vec4<f32>(N * 0.5 + 0.5, 1.0);
    out.albedo = vec4<f32>(albedo, material.base_color.a * tex_color.a);
    out.material_data = vec4<f32>(metallic, roughness, ao, emissive_lum);

    return out;
}
