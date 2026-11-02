@group(0) @binding(0) var hdr_texture: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> result: array<atomic<u32>>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(hdr_texture);
    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }

    let color = textureLoad(hdr_texture, vec2<i32>(gid.xy), 0);
    let lum = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
    let log_lum = log(max(lum, 0.0001));

    let fixed = i32(log_lum * 1000.0);
    atomicAdd(&result[0], bitcast<u32>(fixed));
    atomicAdd(&result[1], 1u);
}
