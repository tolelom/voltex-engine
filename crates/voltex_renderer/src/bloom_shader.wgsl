// Bloom pass shader.
// Two fragment entry points:
//   fs_downsample — 5-tap box filter with optional bright extraction
//   fs_upsample   — 9-tap tent filter for the upsample chain

// ── Group 0 ───────────────────────────────────────────────────────────────────

@group(0) @binding(0) var t_input: texture_2d<f32>;
@group(0) @binding(1) var s_input: sampler;

struct BloomUniform {
    threshold:      f32,
    soft_threshold: f32,
    _padding:       vec2<f32>,
};

@group(0) @binding(2) var<uniform> bloom: BloomUniform;

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

// ── Shared helpers ────────────────────────────────────────────────────────────

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

/// Soft-knee bright-pass: returns the colour with sub-threshold values attenuated.
fn bright_extract(color: vec3<f32>, threshold: f32, soft: f32) -> vec3<f32> {
    let lum = luminance(color);
    let knee = threshold * soft;
    // Quadratic soft knee
    let ramp = clamp(lum - threshold + knee, 0.0, 2.0 * knee);
    let weight = select(
        select(0.0, 1.0, lum >= threshold),
        ramp * ramp / (4.0 * knee + 0.00001),
        lum >= threshold - knee && lum < threshold,
    );
    // When lum >= threshold we pass through, otherwise fade via weight
    if lum >= threshold {
        return color;
    }
    return color * (weight / max(lum, 0.00001));
}

// ── Downsample pass ───────────────────────────────────────────────────────────

/// 5-sample box filter: centre × 4 + 4 diagonal neighbours × 1, divided by 8.
/// When bloom.threshold > 0 a bright-pass is applied after filtering.
@fragment
fn fs_downsample(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let texel = 1.0 / vec2<f32>(textureDimensions(t_input));

    let center = textureSample(t_input, s_input, uv).rgb;
    let tl = textureSample(t_input, s_input, uv + vec2<f32>(-1.0, -1.0) * texel).rgb;
    let tr = textureSample(t_input, s_input, uv + vec2<f32>( 1.0, -1.0) * texel).rgb;
    let bl = textureSample(t_input, s_input, uv + vec2<f32>(-1.0,  1.0) * texel).rgb;
    let br = textureSample(t_input, s_input, uv + vec2<f32>( 1.0,  1.0) * texel).rgb;

    var color = (center * 4.0 + tl + tr + bl + br) / 8.0;

    // Bright extraction on the first downsample level (threshold guard)
    if bloom.threshold > 0.0 {
        color = bright_extract(color, bloom.threshold, bloom.soft_threshold);
    }

    return vec4<f32>(color, 1.0);
}

// ── Upsample pass ─────────────────────────────────────────────────────────────

/// 9-tap tent filter (3×3):
///   corners × 1, axis-aligned edges × 2, centre × 4  →  /16
@fragment
fn fs_upsample(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let texel = 1.0 / vec2<f32>(textureDimensions(t_input));

    let tl = textureSample(t_input, s_input, uv + vec2<f32>(-1.0, -1.0) * texel).rgb;
    let tc = textureSample(t_input, s_input, uv + vec2<f32>( 0.0, -1.0) * texel).rgb;
    let tr = textureSample(t_input, s_input, uv + vec2<f32>( 1.0, -1.0) * texel).rgb;
    let ml = textureSample(t_input, s_input, uv + vec2<f32>(-1.0,  0.0) * texel).rgb;
    let mc = textureSample(t_input, s_input, uv).rgb;
    let mr = textureSample(t_input, s_input, uv + vec2<f32>( 1.0,  0.0) * texel).rgb;
    let bl = textureSample(t_input, s_input, uv + vec2<f32>(-1.0,  1.0) * texel).rgb;
    let bc = textureSample(t_input, s_input, uv + vec2<f32>( 0.0,  1.0) * texel).rgb;
    let br = textureSample(t_input, s_input, uv + vec2<f32>( 1.0,  1.0) * texel).rgb;

    // Tent weights: corners×1 + edges×2 + centre×4 → sum=16
    let color = (tl + tr + bl + br
               + (tc + ml + mr + bc) * 2.0
               + mc * 4.0) / 16.0;

    return vec4<f32>(color, 1.0);
}
