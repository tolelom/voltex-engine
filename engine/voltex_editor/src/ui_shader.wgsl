struct UiUniform {
    projection: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> ui_uniform: UiUniform;
@group(0) @binding(1) var t_atlas: texture_2d<f32>;
@group(0) @binding(2) var s_atlas: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = ui_uniform.projection * vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_alpha = textureSample(t_atlas, s_atlas, in.uv).r;
    return vec4<f32>(in.color.rgb * tex_alpha, in.color.a * tex_alpha);
}

// Image widgets: sample full RGBA from a registered texture (the font path
// above reads only `.r` — the R8Unorm atlas alpha convention).
@fragment
fn fs_image(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_atlas, s_atlas, in.uv) * in.color;
}
