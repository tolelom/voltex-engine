// Depth-only skinned shadow caster.
//
// Reuses the colored skinned bind-group layout (group 0 = SkinCamera, group 1 =
// BonePalette) but with `view_proj` set to the light's view-projection, so
// skinned meshes write their depth into the directional shadow map exactly like
// static meshes do via shadow_shader.wgsl. GPU vertex skinning mirrors
// mesh_skinned.wgsl; there is no fragment stage (depth-only pass).

struct SkinCamera {
    view_proj: mat4x4<f32>, // light view-projection for the shadow pass
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

@vertex
fn vs_main(v: VertexInput) -> @builtin(position) vec4<f32> {
    let skin = palette.bones[v.joints.x] * v.weights.x
             + palette.bones[v.joints.y] * v.weights.y
             + palette.bones[v.joints.z] * v.weights.z
             + palette.bones[v.joints.w] * v.weights.w;
    let world_pos = camera.model * (skin * vec4<f32>(v.position, 1.0));
    return camera.view_proj * world_pos;
}
