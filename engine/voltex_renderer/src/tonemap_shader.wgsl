// Tonemap pass shader.
// Combines HDR scene colour + bloom, applies exposure and ACES tonemapping,
// then converts to gamma-corrected sRGB for the swapchain.

// ── Group 0 ───────────────────────────────────────────────────────────────────

@group(0) @binding(0) var t_hdr:     texture_2d<f32>;
@group(0) @binding(1) var t_bloom:   texture_2d<f32>;
@group(0) @binding(2) var s_sampler: sampler;

struct TonemapUniform {
    bloom_intensity: f32,
    exposure:        f32,
    _padding:        vec2<f32>,
};

@group(0) @binding(3) var<uniform> tonemap: TonemapUniform;

// ── Vertex stage ──────────────────────────────────────────────────────────────

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0)       uv:            vec2<f32>,
};

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(v.position, 0.0, 1.0);
    out.uv = vec2<f32>(v.position.x * 0.5 + 0.5, 1.0 - (v.position.y * 0.5 + 0.5));
    return out;
}

// ── ACES tonemap ──────────────────────────────────────────────────────────────

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let num = x * (2.51 * x + vec3<f32>(0.03));
    let den = x * (2.43 * x + vec3<f32>(0.59)) + vec3<f32>(0.14);
    return clamp(num / den, vec3<f32>(0.0), vec3<f32>(1.0));
}

// ── Fragment stage ────────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let hdr   = textureSample(t_hdr,   s_sampler, uv).rgb;
    let bloom = textureSample(t_bloom, s_sampler, uv).rgb;

    // Combine HDR + bloom
    var color = hdr + bloom * tonemap.bloom_intensity;

    // Apply exposure
    color = color * tonemap.exposure;

    // ACES tonemapping
    color = aces_tonemap(color);

    // Gamma correction (linear → sRGB, γ = 2.2)
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, 1.0);
}
