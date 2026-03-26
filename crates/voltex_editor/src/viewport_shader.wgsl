struct RectUniform {
    rect: vec4<f32>,
    screen: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: RectUniform;
@group(0) @binding(1) var t_viewport: texture_2d<f32>;
@group(0) @binding(2) var s_viewport: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );

    let p = positions[idx];
    let px = u.rect.x + p.x * u.rect.z;
    let py = u.rect.y + p.y * u.rect.w;
    let ndc_x = (px / u.screen.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / u.screen.y) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = p;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_viewport, s_viewport, in.uv);
}
