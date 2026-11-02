// Batched tilemap shader: every tile quad of a map is pre-expanded on the CPU
// into world-space vertices, so one draw covers the whole map. group 0 = frame
// uniform (view_proj), group 1 = atlas texture (white default for solid maps).

struct Frame {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> frame: Frame;

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = frame.view_proj * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.uv) * in.color;
}
