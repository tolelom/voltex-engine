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
    let result = (ambient + diffuse + specular) * tex_color.rgb;
    return vec4<f32>(result, tex_color.a);
}
