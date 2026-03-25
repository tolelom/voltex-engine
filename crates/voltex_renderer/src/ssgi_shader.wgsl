// SSGI (Screen-Space Global Illumination) pass shader.
// Reads the G-Buffer and computes per-pixel ambient occlusion + indirect color bleeding.
// Output: vec4(ao, indirect_r, indirect_g, indirect_b)

// ── Group 0: G-Buffer inputs ──────────────────────────────────────────────────

@group(0) @binding(0) var t_position: texture_2d<f32>;
@group(0) @binding(1) var t_normal:   texture_2d<f32>;
@group(0) @binding(2) var t_albedo:   texture_2d<f32>;
@group(0) @binding(3) var s_gbuffer:  sampler;

// ── Group 1: SSGI data ────────────────────────────────────────────────────────

struct SsgiUniform {
    projection:       mat4x4<f32>,
    view:             mat4x4<f32>,
    radius:           f32,
    bias:             f32,
    intensity:        f32,
    indirect_strength: f32,
};

struct SsgiKernel {
    samples: array<vec4<f32>, 64>,
};

@group(1) @binding(0) var<uniform> ssgi:   SsgiUniform;
@group(1) @binding(1) var<uniform> kernel: SsgiKernel;
@group(1) @binding(2) var t_noise:  texture_2d<f32>;
@group(1) @binding(3) var s_noise:  sampler;

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

// ── Fragment stage ────────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Read world-space position from G-Buffer.
    let world_pos = textureSample(t_position, s_gbuffer, uv).xyz;

    // Background pixel: return full AO, no indirect light.
    if length(world_pos) < 0.001 {
        return vec4<f32>(1.0, 0.0, 0.0, 0.0);
    }

    // World-space normal (stored as [0,1] encoded, decode back to [-1,1]).
    let normal_enc = textureSample(t_normal, s_gbuffer, uv).rgb;
    let N_world = normalize(normal_enc * 2.0 - 1.0);

    // Albedo for color bleeding.
    let albedo = textureSample(t_albedo, s_gbuffer, uv).rgb;

    // Convert world-space position and normal to view space.
    let view_pos4 = ssgi.view * vec4<f32>(world_pos, 1.0);
    let frag_pos_view = view_pos4.xyz;

    // Normal matrix = transpose(inverse(view)) ≈ mat3(view) for orthonormal view matrix.
    let N_view = normalize((ssgi.view * vec4<f32>(N_world, 0.0)).xyz);

    // ── TBN from noise ────────────────────────────────────────────────────────
    // Sample a random rotation vector from the noise texture (tiled 4×4 across screen).
    let tex_size = vec2<f32>(textureDimensions(t_position));
    let noise_uv  = uv * tex_size / 4.0; // tile the 4x4 noise over the screen
    let rand_vec  = textureSample(t_noise, s_noise, noise_uv).xy;
    let rand_dir  = normalize(vec3<f32>(rand_vec, 0.0));

    // Gram-Schmidt orthogonalization to build TBN in view space.
    let tangent   = normalize(rand_dir - N_view * dot(rand_dir, N_view));
    let bitangent = cross(N_view, tangent);
    // TBN matrix columns: tangent, bitangent, normal.

    // ── 64-sample hemisphere loop ─────────────────────────────────────────────
    var occlusion     = 0.0;
    var indirect_rgb  = vec3<f32>(0.0);

    for (var i = 0u; i < 64u; i++) {
        // Transform kernel sample from tangent space to view space.
        let s = kernel.samples[i].xyz;
        let sample_vs = tangent   * s.x
                      + bitangent * s.y
                      + N_view    * s.z;

        // Offset from current view-space fragment position.
        let sample_pos = frag_pos_view + sample_vs * ssgi.radius;

        // Project sample to get screen UV.
        let offset4  = ssgi.projection * vec4<f32>(sample_pos, 1.0);
        let offset_ndc = offset4.xyz / offset4.w;
        let sample_uv  = vec2<f32>(
            offset_ndc.x * 0.5 + 0.5,
            1.0 - (offset_ndc.y * 0.5 + 0.5),
        );

        // Clamp to [0,1] to avoid sampling outside the texture.
        if sample_uv.x < 0.0 || sample_uv.x > 1.0
            || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
            continue;
        }

        // Actual geometry depth at the projected UV.
        let scene_pos_world = textureSample(t_position, s_gbuffer, sample_uv).xyz;
        let scene_pos_view  = (ssgi.view * vec4<f32>(scene_pos_world, 1.0)).xyz;
        let scene_depth     = scene_pos_view.z;

        // Range check: ignore samples that are too far away (avoid halo artefacts).
        let range_check = smoothstep(0.0, 1.0, ssgi.radius / abs(frag_pos_view.z - scene_depth));

        // Occlusion: geometry behind the sample blocks light.
        let occluded = select(0.0, 1.0, scene_depth >= sample_pos.z + ssgi.bias);
        occlusion += occluded * range_check;

        // Color bleeding: gather albedo from occluding surfaces.
        let neighbor_albedo = textureSample(t_albedo, s_gbuffer, sample_uv).rgb;
        indirect_rgb += neighbor_albedo * occluded * range_check;
    }

    // Normalize by sample count.
    let inv_samples = 1.0 / 64.0;
    occlusion    *= inv_samples;
    indirect_rgb *= inv_samples;

    // AO: how unoccluded the fragment is (1 = fully lit, 0 = fully occluded).
    let ao = 1.0 - occlusion * ssgi.intensity;
    let ao_clamped = clamp(ao, 0.0, 1.0);

    // Indirect light contribution scaled by indirect_strength and albedo.
    let indirect = indirect_rgb * ssgi.indirect_strength * albedo;

    return vec4<f32>(ao_clamped, indirect.r, indirect.g, indirect.b);
}
