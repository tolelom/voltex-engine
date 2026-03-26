struct Params {
    blend_factor: f32,
    _pad: vec3<f32>,
};

@group(0) @binding(0) var current_tex: texture_2d<f32>;
@group(0) @binding(1) var history_tex: texture_2d<f32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(current_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let pos = vec2<i32>(gid.xy);
    let current = textureLoad(current_tex, pos, 0);
    let history = textureLoad(history_tex, pos, 0);

    let result = mix(history, current, params.blend_factor);
    textureStore(output_tex, pos, result);
}
