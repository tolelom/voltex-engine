//! Scenes built from *declarative* renderables (`PrimitiveMesh` +
//! `MaterialDesc`), materialized the way the runtime does at `gpu_init`.

use voltex_ecs::{Transform, World};
use voltex_math::{Quat, Vec3};
use voltex_renderer::{Camera, LightUniform};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};

use super::draw_and_read;
use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// Render a scene built entirely through the *declarative* renderable path:
/// entities carry only `PrimitiveMesh` + `MaterialDesc` (no device, no
/// `MeshHandle`/`MaterialHandle`), and [`materialize_renderables`] resolves
/// them into GPU meshes + materials before drawing — exactly what
/// `voltex_runtime` does at `gpu_init`. Proves the one-call "spawn a visible
/// object" path actually rasterizes. Returns RGBA8.
///
/// [`materialize_renderables`]: voltex_renderer_ecs::materialize_renderables
pub fn render_materialized_primitives(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    use voltex_renderer_ecs::{materialize_renderables, MaterialDesc, PrimitiveMesh};

    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 16);
    let mut assets = RenderAssets::new();
    let mut world = World::new();

    // A red cube and a green sphere — declared purely by value.
    let cube = world.spawn();
    let mut ct = Transform::from_position(Vec3::new(-0.85, 0.0, 0.0));
    ct.rotation = Quat::from_euler_yxz(Vec3::new(0.3, 0.6, 0.0));
    world.add(cube, ct);
    world.add(cube, PrimitiveMesh::cube(1.0));
    world.add(cube, MaterialDesc::color([0.9, 0.2, 0.15, 1.0]));

    let sphere = world.spawn();
    world.add(sphere, Transform::from_position(Vec3::new(0.85, 0.0, 0.0)));
    world.add(sphere, PrimitiveMesh::sphere_default(0.6));
    world.add(sphere, MaterialDesc::color([0.2, 0.8, 0.3, 1.0]));

    // The single call the runtime makes for the game at gpu_init.
    materialize_renderables(&mut world, &mut assets, &gpu.device);

    let mut camera = Camera::new(Vec3::new(0.0, 1.0, 4.0), width as f32 / height as f32);
    camera.pitch = -0.22;
    let view_proj = camera.view_projection();
    let mut light = LightUniform::new();
    light.direction = [-0.4, -1.0, -0.6];

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture materialized_primitives",
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

/// Render a scene authored as **data**: a world of `PrimitiveMesh` +
/// `MaterialDesc` entities is serialized to a JSON scene string, then loaded
/// into a *fresh* world purely from that string (no spawn code), materialized,
/// and drawn. Proves the full "data scene → pixels" path — the same one a
/// `.vscn`/JSON file on disk drives. Returns RGBA8.
pub fn render_data_authored_scene(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    use voltex_ecs::{
        deserialize_scene_json, serialize_scene_json, ComponentRegistry,
    };
    use voltex_renderer_ecs::{
        materialize_renderables, register_renderable_components, MaterialDesc, PrimitiveMesh,
    };

    // Registry mirrors RuntimeApp boot: built-ins + declarative renderables.
    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    register_renderable_components(&mut registry);

    // Author in code, then freeze to the on-disk JSON format.
    let mut authored = World::new();
    let c = authored.spawn();
    let mut ct = Transform::from_position(Vec3::new(-0.85, 0.0, 0.0));
    ct.rotation = Quat::from_euler_yxz(Vec3::new(0.3, 0.6, 0.0));
    authored.add(c, ct);
    authored.add(c, PrimitiveMesh::cube(1.0));
    authored.add(c, MaterialDesc::color([0.9, 0.2, 0.15, 1.0]));
    let s = authored.spawn();
    authored.add(s, Transform::from_position(Vec3::new(0.85, 0.0, 0.0)));
    authored.add(s, PrimitiveMesh::sphere_default(0.6));
    authored.add(s, MaterialDesc::color([0.2, 0.8, 0.3, 1.0]));
    let json = serialize_scene_json(&authored, &registry);

    // Load a brand-new world from the data alone.
    let mut world = World::new();
    deserialize_scene_json(&mut world, &json, &registry).expect("scene JSON loads");

    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 16);
    let mut assets = RenderAssets::new();
    materialize_renderables(&mut world, &mut assets, &gpu.device);

    let mut camera = Camera::new(Vec3::new(0.0, 1.0, 4.0), width as f32 / height as f32);
    camera.pitch = -0.22;
    let view_proj = camera.view_projection();
    let mut light = LightUniform::new();
    light.direction = [-0.4, -1.0, -0.6];

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture data_authored_scene",
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

/// Render a top-down boids-style swarm: many small, randomly-placed,
/// individually-colored cubes declared with `PrimitiveMesh` + `MaterialDesc`
/// (seeded by `Rng`), materialized, and drawn from above — the visual of
/// `examples/boids_demo` (steady-state flocking is exercised by voltex_ai's
/// unit tests; this validates the declarative render at swarm scale). RGBA8.
pub fn render_boids_swarm(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    use voltex_math::Rng;
    use voltex_renderer_ecs::{materialize_renderables, MaterialDesc, PrimitiveMesh};

    const ARENA: f32 = 14.0;
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 256);
    let mut assets = RenderAssets::new();
    let mut world = World::new();
    let mut rng = Rng::seeded(0xB0_1D5);

    let mut ground = Transform::from_position(Vec3::new(0.0, -0.2, 0.0));
    ground.scale = Vec3::new(ARENA * 2.2, 0.2, ARENA * 2.2);
    world.spawn_bundle((
        ground,
        PrimitiveMesh::cube(1.0),
        MaterialDesc::color([0.12, 0.13, 0.16, 1.0]),
    ));
    for _ in 0..120 {
        let pos = Vec3::new(rng.range_f32(-ARENA, ARENA), 0.5, rng.range_f32(-ARENA, ARENA));
        let color = [
            rng.range_f32(0.5, 1.0),
            rng.range_f32(0.4, 0.8),
            rng.range_f32(0.2, 0.5),
            1.0,
        ];
        world.spawn_bundle((
            Transform::from_position(pos),
            PrimitiveMesh::cube(0.5),
            MaterialDesc::color(color),
        ));
    }
    materialize_renderables(&mut world, &mut assets, &gpu.device);

    let mut camera = Camera::new(Vec3::new(0.0, 36.0, 0.01), width as f32 / height as f32);
    camera.pitch = -std::f32::consts::FRAC_PI_2;
    let view_proj = camera.view_projection();
    let mut light = LightUniform::new();
    light.direction = [-0.3, -1.0, -0.2];

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture boids_swarm",
        wgpu::Color { r: 0.02, g: 0.02, b: 0.04, a: 1.0 },
        &mut world,
        &assets,
        view_proj,
        camera.position,
        light,
    )
}

/// Render an arbitrary caller-built `World` of *declarative* renderables
/// (`PrimitiveMesh` + `MaterialDesc` entities): materialize → `BasicRenderer`
/// → RGBA8. The public entry point for game crates to pixel-verify their own
/// scenes (added for depths dogfooding).
///
/// Fixed contract (not parameters — parameterize when a second game needs it):
/// - Light: one asymmetric directional, direction `[-0.4, -1.0, -0.6]`
///   (`LightUniform::new()` defaults otherwise).
/// - Camera: perspective, default FOV 45° (`FRAC_PI_4`), near 0.1 / far 100;
///   looks from `cam_pos` with the given yaw/pitch (radians).
/// - Capacity: `max_entities = 1024` — entities beyond the cap are
///   **silently dropped** from the frame.
pub fn render_world(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    world: &mut World,
    cam_pos: Vec3,
    cam_yaw: f32,
    cam_pitch: f32,
) -> Vec<u8> {
    render_world_impl(gpu, width, height, world, cam_pos, cam_yaw, cam_pitch, None)
}

/// [`render_world`] with an **orthographic** camera (view height `ortho_height`
/// world units, width = height × aspect) — matches the runtime's
/// `RuntimeConfig::with_orthographic`, so 2D games can pixel-verify the same
/// framing the player sees. Same fixed light/capacity contract as
/// [`render_world`].
#[allow(clippy::too_many_arguments)]
pub fn render_world_ortho(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    world: &mut World,
    cam_pos: Vec3,
    cam_yaw: f32,
    cam_pitch: f32,
    ortho_height: f32,
) -> Vec<u8> {
    render_world_impl(
        gpu,
        width,
        height,
        world,
        cam_pos,
        cam_yaw,
        cam_pitch,
        Some(ortho_height),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_world_impl(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    world: &mut World,
    cam_pos: Vec3,
    cam_yaw: f32,
    cam_pitch: f32,
    ortho_height: Option<f32>,
) -> Vec<u8> {
    use voltex_renderer_ecs::materialize_renderables;

    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 1024);
    let mut assets = RenderAssets::new();
    materialize_renderables(world, &mut assets, &gpu.device);

    let mut camera = Camera::new(cam_pos, width as f32 / height as f32);
    camera.yaw = cam_yaw;
    camera.pitch = cam_pitch;
    camera.ortho_height = ortho_height;
    let view_proj = camera.view_projection();
    let mut light = LightUniform::new();
    light.direction = [-0.4, -1.0, -0.6];

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture render_world",
        wgpu::Color {
            r: 0.05,
            g: 0.05,
            b: 0.08,
            a: 1.0,
        },
        world,
        &assets,
        view_proj,
        camera.position,
        light,
    )
}

/// Render `survivor`'s seed scene through the *declarative* path it now
/// uses (v1.0.70): a gray ground, a green player, and a red enemy declared with
/// `PrimitiveMesh` + `MaterialDesc`, materialized, and drawn with the runtime's
/// elevated camera. Proves the flagship's per-object colors survive
/// materialize→render (they were all white before the modernization). RGBA8.
pub fn render_survivor_seed_scene(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    use voltex_renderer_ecs::{MaterialDesc, PrimitiveMesh};

    let mut world = World::new();

    // Mirror survivor::setup_scene roles (gray ground, green player) plus
    // a runtime-spawned-style red enemy.
    let mut ground_t = Transform::from_position(Vec3::new(0.0, -0.5, 0.0));
    ground_t.scale = Vec3::new(20.0, 0.1, 20.0);
    world.spawn_bundle((
        ground_t,
        PrimitiveMesh::cube(1.0),
        MaterialDesc::color([0.30, 0.32, 0.35, 1.0]),
    ));
    world.spawn_bundle((
        Transform::from_position(Vec3::new(0.0, 0.5, 0.0)),
        PrimitiveMesh::cube(1.0),
        MaterialDesc::color([0.20, 0.80, 0.30, 1.0]),
    ));
    world.spawn_bundle((
        Transform::from_position(Vec3::new(2.0, 0.5, 0.0)),
        PrimitiveMesh::cube(1.0),
        MaterialDesc::color([0.90, 0.25, 0.20, 1.0]),
    ));

    render_world(gpu, width, height, &mut world, Vec3::new(0.0, 4.0, 8.0), 0.0, -0.45)
}
