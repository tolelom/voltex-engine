@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var normal_tex: texture_2d<f32>;
@group(0) @binding(3) var output_tex: texture_storage_2d<rgba16float, write>;

const KERNEL_RADIUS: i32 = 2;
const SIGMA_SPATIAL: f32 = 2.0;
const SIGMA_DEPTH: f32 = 0.1;
const SIGMA_NORMAL: f32 = 16.0;

fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(input_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let center = vec2<i32>(gid.xy);
    let center_depth = textureLoad(depth_tex, center, 0);
    let center_normal = textureLoad(normal_tex, center, 0).xyz;

    var sum = vec4<f32>(0.0);
    var weight_sum = 0.0;

    for (var dy = -KERNEL_RADIUS; dy <= KERNEL_RADIUS; dy++) {
        for (var dx = -KERNEL_RADIUS; dx <= KERNEL_RADIUS; dx++) {
            let offset = vec2<i32>(dx, dy);
            let sample_pos = center + offset;

            if (sample_pos.x < 0 || sample_pos.y < 0 ||
                sample_pos.x >= i32(dims.x) || sample_pos.y >= i32(dims.y)) {
                continue;
            }

            let spatial_dist = sqrt(f32(dx * dx + dy * dy));
            let w_spatial = gaussian(spatial_dist, SIGMA_SPATIAL);

            let sample_depth = textureLoad(depth_tex, sample_pos, 0);
            let depth_diff = abs(center_depth - sample_depth);
            let w_depth = exp(-depth_diff / SIGMA_DEPTH);

            let sample_normal = textureLoad(normal_tex, sample_pos, 0).xyz;
            let n_dot = max(dot(center_normal, sample_normal), 0.0);
            let w_normal = pow(n_dot, SIGMA_NORMAL);

            let weight = w_spatial * w_depth * w_normal;
            let sample_color = textureLoad(input_tex, sample_pos, 0);

            sum += sample_color * weight;
            weight_sum += weight;
        }
    }

    let result = select(vec4<f32>(0.0), sum / weight_sum, weight_sum > 0.0);
    textureStore(output_tex, vec2<i32>(gid.xy), result);
}
