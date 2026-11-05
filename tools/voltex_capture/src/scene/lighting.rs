//! Shadow-mapping and point/spot-light scenes rendered through
//! `BasicRenderer`.

use voltex_ecs::{MaterialHandle, MeshHandle, Transform, World};
use voltex_math::Vec3;
use voltex_renderer::{Camera, LightUniform, MaterialUniform};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};

use super::{draw_and_read, ground_quad_mesh, matte, unit_cube};
use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// Render a ground slab + a floating occluder cube lit by one angled
/// directional light, with shadow mapping `shadows` on/off. With shadows on,
/// the occluder casts a shadow onto the ground. Returns RGBA8.
pub fn render_shadow_demo(gpu: &HeadlessGpu, width: u32, height: u32, shadows: bool) -> Vec<u8> {
    let mut renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    renderer.set_shadows(shadows);
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let mut assets = RenderAssets::new();
    let cube = assets.add_mesh(unit_cube(&gpu.device));
    let gray = assets.add_material(matte([0.7, 0.7, 0.7, 1.0]));
    let occluder_material = assets.add_material(matte([0.8, 0.5, 0.3, 1.0]));

    // Up-facing ground quad so the overhead light lights it and the shadow is visible.
    let ground_mesh = assets.add_mesh(ground_quad_mesh(&gpu.device, 12.0, 1.0));

    let mut world = World::new();
    let ground = world.spawn();
    world.add(ground, Transform::from_position(Vec3::ZERO));
    world.add(ground, MeshHandle(ground_mesh.0));
    world.add(ground, MaterialHandle(gray.0));
    // Floating occluder.
    let occluder = world.spawn();
    let mut ot = Transform::from_position(Vec3::new(0.0, 2.5, 0.0));
    ot.scale = Vec3::new(1.5, 1.5, 1.5);
    world.add(occluder, ot);
    world.add(occluder, MeshHandle(cube.0));
    world.add(occluder, MaterialHandle(occluder_material.0));
    voltex_ecs::propagate_transforms(&mut world);

    let mut camera = Camera::new(Vec3::new(7.0, 7.0, 7.0), width as f32 / height as f32);
    camera.yaw = -0.785;
    camera.pitch = -0.615;
    let mut light = LightUniform::new();
    light.direction = [-0.3, -1.0, -0.25];
    light.color = [1.0, 1.0, 1.0];
    light.ambient_strength = 0.12;

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "shadow demo",
        wgpu::Color { r: 0.1, g: 0.1, b: 0.15, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}

/// Render a white sphere under a dim directional light + the given additive
/// point lights (via `BasicRenderer::set_point_lights`). Returns RGBA8.
pub fn render_sphere_point_lit(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    lights: &[voltex_renderer::PointLight],
) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();
    // Sphere — radial outward normals, so the hemisphere facing a light has an
    // unambiguous positive N·L (no winding/normal quirks).
    let mesh_h = assets.add_mesh(voltex_renderer::generate_sphere_mesh(1.0, 32, 16).upload(&gpu.device));
    let mat_h = assets.add_material(MaterialUniform::new());

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::ZERO));
    world.add(e, MeshHandle(mesh_h.0));
    world.add(e, MaterialHandle(mat_h.0));
    voltex_ecs::propagate_transforms(&mut world);

    let camera = Camera::new(Vec3::new(0.0, 0.0, 4.0), width as f32 / height as f32);
    // Moderate directional with headroom so a point light measurably brightens.
    let mut light = LightUniform::new();
    light.direction = [0.0, 0.0, -1.0];
    light.color = [0.30, 0.30, 0.30];
    light.ambient_strength = 0.08;

    renderer.set_point_lights(&gpu.queue, lights);
    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture plight",
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}

/// Render a large up-facing ground plane viewed from straight above, lit only
/// by the given point/spot lights (the directional light is nearly off). With a
/// downward-aimed spotlight this produces a bright circular cone footprint in
/// the center with dark corners outside the cone. Returns RGBA8.
pub fn render_spot_lit(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    lights: &[voltex_renderer::PointLight],
) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();

    // Up-facing ground quad so an overhead spot lights it via N·L. Spans the
    // full view so the cone footprint sits inside it.
    let mesh_h = assets.add_mesh(ground_quad_mesh(&gpu.device, 10.0, 0.0));
    let mat_h = assets.add_material(matte([1.0, 1.0, 1.0, 1.0]));

    let mut world = World::new();
    let ground = world.spawn();
    world.add(ground, Transform::from_position(Vec3::ZERO));
    world.add(ground, MeshHandle(mesh_h.0));
    world.add(ground, MaterialHandle(mat_h.0));
    voltex_ecs::propagate_transforms(&mut world);

    // Top-down camera looking straight at the ground.
    let mut camera = Camera::new(Vec3::new(0.0, 8.0, 0.0), width as f32 / height as f32);
    camera.yaw = 0.0;
    camera.pitch = -std::f32::consts::FRAC_PI_2 + 0.001; // look ~straight down
    // Directional light nearly off so the spot dominates what we measure.
    let mut light = LightUniform::new();
    light.direction = [0.0, -1.0, 0.0];
    light.color = [0.02, 0.02, 0.02];
    light.ambient_strength = 0.02;

    renderer.set_point_lights(&gpu.queue, lights);
    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture spot",
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}
