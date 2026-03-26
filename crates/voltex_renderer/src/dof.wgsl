struct DofParams {
    focus_distance: f32,
    focus_range: f32,
    max_blur: f32,
    _pad: f32,
};

@group(0) @binding(0) var color_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: DofParams;

fn circle_of_confusion(depth: f32) -> f32 {
    let diff = abs(depth - params.focus_distance);
    let coc = clamp(diff / params.focus_range, 0.0, 1.0) * params.max_blur;
    return coc;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(color_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let pos = vec2<i32>(gid.xy);
    let depth = textureLoad(depth_tex, pos, 0);
    let coc = circle_of_confusion(depth);

    if (coc < 0.5) {
        // In focus — no blur
        textureStore(output_tex, pos, textureLoad(color_tex, pos, 0));
        return;
    }

    // Disc blur with radius = coc
    let radius = i32(coc);
    var color = vec4<f32>(0.0);
    var weight = 0.0;

    for (var dy = -radius; dy <= radius; dy++) {
        for (var dx = -radius; dx <= radius; dx++) {
            let dist = sqrt(f32(dx * dx + dy * dy));
            if (dist > coc) { continue; }

            let sample_pos = pos + vec2<i32>(dx, dy);
            let clamped = clamp(sample_pos, vec2<i32>(0), vec2<i32>(dims) - 1);
            let sample_color = textureLoad(color_tex, clamped, 0);
            let w = 1.0 - dist / (coc + 0.001);
            color += sample_color * w;
            weight += w;
        }
    }

    let result = select(textureLoad(color_tex, pos, 0), color / weight, weight > 0.0);
    textureStore(output_tex, pos, result);
}
