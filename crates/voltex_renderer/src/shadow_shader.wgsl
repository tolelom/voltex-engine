struct ShadowPassUniform {
    light_vp_model: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> shadow_pass: ShadowPassUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(model_v: VertexInput) -> @builtin(position) vec4<f32> {
    return shadow_pass.light_vp_model * vec4<f32>(model_v.position, 1.0);
}
