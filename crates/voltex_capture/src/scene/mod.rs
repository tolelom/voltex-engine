//! Reusable demo scenes rendered through the *real* ECS render path
//! (`voltex_renderer_ecs::BasicRenderer`). Each returns a tightly-packed
//! RGBA8 buffer so a bin can save a PNG and a (GPU-gated) test can assert the
//! frame isn't blank.

use voltex_ecs::{Bounds, MaterialHandle, MeshHandle, Transform, World};
use voltex_math::{Mat4, Quat, Vec3, AABB};
use voltex_renderer::{
    create_pbr_full_texture_bind_group, create_pbr_pipeline, generate_sphere,
    pbr_full_texture_bind_group_layout, Camera, CameraUniform, GpuTexture, IblResources, LightData,
    LightUniform, LightsUniform, MaterialUniform, Mesh, MeshVertex, ShadowMap, ShadowUniform,
};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};
use wgpu::util::DeviceExt;

use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

pub use crate::scene_hot_reload::{render_mesh_hot_reload, render_texture_hot_reload};
pub use crate::scene_sprite::{render_billboard_sprite, render_particle_burst, render_sprite_sheet_frame};
pub use crate::scene_texture::{
    render_cube_with_png, render_cube_with_texture, render_obj_model, render_textured_cube,
};

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
/// scenes (added for voltex_depths dogfooding).
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

/// Render `voltex_survivor`'s seed scene through the *declarative* path it now
/// uses (v1.0.70): a gray ground, a green player, and a red enemy declared with
/// `PrimitiveMesh` + `MaterialDesc`, materialized, and drawn with the runtime's
/// elevated camera. Proves the flagship's per-object colors survive
/// materialize→render (they were all white before the modernization). RGBA8.
pub fn render_declarative_colored_scene(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    use voltex_renderer_ecs::{MaterialDesc, PrimitiveMesh};

    let mut world = World::new();

    // Mirror voltex_survivor::setup_scene roles (gray ground, green player) plus
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

/// Render the canonical PBR test: a 7×7 grid of spheres with metallic varying
/// left→right and roughness varying bottom→top, lit by one directional light.
/// Exercises the full Cook-Torrance pipeline (`create_pbr_pipeline`) plus the
/// IBL / shadow bind groups. Returns RGBA8.
pub fn render_pbr_grid(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    const GRID: usize = 7;
    const NUM: usize = GRID * GRID;
    const SPACING: f32 = 1.2;

    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);

    let alignment = device.limits().min_uniform_buffer_offset_alignment;
    let cam_aligned = align_up(std::mem::size_of::<CameraUniform>() as u32, alignment);
    let mat_aligned = align_up(std::mem::size_of::<MaterialUniform>() as u32, alignment);

    let (vertices, indices) = generate_sphere(0.4, 32, 16);
    let mesh = Mesh::new(device, &vertices, &indices);

    let camera = Camera::new(Vec3::new(0.0, 0.0, 12.0), width as f32 / height as f32);
    let view_proj = camera.view_projection();
    let cam_pos = [camera.position.x, camera.position.y, camera.position.z];

    let mut lights = LightsUniform::new();
    lights.add_light(LightData::directional(
        [-1.0, -1.0, -1.0],
        [1.0, 1.0, 1.0],
        1.0,
    ));

    let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("PBR Camera UBO"),
        size: (cam_aligned as usize * NUM) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("PBR Light UBO"),
        contents: bytemuck::cast_slice(&[lights]),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("PBR Material UBO"),
        size: (mat_aligned as usize * NUM) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let cl_layout = camera_light_bgl(device);
    let pbr_tex_layout = pbr_full_texture_bind_group_layout(device);
    let mat_layout = MaterialUniform::bind_group_layout(device);

    let camera_light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("PBR Camera+Light BG"),
        layout: &cl_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &camera_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<CameraUniform>() as u64),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: light_buffer.as_entire_binding(),
            },
        ],
    });

    let old_tex_layout = GpuTexture::bind_group_layout(device);
    let albedo = GpuTexture::white_1x1(device, queue, &old_tex_layout);
    let normal = GpuTexture::flat_normal_1x1(device, queue);
    let orm = GpuTexture::white_1x1(device, queue, &old_tex_layout);
    let emissive = GpuTexture::black_1x1(device, queue, &old_tex_layout);
    let pbr_texture_bind_group = create_pbr_full_texture_bind_group(
        device,
        &pbr_tex_layout,
        &albedo.view,
        &albedo.sampler,
        &normal.1,
        &normal.2,
        &orm.view,
        &orm.sampler,
        &emissive.view,
        &emissive.sampler,
    );

    let ibl = IblResources::new(device, queue);

    let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("PBR Material BG"),
        layout: &mat_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &material_buffer,
                offset: 0,
                size: wgpu::BufferSize::new(std::mem::size_of::<MaterialUniform>() as u64),
            }),
        }],
    });

    let shadow_map = ShadowMap::new(device);
    let shadow_layout = ShadowMap::bind_group_layout(device);
    let shadow_uniform = ShadowUniform {
        light_view_proj: [[0.0; 4]; 4],
        shadow_map_size: 0.0,
        shadow_bias: 0.0,
        _padding: [0.0; 2],
        sun_direction: [0.5, -0.7, 0.5],
        turbidity: 3.0,
        sh_coefficients: [[0.0; 4]; 7],
    };
    let shadow_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("PBR Shadow UBO"),
        contents: bytemuck::cast_slice(&[shadow_uniform]),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let shadow_bind_group = shadow_map.create_bind_group(
        device,
        &shadow_layout,
        &shadow_uniform_buffer,
        &ibl.brdf_lut_view,
        &ibl.brdf_lut_sampler,
    );

    let pipeline = create_pbr_pipeline(
        device,
        CAPTURE_FORMAT,
        &cl_layout,
        &pbr_tex_layout,
        &mat_layout,
        &shadow_layout,
    );

    // Per-sphere camera + material staging.
    let mut cam_staging = vec![0u8; NUM * cam_aligned as usize];
    let mut mat_staging = vec![0u8; NUM * mat_aligned as usize];
    let half_grid = (GRID as f32 - 1.0) * SPACING * 0.5;
    for row in 0..GRID {
        for col in 0..GRID {
            let i = row * GRID + col;
            let x = col as f32 * SPACING - half_grid;
            let y = row as f32 * SPACING - half_grid;
            let cam_uniform = CameraUniform {
                view_proj: view_proj.cols,
                model: Mat4::translation(x, y, 0.0).cols,
                camera_pos: cam_pos,
                _padding: 0.0,
            };
            let o = i * cam_aligned as usize;
            cam_staging[o..o + std::mem::size_of::<CameraUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&cam_uniform));

            let metallic = col as f32 / (GRID as f32 - 1.0);
            let roughness = 0.05 + row as f32 * (0.95 / (GRID as f32 - 1.0));
            let mat_uniform =
                MaterialUniform::with_params([0.8, 0.2, 0.2, 1.0], metallic, roughness);
            let o = i * mat_aligned as usize;
            mat_staging[o..o + std::mem::size_of::<MaterialUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&mat_uniform));
        }
    }
    queue.write_buffer(&camera_buffer, 0, &cam_staging);
    queue.write_buffer(&material_buffer, 0, &mat_staging);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("PBR capture"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("PBR Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.15,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(1, &pbr_texture_bind_group, &[]);
        pass.set_bind_group(3, &shadow_bind_group, &[]);
        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        for i in 0..NUM {
            pass.set_bind_group(0, &camera_light_bind_group, &[(i as u32) * cam_aligned]);
            pass.set_bind_group(2, &material_bind_group, &[(i as u32) * mat_aligned]);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }
    queue.submit(Some(encoder.finish()));

    target.read_rgba8(device, queue)
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

/// Render a ground slab + a floating occluder cube lit by one angled
/// directional light, with shadow mapping `shadows` on/off. With shadows on,
/// the occluder casts a shadow onto the ground. Returns RGBA8.
pub fn render_shadow_demo(gpu: &HeadlessGpu, width: u32, height: u32, shadows: bool) -> Vec<u8> {
    let mut renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    renderer.set_shadows(shadows);
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let mut assets = RenderAssets::new();
    let cube = assets.add_mesh(unit_cube(&gpu.device));
    let gray = assets.add_material(MaterialUniform::with_params([0.7, 0.7, 0.7, 1.0], 0.0, 1.0));
    let occ = assets.add_material(MaterialUniform::with_params([0.8, 0.5, 0.3, 1.0], 0.0, 1.0));

    // Up-facing ground quad (correct +Y normal + CCW-from-above winding) so the
    // overhead light actually lits it and the shadow is visible.
    let n = [0.0f32, 1.0, 0.0];
    let mk = |x: f32, z: f32, u: f32, v: f32| MeshVertex {
        position: [x, 0.0, z],
        normal: n,
        uv: [u, v],
        tangent: [1.0, 0.0, 0.0, 1.0],
    };
    let gverts = [
        mk(-12.0, -12.0, 0.0, 0.0),
        mk(12.0, -12.0, 1.0, 0.0),
        mk(12.0, 12.0, 1.0, 1.0),
        mk(-12.0, 12.0, 0.0, 1.0),
    ];
    let gidx = [0u32, 2, 1, 0, 3, 2];
    let ground_mesh = assets.add_mesh(voltex_renderer::Mesh::new(&gpu.device, &gverts, &gidx));

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
    world.add(occluder, MaterialHandle(occ.0));
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

/// Render a white cube under a dim directional light + the given additive
/// point lights (via `BasicRenderer::set_point_lights`). Returns RGBA8.
pub fn render_cube_point_lit(
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
    let (sv, si) = voltex_renderer::generate_sphere(1.0, 32, 16);
    let mesh_h = assets.add_mesh(voltex_renderer::Mesh::new(&gpu.device, &sv, &si));
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

    // Up-facing ground quad (+Y normal, CCW-from-above) so an overhead spot lits
    // it via N·L. Spans the full view so the cone footprint sits inside it.
    let n = [0.0f32, 1.0, 0.0];
    let mk = |x: f32, z: f32| MeshVertex {
        position: [x, 0.0, z],
        normal: n,
        uv: [0.0, 0.0],
        tangent: [1.0, 0.0, 0.0, 1.0],
    };
    let gverts = [
        mk(-10.0, -10.0),
        mk(10.0, -10.0),
        mk(10.0, 10.0),
        mk(-10.0, 10.0),
    ];
    let gidx = [0u32, 2, 1, 0, 3, 2];
    let mesh_h = assets.add_mesh(voltex_renderer::Mesh::new(&gpu.device, &gverts, &gidx));
    let mat_h = assets.add_material(MaterialUniform::with_params([1.0, 1.0, 1.0, 1.0], 0.0, 1.0));

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

/// Render a row of 3 spheres (metallic 0.0 / 0.5 / 1.0, fixed roughness)
/// through `BasicRenderer` with PBR `pbr` on/off, under one directional light.
/// Exercises the runtime PBR path end-to-end. Returns RGBA8.
pub fn render_pbr_basic_renderer(gpu: &HeadlessGpu, width: u32, height: u32, pbr: bool) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let mut renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    renderer.set_pbr_enabled(pbr);
    let mut assets = RenderAssets::new();
    let (sv, si) = generate_sphere(0.8, 32, 16);
    let mesh = assets.add_mesh(Mesh::new(&gpu.device, &sv, &si));

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
    let (sv, si) = generate_sphere(1.2, 48, 24);
    let mesh = assets.add_mesh(Mesh::new(&gpu.device, &sv, &si));
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
