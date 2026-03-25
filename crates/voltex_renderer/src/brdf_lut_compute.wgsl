// GPU Compute shader for BRDF LUT generation (split-sum approximation).
// Workgroup size: 16x16, each thread computes one texel.
// Output: Rg16Float texture with (scale, bias) per texel.

@group(0) @binding(0) var output_tex: texture_storage_2d<rg16float, write>;

const PI: f32 = 3.14159265358979;
const NUM_SAMPLES: u32 = 1024u;

// Van der Corput radical inverse via bit-reversal
fn radical_inverse_vdc(bits_in: u32) -> f32 {
    var bits = bits_in;
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return f32(bits) * 2.3283064365386963e-10; // / 0x100000000
}

// Hammersley low-discrepancy 2D sample
fn hammersley(i: u32, n: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

// GGX importance-sampled half vector in tangent space (N = (0,0,1))
fn importance_sample_ggx(xi: vec2<f32>, roughness: f32) -> vec3<f32> {
    let a = roughness * roughness;
    let phi = 2.0 * PI * xi.x;
    let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
    let sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
    return vec3<f32>(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);
}

// Smith geometry function for IBL: k = a^2/2
fn geometry_smith_ibl(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let k = a / 2.0;
    let ggx_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let ggx_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
    return ggx_v * ggx_l;
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(output_tex);
    if gid.x >= dims.x || gid.y >= dims.y {
        return;
    }

    let size = f32(dims.x);
    let n_dot_v = (f32(gid.x) + 0.5) / size;
    let roughness = clamp((f32(gid.y) + 0.5) / size, 0.0, 1.0);

    let n_dot_v_clamped = clamp(n_dot_v, 0.0, 1.0);

    // View vector in tangent space where N = (0,0,1)
    let v = vec3<f32>(sqrt(max(1.0 - n_dot_v_clamped * n_dot_v_clamped, 0.0)), 0.0, n_dot_v_clamped);

    var scale = 0.0;
    var bias = 0.0;

    for (var i = 0u; i < NUM_SAMPLES; i++) {
        let xi = hammersley(i, NUM_SAMPLES);
        let h = importance_sample_ggx(xi, roughness);

        // dot(V, H)
        let v_dot_h = max(dot(v, h), 0.0);

        // Reflect V around H to get L
        let l = 2.0 * v_dot_h * h - v;

        let n_dot_l = max(l.z, 0.0); // L.z in tangent space
        let n_dot_h = max(h.z, 0.0);

        if n_dot_l > 0.0 {
            let g = geometry_smith_ibl(n_dot_v_clamped, n_dot_l, roughness);
            let g_vis = g * v_dot_h / max(n_dot_h * n_dot_v_clamped, 0.001);
            let fc = pow(1.0 - v_dot_h, 5.0);
            scale += g_vis * (1.0 - fc);
            bias += g_vis * fc;
        }
    }

    scale /= f32(NUM_SAMPLES);
    bias /= f32(NUM_SAMPLES);

    textureStore(output_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(scale, bias, 0.0, 1.0));
}
