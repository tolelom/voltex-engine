enable wgpu_ray_query;

// RT Shadow compute shader.
// Reads world-space position and normal from the G-Buffer, then fires a
// shadow ray against the TLAS to determine per-pixel visibility.
// Output is 1.0 (lit) or 0.0 (shadowed) stored in an R32Float texture.

// ── Group 0: G-Buffer inputs ──────────────────────────────────────────────────

@group(0) @binding(0) var t_position: texture_2d<f32>;
@group(0) @binding(1) var t_normal:   texture_2d<f32>;

// ── Group 1: RT data ─────────────────────────────────────────────────────────

@group(1) @binding(0) var tlas:          acceleration_structure;
@group(1) @binding(1) var t_shadow_out:  texture_storage_2d<r32float, write>;

struct RtShadowUniform {
    light_direction: vec3<f32>,
    _pad0: f32,
    width: u32,
    height: u32,
    _pad1: vec2<u32>,
};

@group(1) @binding(2) var<uniform> uniforms: RtShadowUniform;

// ── Compute entry point ───────────────────────────────────────────────────────

@compute @workgroup_size(8, 8)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let coord = vec2<i32>(i32(gid.x), i32(gid.y));

    // Bounds check
    if gid.x >= uniforms.width || gid.y >= uniforms.height {
        return;
    }

    // Read world position from G-Buffer
    let world_pos = textureLoad(t_position, coord, 0).xyz;

    // Background pixel: skip (position is (0,0,0) for skybox pixels)
    if dot(world_pos, world_pos) < 0.001 {
        textureStore(t_shadow_out, coord, vec4<f32>(1.0, 0.0, 0.0, 0.0));
        return;
    }

    // Read and decode normal — G-Buffer stores N * 0.5 + 0.5
    let normal_encoded = textureLoad(t_normal, coord, 0).rgb;
    let N = normalize(normal_encoded * 2.0 - 1.0);

    // Ray: from surface towards the light, biased along normal to avoid self-intersection
    let ray_origin = world_pos + N * 0.01;
    let ray_dir    = normalize(-uniforms.light_direction);

    // Build ray descriptor
    var desc: RayDesc;
    desc.flags = RAY_FLAG_TERMINATE_ON_FIRST_HIT;
    desc.cull_mask = 0xFFu;
    desc.tmin = 0.001;
    desc.tmax = 1000.0;
    desc.origin = ray_origin;
    desc.dir = ray_dir;

    // Ray query
    var rq: ray_query;
    rayQueryInitialize(&rq, tlas, desc);
    while rayQueryProceed(&rq) {}

    // Check result
    let intersection = rayQueryGetCommittedIntersection(&rq);
    var shadow: f32 = 1.0;
    if intersection.kind != RAY_QUERY_INTERSECTION_NONE {
        shadow = 0.0;
    }

    textureStore(t_shadow_out, coord, vec4<f32>(shadow, 0.0, 0.0, 0.0));
}
