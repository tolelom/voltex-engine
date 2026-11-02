struct MotionBlurParams {
    inv_view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
    num_samples: u32,
    strength: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var color_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: MotionBlurParams;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(color_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let pos = vec2<i32>(gid.xy);
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(dims);

    // Reconstruct world position from depth
    let depth = textureLoad(depth_tex, pos, 0);
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world = params.inv_view_proj * ndc;
    let world_pos = world.xyz / world.w;

    // Project to previous frame
    let prev_clip = params.prev_view_proj * vec4<f32>(world_pos, 1.0);
    let prev_ndc = prev_clip.xyz / prev_clip.w;
    let prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 1.0 - (prev_ndc.y * 0.5 + 0.5));

    // Velocity = current_uv - prev_uv
    let velocity = (uv - prev_uv) * params.strength;

    // Sample along velocity direction
    var color = vec4<f32>(0.0);
    let n = params.num_samples;
    for (var i = 0u; i < n; i++) {
        let t = f32(i) / f32(n - 1u) - 0.5; // -0.5 to 0.5
        let sample_uv = uv + velocity * t;
        let sample_pos = vec2<i32>(sample_uv * vec2<f32>(dims));
        let clamped = clamp(sample_pos, vec2<i32>(0), vec2<i32>(dims) - 1);
        color += textureLoad(color_tex, clamped, 0);
    }
    color /= f32(n);

    textureStore(output_tex, pos, color);
}
