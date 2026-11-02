struct RtPointShadowParams {
    light_position: vec3<f32>,
    radius: f32,
    width: u32,
    height: u32,
    _pad: vec2<u32>,
};

@group(0) @binding(0) var position_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: RtPointShadowParams;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(position_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let pos = vec2<i32>(gid.xy);

    // Placeholder: output 1.0 (no shadow) for infrastructure validation
    textureStore(output_tex, pos, vec4<f32>(1.0, 0.0, 0.0, 0.0));
}
