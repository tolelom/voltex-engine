//! Reusable demo scenes rendered through the *real* ECS render path
//! (`voltex_renderer_ecs::BasicRenderer`). Each returns a tightly-packed
//! RGBA8 buffer so a bin can save a PNG and a (GPU-gated) test can assert the
//! frame isn't blank.

use voltex_ecs::{Bounds, MaterialHandle, MeshHandle, Transform, World};
use voltex_math::{Mat4, Quat, Vec3, AABB};
use voltex_renderer::{Camera, CameraUniform, LightUniform, MaterialUniform, Mesh};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};

use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

pub use crate::scene_hot_reload::{render_mesh_hot_reload, render_texture_hot_reload};
pub use crate::scene_sprite::{render_billboard_sprite, render_particle_burst, render_sprite_sheet_frame};
pub use crate::scene_texture::{
    render_cube_with_png, render_cube_with_texture, render_obj_model, render_textured_cube,
};

mod declarative;
mod lighting;
mod pbr;

pub use declarative::{
    render_boids_swarm, render_data_authored_scene, render_declarative_colored_scene,
    render_materialized_primitives, render_world, render_world_ortho,
};
pub use lighting::{render_cube_point_lit, render_shadow_demo, render_spot_lit};
pub use pbr::{render_pbr_basic_renderer, render_pbr_grid, render_pbr_sphere_normalmap};

fn align_up(size: u32, alignment: u32) -> u32 {
    size.div_ceil(alignment) * alignment
}

/// Build a unit cube mesh (24 verts, 36 indices, outward normals).
pub(crate) fn unit_cube(device: &wgpu::Device) -> Mesh {
    Mesh::cube(device, 1.0)
}

/// Record one `BasicRenderer::render` into a fresh encoder, submit it and read
/// the target back as RGBA8 (the shared tail of the scene functions below).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_and_read(
    gpu: &HeadlessGpu,
    target: &OffscreenTarget,
    renderer: &BasicRenderer,
    label: &str,
    clear_color: wgpu::Color,
    world: &mut World,
    assets: &RenderAssets,
    view_proj: Mat4,
    cam_pos: Vec3,
    light: LightUniform,
) -> Vec<u8> {
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        clear_color,
        world,
        assets,
        view_proj,
        cam_pos,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
}

/// Render a 3×3 grid of cubes lit by the default directional light, viewed
/// from a slightly elevated angle. Returns RGBA8 (`width*height*4`).
pub fn render_cube_grid(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 64);

    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
    let mat_h = assets.add_material(MaterialUniform::new());
    let bounds = Bounds(AABB::new(
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, 0.5, 0.5),
    ));

    let mut world = World::new();
    for gx in -1..=1 {
        for gz in -1..=1 {
            let e = world.spawn();
            world.add(
                e,
                Transform::from_position(Vec3::new(gx as f32 * 2.0, 0.0, gz as f32 * 2.0)),
            );
            world.add(e, MeshHandle(mesh_h.0));
            world.add(e, MaterialHandle(mat_h.0));
            world.add(e, bounds);
        }
    }

    // Camera looks down and forward at the grid.
    let mut camera = Camera::new(Vec3::new(0.0, 4.0, 8.0), width as f32 / height as f32);
    camera.pitch = -0.45;
    let view_proj = camera.view_projection();
    let light = LightUniform::new();

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture cube_grid",
        wgpu::Color {
            r: 0.1,
            g: 0.12,
            b: 0.18,
            a: 1.0,
        },
        &mut world,
        &assets,
        view_proj,
        camera.position,
        light,
    )
}

/// Render a single large centered cube — the simplest "did anything draw"
/// smoke scene. Returns RGBA8.
pub fn render_single_cube(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);

    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
    let mat_h = assets.add_material(MaterialUniform::new());

    let mut world = World::new();
    let e = world.spawn();
    let mut t = Transform::from_position(Vec3::ZERO);
    t.rotation = Quat::from_euler_yxz(Vec3::new(0.3, 0.6, 0.0));
    world.add(e, t);
    world.add(e, MeshHandle(mesh_h.0));
    world.add(e, MaterialHandle(mat_h.0));

    // Corner-on view so three faces are visible.
    let mut camera = Camera::new(Vec3::new(2.2, 1.8, 2.6), width as f32 / height as f32);
    camera.yaw = -0.7;
    camera.pitch = -0.45;
    let view_proj = camera.view_projection();

    // Asymmetric light so each face receives a distinct diffuse term
    // (the default (0,-1,-1) lights +Y and +Z identically).
    let mut light = LightUniform::new();
    light.direction = [-0.4, -1.0, -0.6];

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture single_cube",
        wgpu::Color {
            r: 0.05,
            g: 0.05,
            b: 0.08,
            a: 1.0,
        },
        &mut world,
        &assets,
        view_proj,
        camera.position,
        light,
    )
}

pub(crate) fn camera_light_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera+Light BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<CameraUniform>() as u64
                    ),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

/// DEBUG: replicate `examples/game_ready_demo` (ground + player + chaser cubes)
/// rendered with the *runtime's* fixed camera (`(0,10,20)`, pitch -0.4) to see
/// what the user actually sees. Returns RGBA8.
pub fn render_demo_like_cam(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    cam_pos: Vec3,
    yaw: f32,
    pitch: f32,
    include_ground: bool,
) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 1024);
    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
    // Distinct per-role colors (exercises BasicRenderer's base_color path).
    let gray = assets.add_material(MaterialUniform::with_params([0.35, 0.37, 0.40, 1.0], 0.0, 1.0));
    let blue = assets.add_material(MaterialUniform::with_params([0.20, 0.50, 1.00, 1.0], 0.0, 1.0));
    let red = assets.add_material(MaterialUniform::with_params([1.00, 0.30, 0.20, 1.0], 0.0, 1.0));
    let mut world = World::new();
    let all = [
        (Vec3::new(0.0, -0.1, 0.0), Vec3::new(40.0, 0.2, 40.0), gray), // ground
        (Vec3::new(0.0, 0.5, 0.0), Vec3::ONE, blue),                   // player
        (Vec3::new(10.0, 0.5, 10.0), Vec3::new(0.8, 0.8, 0.8), red),   // chaser
    ];
    let specs: &[(Vec3, Vec3, MaterialHandle)] = if include_ground { &all } else { &all[1..] };
    for &(pos, scale, mat) in specs {
        let e = world.spawn();
        let mut t = Transform::from_position(pos);
        t.scale = scale;
        world.add(e, t);
        world.add(e, MeshHandle(mesh_h.0));
        world.add(e, MaterialHandle(mat.0));
    }
    voltex_ecs::propagate_transforms(&mut world);
    let mut camera = Camera::new(cam_pos, width as f32 / height as f32);
    camera.yaw = yaw;
    camera.pitch = pitch;
    let view_proj = camera.view_projection();
    let light = LightUniform::new();
    draw_and_read(
        gpu,
        &target,
        &renderer,
        "demo_like_cam",
        wgpu::Color { r: 0.02, g: 0.02, b: 0.05, a: 1.0 },
        &mut world,
        &assets,
        view_proj,
        camera.position,
        light,
    )
}

/// Render one white cube at world `depth` (more negative = farther from the
/// camera at z=8) under either an orthographic or perspective projection.
/// Returns RGBA8. Used to show that orthographic footprints are
/// depth-independent while perspective foreshortens.
pub fn render_cube_at_depth(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    ortho: bool,
    depth: f32,
) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();
    let cube = assets.add_mesh(unit_cube(&gpu.device));
    let mat = assets.add_material(MaterialUniform::with_params([1.0, 1.0, 1.0, 1.0], 0.0, 1.0));

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::new(0.0, 0.0, depth)));
    world.add(e, MeshHandle(cube.0));
    world.add(e, MaterialHandle(mat.0));
    voltex_ecs::propagate_transforms(&mut world);

    let aspect = width as f32 / height as f32;
    let camera = if ortho {
        Camera::orthographic(Vec3::new(0.0, 0.0, 8.0), aspect, 10.0)
    } else {
        Camera::new(Vec3::new(0.0, 0.0, 8.0), aspect)
    };
    let mut light = LightUniform::new();
    light.direction = [0.2, -0.3, -1.0];
    light.ambient_strength = 0.3;

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture proj",
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}

/// Render only the given debug line segments (no meshes) on a black background,
/// viewed by a fixed front camera. Exercises `BasicRenderer::set_debug_lines` +
/// the line pipeline. Returns RGBA8.
pub fn render_debug_lines(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    verts: &[voltex_renderer::LineVertex],
) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let assets = RenderAssets::new();
    let mut world = World::new();

    let camera = Camera::new(Vec3::new(0.0, 0.0, 4.0), width as f32 / height as f32);
    let light = LightUniform::new();

    renderer.set_debug_lines(&gpu.queue, verts);
    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture lines",
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}

#[cfg(test)]
mod tests;
