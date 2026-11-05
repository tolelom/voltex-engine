//! PBR scenes through `BasicRenderer`'s runtime PBR path (metallic row,
//! normal map). The hand-built pipeline sphere grid is `scene::raw_pbr`.

use voltex_ecs::{MaterialHandle, MeshHandle, Transform, World};
use voltex_math::Vec3;
use voltex_renderer::{generate_sphere_mesh, Camera, LightUniform, MaterialUniform};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};

use super::draw_and_read;
use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// Render a row of 3 spheres (metallic 0.0 / 0.5 / 1.0, fixed roughness)
/// through `BasicRenderer` with PBR `pbr` on/off, under one directional light.
/// Exercises the runtime PBR path end-to-end. Returns RGBA8.
pub fn render_pbr_basic_renderer(gpu: &HeadlessGpu, width: u32, height: u32, pbr: bool) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let mut renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    renderer.set_pbr_enabled(pbr);
    let mut assets = RenderAssets::new();
    let mesh = assets.add_mesh(generate_sphere_mesh(0.8, 32, 16).upload(&gpu.device));

    let mut world = World::new();
    for (i, metallic) in [0.0f32, 0.5, 1.0].into_iter().enumerate() {
        let mat = assets.add_material(MaterialUniform::with_params(
            [0.85, 0.20, 0.15, 1.0],
            metallic,
            0.3,
        ));
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(i as f32 * 2.0 - 2.0, 0.0, 0.0)));
        world.add(e, MeshHandle(mesh.0));
        world.add(e, MaterialHandle(mat.0));
    }
    voltex_ecs::propagate_transforms(&mut world);

    let camera = Camera::new(Vec3::new(0.0, 0.0, 6.0), width as f32 / height as f32);
    let mut light = LightUniform::new();
    light.direction = [-0.4, -0.5, -1.0];
    light.color = [1.0, 1.0, 1.0];
    light.ambient_strength = 0.1;

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture pbr-br",
        wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}

/// Render a single PBR sphere; when `normal_map` is set, attach a non-flat
/// normal map via `PbrTextures` (uploaded linear). Used to prove the normal-map
/// slot perturbs shading. Returns RGBA8.
pub fn render_pbr_sphere_normalmap(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    normal_map: bool,
) -> Vec<u8> {
    use voltex_renderer_ecs::PbrTextures;
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let mut renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    renderer.set_pbr_enabled(true);
    let mut assets = RenderAssets::new();
    let mesh = assets.add_mesh(generate_sphere_mesh(1.2, 48, 24).upload(&gpu.device));
    let mat = assets.add_material(MaterialUniform::with_params([0.7, 0.7, 0.7, 1.0], 0.1, 0.5));

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::ZERO));
    world.add(e, MeshHandle(mesh.0));
    world.add(e, MaterialHandle(mat.0));
    if normal_map {
        // 4×4 strongly-tilted normals (linear): xy pushed far off-axis.
        let mut px = Vec::new();
        for i in 0..16 {
            let tilt = if i % 2 == 0 { 30u8 } else { 225 };
            px.extend_from_slice(&[tilt, 255 - tilt, 200, 255]);
        }
        let nh = assets.add_texture_rgba_linear(&gpu.device, &gpu.queue, 4, 4, &px);
        world.add(e, PbrTextures { normal: Some(nh), ..Default::default() });
    }
    voltex_ecs::propagate_transforms(&mut world);

    let camera = Camera::new(Vec3::new(0.0, 0.0, 4.0), width as f32 / height as f32);
    let mut light = LightUniform::new();
    light.direction = [-0.3, -0.4, -1.0];
    light.ambient_strength = 0.1;

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture pbr-nm",
        wgpu::Color { r: 0.02, g: 0.02, b: 0.04, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}
