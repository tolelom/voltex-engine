// Billboard sprite shader: a unit quad expanded along the camera's right/up
// axes so it always faces the camera. group 0 = per-sprite uniform, group 1 =
// diffuse texture (white default for solid-color sprites).

struct Sprite {
    view_proj: mat4x4<f32>,
    camera_right: vec4<f32>,
    camera_up: vec4<f32>,
    center: vec4<f32>,
    color: vec4<f32>,
    size: vec4<f32>,    // xy = width/height, z = screen-plane roll (radians)
    uv_rect: vec4<f32>, // [u_min, v_min, u_max, v_max]
};
@group(0) @binding(0) var<uniform> sprite: Sprite;

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

struct VsIn {
    @location(0) corner: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    // Scale the unit corner to the sprite size, then roll it in the screen
    // plane (size.z = 0 for upright billboards).
    let local = vec2<f32>(in.corner.x * sprite.size.x, in.corner.y * sprite.size.y);
    let c = cos(sprite.size.z);
    let s = sin(sprite.size.z);
    let rolled = vec2<f32>(local.x * c - local.y * s, local.x * s + local.y * c);
    let world = sprite.center.xyz
        + sprite.camera_right.xyz * rolled.x
        + sprite.camera_up.xyz * rolled.y;
    out.clip = sprite.view_proj * vec4<f32>(world, 1.0);
    // Remap the unit-quad UV into the sprite's texture sub-rect (atlas frame).
    out.uv = mix(sprite.uv_rect.xy, sprite.uv_rect.zw, in.uv);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.uv) * sprite.color;
}
