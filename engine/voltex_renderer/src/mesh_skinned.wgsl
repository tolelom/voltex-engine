// Skinned (GPU vertex skinning) Blinn-Phong-ish shader.
//
// Each vertex carries 4 joint indices + 4 weights (SkinnedMeshVertex). The
// vertex stage blends the bone palette matrices by those weights and applies
// the result before the model/view-projection transforms, so the mesh deforms
// with the skeleton pose computed on the CPU (`Skeleton::compute_palette`).
//
// Self-contained bind groups (no dynamic offsets) so it is easy to drive from
// the capture harness and the runtime alike:
//   group(0): camera (view_proj + model)
//   group(1): bone palette (array<mat4x4, MAX_BONES>)
// Lighting is a single baked directional term — enough to read the deformation
// in a rendered frame without a separate light uniform.

struct SkinCamera {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: SkinCamera;

struct BonePalette {
    bones: array<mat4x4<f32>, 128>,
};
@group(1) @binding(0) var<uniform> palette: BonePalette;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(4) joints: vec4<u32>,
    @location(5) weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
};

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    // Weighted blend of the four influencing bone matrices.
    let skin = palette.bones[v.joints.x] * v.weights.x
             + palette.bones[v.joints.y] * v.weights.y
             + palette.bones[v.joints.z] * v.weights.z
             + palette.bones[v.joints.w] * v.weights.w;

    let skinned_pos = skin * vec4<f32>(v.position, 1.0);
    let world_pos = camera.model * skinned_pos;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_pos;
    out.world_normal = (camera.model * skin * vec4<f32>(v.normal, 0.0)).xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let light_dir = normalize(vec3<f32>(0.4, 0.8, 0.6));
    let diff = max(dot(n, light_dir), 0.0);
    let base = vec3<f32>(0.70, 0.75, 0.85);
    let color = base * (0.2 + 0.8 * diff);
    return vec4<f32>(color, 1.0);
}
