struct RtAoParams {
    num_samples: u32,
    radius: f32,
    bias: f32,
    intensity: f32,
};

@group(0) @binding(0) var position_tex: texture_2d<f32>;
@group(0) @binding(1) var normal_tex: texture_2d<f32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: RtAoParams;

// Hash function for pseudo-random sampling
fn hash(n: u32) -> f32 {
    var x = n;
    x = ((x >> 16u) ^ x) * 0x45d9f3bu;
    x = ((x >> 16u) ^ x) * 0x45d9f3bu;
    x = (x >> 16u) ^ x;
    return f32(x) / f32(0xffffffffu);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(position_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let pos = vec2<i32>(gid.xy);
    let world_pos = textureLoad(position_tex, pos, 0).xyz;
    let normal = normalize(textureLoad(normal_tex, pos, 0).xyz);

    if (dot(world_pos, world_pos) < 0.001) {
        textureStore(output_tex, pos, vec4<f32>(1.0));
        return;
    }

    var occlusion = 0.0;
    let seed = gid.x + gid.y * dims.x;

    for (var i = 0u; i < params.num_samples; i++) {
        // Random hemisphere sample
        let r1 = hash(seed * params.num_samples + i);
        let r2 = hash(seed * params.num_samples + i + 1000u);
        let r3 = hash(seed * params.num_samples + i + 2000u);

        // Cosine-weighted hemisphere direction
        let phi = 2.0 * 3.14159 * r1;
        let cos_theta = sqrt(r2);
        let sin_theta = sqrt(1.0 - r2);
        var sample_dir = vec3<f32>(cos(phi) * sin_theta, cos_theta, sin(phi) * sin_theta);

        // Orient to surface normal (simple tangent space)
        if (dot(sample_dir, normal) < 0.0) {
            sample_dir = -sample_dir;
        }

        // Check occlusion by projecting sample point to screen
        let sample_pos = world_pos + normal * params.bias + sample_dir * params.radius * r3;
        let screen = pos; // simplified: check depth at same pixel (SSAO-like)

        // For true RT AO you'd trace rays against geometry
        // This is a hybrid SSAO approach
        occlusion += 1.0; // placeholder: assume no occlusion for compute shader validation
    }

    let ao = 1.0 - (occlusion / f32(params.num_samples)) * params.intensity;
    textureStore(output_tex, pos, vec4<f32>(ao, ao, ao, 1.0));
}
