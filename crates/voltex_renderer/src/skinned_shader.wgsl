struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
};

struct LightUniform {
    direction: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    ambient_strength: f32,
};

struct BonePalette {
    bones: array<mat4x4<f32>, 128>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> light: LightUniform;
@group(0) @binding(2) var<storage, read> bone_palette: BonePalette;

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

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
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // Skinning: blend bone matrices
    let skin_matrix =
        bone_palette.bones[in.joints.x] * in.weights.x +
        bone_palette.bones[in.joints.y] * in.weights.y +
        bone_palette.bones[in.joints.z] * in.weights.z +
        bone_palette.bones[in.joints.w] * in.weights.w;

    let skinned_pos = skin_matrix * vec4<f32>(in.position, 1.0);
    let skinned_normal = (skin_matrix * vec4<f32>(in.normal, 0.0)).xyz;

    let world_pos = camera.model * skinned_pos;

    var out: VertexOutput;
    out.world_pos = world_pos.xyz;
    out.world_normal = normalize((camera.model * vec4<f32>(skinned_normal, 0.0)).xyz);
    out.clip_position = camera.view_proj * world_pos;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    let normal = normalize(in.world_normal);
    let light_dir = normalize(-light.direction);

    let ndotl = max(dot(normal, light_dir), 0.0);
    let diffuse = light.color * ndotl;

    let view_dir = normalize(camera.camera_pos - in.world_pos);
    let half_dir = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, half_dir), 0.0), 32.0);
    let specular = light.color * spec * 0.5;

    let ambient = light.color * light.ambient_strength;
    let lit = (ambient + diffuse + specular) * tex_color.rgb;
    return vec4<f32>(lit, tex_color.a);
}
