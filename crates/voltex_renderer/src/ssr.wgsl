struct SsrParams {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    max_steps: u32,
    step_size: f32,
    thickness: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var position_tex: texture_2d<f32>;   // G-Buffer world position
@group(0) @binding(1) var normal_tex: texture_2d<f32>;     // G-Buffer world normal
@group(0) @binding(2) var color_tex: texture_2d<f32>;      // Lit HDR color
@group(0) @binding(3) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var<uniform> params: SsrParams;

fn world_to_screen(world_pos: vec3<f32>) -> vec3<f32> {
    let clip = params.view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    let dims = vec2<f32>(textureDimensions(color_tex));
    return vec3<f32>(
        (ndc.x * 0.5 + 0.5) * dims.x,
        (1.0 - (ndc.y * 0.5 + 0.5)) * dims.y,
        ndc.z,
    );
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(position_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let pos = vec2<i32>(gid.xy);
    let world_pos = textureLoad(position_tex, pos, 0).xyz;
    let normal = normalize(textureLoad(normal_tex, pos, 0).xyz);

    // Skip pixels with no geometry (position = 0)
    if (dot(world_pos, world_pos) < 0.001) {
        textureStore(output_tex, pos, vec4<f32>(0.0));
        return;
    }

    // Reflect view direction
    let view_dir = normalize(world_pos - params.camera_pos);
    let reflect_dir = reflect(view_dir, normal);

    // Ray march in world space, project to screen each step
    var ray_pos = world_pos + reflect_dir * params.step_size;
    var hit_color = vec4<f32>(0.0);

    for (var i = 0u; i < params.max_steps; i++) {
        let screen = world_to_screen(ray_pos);
        let sx = i32(screen.x);
        let sy = i32(screen.y);

        // Bounds check
        if (sx < 0 || sy < 0 || sx >= i32(dims.x) || sy >= i32(dims.y) || screen.z < 0.0 || screen.z > 1.0) {
            break;
        }

        // Compare depth
        let sample_pos = textureLoad(position_tex, vec2<i32>(sx, sy), 0).xyz;
        let sample_screen = world_to_screen(sample_pos);
        let depth_diff = screen.z - sample_screen.z;

        if (depth_diff > 0.0 && depth_diff < params.thickness) {
            // Hit! Sample the color
            hit_color = textureLoad(color_tex, vec2<i32>(sx, sy), 0);
            // Fade at edges
            let edge_fade = 1.0 - max(
                abs(f32(sx) / f32(dims.x) * 2.0 - 1.0),
                abs(f32(sy) / f32(dims.y) * 2.0 - 1.0),
            );
            hit_color = hit_color * clamp(edge_fade, 0.0, 1.0);
            break;
        }

        ray_pos += reflect_dir * params.step_size;
    }

    textureStore(output_tex, pos, hit_color);
}
