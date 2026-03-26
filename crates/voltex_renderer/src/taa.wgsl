struct TaaParams {
    jitter: vec2<f32>,        // current frame jitter offset
    prev_jitter: vec2<f32>,   // previous frame jitter offset
    blend_factor: f32,        // 0.1 (10% new, 90% history)
    _pad: vec3<f32>,
};

@group(0) @binding(0) var current_tex: texture_2d<f32>;
@group(0) @binding(1) var history_tex: texture_2d<f32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: TaaParams;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(current_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let pos = vec2<i32>(gid.xy);
    let current = textureLoad(current_tex, pos, 0);

    // Neighborhood color clamping (3x3 AABB)
    var color_min = vec4<f32>(999.0);
    var color_max = vec4<f32>(-999.0);
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let np = pos + vec2<i32>(dx, dy);
            let nc = textureLoad(current_tex, clamp(np, vec2<i32>(0), vec2<i32>(dims) - 1), 0);
            color_min = min(color_min, nc);
            color_max = max(color_max, nc);
        }
    }

    // Reprojection: unjitter to find history position
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(dims);
    let history_uv = uv + params.prev_jitter - params.jitter;
    let history_pos = vec2<i32>(history_uv * vec2<f32>(dims));

    var history: vec4<f32>;
    if (history_pos.x >= 0 && history_pos.y >= 0 &&
        history_pos.x < i32(dims.x) && history_pos.y < i32(dims.y)) {
        history = textureLoad(history_tex, history_pos, 0);
    } else {
        history = current;
    }

    // Clamp history to neighborhood
    let clamped_history = clamp(history, color_min, color_max);

    // Blend
    let result = mix(clamped_history, current, params.blend_factor);
    textureStore(output_tex, pos, result);
}
