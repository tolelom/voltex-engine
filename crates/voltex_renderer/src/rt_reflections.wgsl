struct RtReflParams {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    max_distance: f32,
    num_samples: u32,
    _pad: vec3<u32>,
};

@group(0) @binding(0) var position_tex: texture_2d<f32>;
@group(0) @binding(1) var normal_tex: texture_2d<f32>;
@group(0) @binding(2) var material_tex: texture_2d<f32>;
@group(0) @binding(3) var color_tex: texture_2d<f32>;
@group(0) @binding(4) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var<uniform> params: RtReflParams;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(position_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let pos = vec2<i32>(gid.xy);
    let world_pos = textureLoad(position_tex, pos, 0).xyz;
    let normal = normalize(textureLoad(normal_tex, pos, 0).xyz);
    let material = textureLoad(material_tex, pos, 0);
    let roughness = material.g;

    if (dot(world_pos, world_pos) < 0.001) {
        textureStore(output_tex, pos, vec4<f32>(0.0));
        return;
    }

    let view_dir = normalize(world_pos - params.camera_pos);
    let reflect_dir = reflect(view_dir, normal);

    // Fade by roughness (rough surfaces = less reflection)
    let fade = 1.0 - roughness;

    // March along reflection ray in world space (simplified)
    var hit_color = vec4<f32>(0.0);
    var ray_pos = world_pos + reflect_dir * 0.1;
    let step = params.max_distance / f32(params.num_samples);

    for (var i = 0u; i < params.num_samples; i++) {
        let clip = params.view_proj * vec4<f32>(ray_pos, 1.0);
        let ndc = clip.xyz / clip.w;
        let screen_uv = vec2<f32>(ndc.x * 0.5 + 0.5, 1.0 - (ndc.y * 0.5 + 0.5));
        let sx = i32(screen_uv.x * f32(dims.x));
        let sy = i32(screen_uv.y * f32(dims.y));

        if (sx >= 0 && sy >= 0 && sx < i32(dims.x) && sy < i32(dims.y) && ndc.z > 0.0 && ndc.z < 1.0) {
            let sample_world = textureLoad(position_tex, vec2<i32>(sx, sy), 0).xyz;
            let dist_to_surface = length(ray_pos - sample_world);
            if (dist_to_surface < step * 1.5) {
                hit_color = textureLoad(color_tex, vec2<i32>(sx, sy), 0) * fade;
                break;
            }
        }
        ray_pos += reflect_dir * step;
    }

    textureStore(output_tex, pos, hit_color);
}
