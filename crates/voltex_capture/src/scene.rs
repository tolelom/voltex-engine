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

fn align_up(size: u32, alignment: u32) -> u32 {
    size.div_ceil(alignment) * alignment
}

/// Build a unit cube mesh (24 verts, 36 indices, outward normals).
pub(crate) fn unit_cube(device: &wgpu::Device) -> Mesh {
    Mesh::cube(device, 1.0)
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

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("capture cube_grid"),
        });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
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
    );
    gpu.queue.submit(Some(encoder.finish()));

    target.read_rgba8(&gpu.device, &gpu.queue)
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

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("capture single_cube"),
        });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
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
    );
    gpu.queue.submit(Some(encoder.finish()));

    target.read_rgba8(&gpu.device, &gpu.queue)
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

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("capture materialized_primitives"),
        });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
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
    );
    gpu.queue.submit(Some(encoder.finish()));

    target.read_rgba8(&gpu.device, &gpu.queue)
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
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("demo_like_cam") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.02, g: 0.02, b: 0.05, a: 1.0 },
        &mut world,
        &assets,
        view_proj,
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
}

/// Render a single cube with a pre-built [`GpuTexture`] via a runtime
/// `TextureHandle` + `BasicRenderer`. Proves per-entity textures are sampled.
pub fn render_cube_with_texture(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    tex: voltex_renderer::GpuTexture,
) -> Vec<u8> {
    use voltex_renderer_ecs::TextureHandle;

    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
    let mat_h = assets.add_material(MaterialUniform::new()); // white → texture shows through
    let tex_h: TextureHandle = assets.add_texture(tex);

    let mut world = World::new();
    let e = world.spawn();
    let mut t = Transform::from_position(Vec3::ZERO);
    t.rotation = Quat::from_euler_yxz(Vec3::new(0.3, 0.6, 0.0));
    world.add(e, t);
    world.add(e, MeshHandle(mesh_h.0));
    world.add(e, MaterialHandle(mat_h.0));
    world.add(e, tex_h);
    voltex_ecs::propagate_transforms(&mut world);

    let mut camera = Camera::new(Vec3::new(2.0, 1.5, 2.5), width as f32 / height as f32);
    camera.yaw = -0.7;
    camera.pitch = -0.4;

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture tex") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        LightUniform::new(),
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
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

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("shadow demo") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.1, g: 0.1, b: 0.15, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
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
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture plight") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
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
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture spot") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
}

/// Spawn an orange particle burst at the origin, advance it `ticks` times by
/// `dt`, then render the surviving particles (as billboard sprites) from a
/// top-down orthographic camera. Exercises particle update → sprite render.
/// Returns RGBA8.
pub fn render_particle_burst(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    ticks: u32,
    dt: f32,
) -> Vec<u8> {
    use voltex_renderer_ecs::{spawn_particle, update_particles, Particle};
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let assets = RenderAssets::new();

    let mut world = World::new();
    let n = 16;
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        // Spread in the XY (screen) plane so a front camera sees the whole fan.
        let vel = Vec3::new(a.cos() * 3.0, a.sin() * 3.0, 0.0);
        let part = Particle::new(vel, 2.0, 0.4);
        spawn_particle(&mut world, Vec3::ZERO, [1.0, 0.55, 0.1, 1.0], part);
    }
    for _ in 0..ticks {
        update_particles(&mut world, dt);
    }
    voltex_ecs::propagate_transforms(&mut world);

    // Front-facing ortho (looks -Z, up +Y) so the radial spread fills the view.
    let camera =
        Camera::orthographic(Vec3::new(0.0, 0.0, 12.0), width as f32 / height as f32, 16.0);
    let light = LightUniform::new();

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture parts") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
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

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture pbr-br") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
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

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture pbr-nm") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.02, g: 0.02, b: 0.04, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
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

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture proj") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
}

/// Render a textured billboard sampling `frame` of a 2×2 sprite sheet. The
/// atlas is a 64×64 four-color grid (TL red, TR green, BL blue, BR yellow), so
/// each frame fills the sprite with one solid color — verifying `uv_rect`
/// sub-rect sampling. Returns RGBA8.
pub fn render_sprite_sheet_frame(gpu: &HeadlessGpu, width: u32, height: u32, frame: u32) -> Vec<u8> {
    use voltex_renderer::GpuTexture;
    use voltex_renderer_ecs::Sprite;

    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();

    // 64×64 atlas: 2×2 blocks of solid color (row-major, v=0 at top).
    let dim = 64usize;
    let mut pixels = vec![0u8; dim * dim * 4];
    let color_at = |cx: usize, cy: usize| -> [u8; 4] {
        match (cx, cy) {
            (0, 0) => [255, 0, 0, 255],   // TL red
            (1, 0) => [0, 255, 0, 255],   // TR green
            (0, 1) => [0, 0, 255, 255],   // BL blue
            _ => [255, 255, 0, 255],      // BR yellow
        }
    };
    for y in 0..dim {
        for x in 0..dim {
            let c = color_at(x / (dim / 2), y / (dim / 2));
            let o = (y * dim + x) * 4;
            pixels[o..o + 4].copy_from_slice(&c);
        }
    }
    let layout = GpuTexture::bind_group_layout(&gpu.device);
    let tex = GpuTexture::from_rgba(&gpu.device, &gpu.queue, dim as u32, dim as u32, &pixels, &layout);
    let tex_h = assets.add_texture(tex);

    let mut sprite = Sprite::textured(tex_h, [3.0, 3.0]);
    sprite.set_sheet_frame(2, 2, frame);

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::ZERO));
    world.add(e, sprite);
    voltex_ecs::propagate_transforms(&mut world);

    let camera = Camera::orthographic(Vec3::new(0.0, 0.0, 8.0), width as f32 / height as f32, 6.0);
    let light = LightUniform::new();

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture sheet") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
}

/// Render a single magenta billboard [`Sprite`] at the world origin, viewed
/// from `cam_pos` with the given `yaw` (pitch 0). A camera-facing quad should
/// present the same footprint from any orbit angle. Returns RGBA8.
pub fn render_billboard_sprite(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    cam_pos: Vec3,
    yaw: f32,
) -> Vec<u8> {
    use voltex_renderer_ecs::Sprite;
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let assets = RenderAssets::new();

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::ZERO));
    world.add(e, Sprite::solid([1.5, 1.5], [1.0, 0.0, 1.0, 1.0]));
    voltex_ecs::propagate_transforms(&mut world);

    let mut camera = Camera::new(cam_pos, width as f32 / height as f32);
    camera.yaw = yaw;
    camera.pitch = 0.0;
    let light = LightUniform::new();

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture sprite") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
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
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture lines") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
}

/// Render a cube textured with a 2×2 solid `color` (RGBA).
pub fn render_textured_cube(gpu: &HeadlessGpu, width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    use voltex_renderer::GpuTexture;
    let layout = GpuTexture::bind_group_layout(&gpu.device);
    let pixels: Vec<u8> = color.iter().cycle().take(2 * 2 * 4).copied().collect();
    let tex = GpuTexture::from_rgba(&gpu.device, &gpu.queue, 2, 2, &pixels, &layout);
    render_cube_with_texture(gpu, width, height, tex)
}

/// Load a Wavefront OBJ source via `Mesh::from_obj` and render it as a single
/// centered model through `BasicRenderer`. Returns RGBA8.
pub fn render_obj_model(gpu: &HeadlessGpu, width: u32, height: u32, obj_src: &str) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(voltex_renderer::Mesh::from_obj(&gpu.device, obj_src));
    let mat_h = assets.add_material(MaterialUniform::with_params([0.6, 0.7, 0.9, 1.0], 0.0, 0.6));

    let mut world = World::new();
    let e = world.spawn();
    let mut t = Transform::from_position(Vec3::ZERO);
    t.rotation = Quat::from_euler_yxz(Vec3::new(0.3, 0.6, 0.0));
    world.add(e, t);
    world.add(e, MeshHandle(mesh_h.0));
    world.add(e, MaterialHandle(mat_h.0));
    voltex_ecs::propagate_transforms(&mut world);

    let mut camera = Camera::new(Vec3::new(2.0, 1.5, 2.5), width as f32 / height as f32);
    camera.yaw = -0.7;
    camera.pitch = -0.4;
    let mut light = LightUniform::new();
    light.direction = [-0.4, -1.0, -0.6];

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture obj") });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageStats;

    /// The repo's `assets/cube.obj`, embedded for the loader regression.
    const CUBE_OBJ: &str = include_str!("../../../assets/cube.obj");

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn basic_renderer_samples_per_entity_texture() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // mean (R, B) over non-background pixels.
        fn mean_rb(rgba: &[u8]) -> (f64, f64) {
            let bg = [rgba[0], rgba[1], rgba[2]];
            let mut r = 0u64;
            let mut b = 0u64;
            let mut n = 0u64;
            for p in rgba.chunks_exact(4) {
                if (p[0] as i32 - bg[0] as i32).abs() > 20
                    || (p[2] as i32 - bg[2] as i32).abs() > 20
                {
                    r += p[0] as u64;
                    b += p[2] as u64;
                    n += 1;
                }
            }
            let n = n.max(1) as f64;
            (r as f64 / n, b as f64 / n)
        }

        let red = render_textured_cube(&gpu, 256, 256, [220, 30, 30, 255]);
        let blue = render_textured_cube(&gpu, 256, 256, [30, 30, 220, 255]);
        let (rr, rb) = mean_rb(&red);
        let (br, bb) = mean_rb(&blue);
        assert!(rr > rb, "red-textured cube should be red-dominant: R={rr} B={rb}");
        assert!(bb > br, "blue-textured cube should be blue-dominant: R={br} B={bb}");
    }

    /// Minimal GLB (one triangle, POSITION + indices) — mirrors the
    /// voltex_image parser's own test fixture.
    fn minimal_glb_triangle() -> Vec<u8> {
        let mut bin = Vec::new();
        for &v in &[0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        for &i in &[0u16, 1, 2] {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        bin.extend_from_slice(&[0, 0]); // 4-byte align
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"buffers":[{{"byteLength":{}}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":36}},{{"buffer":0,"byteOffset":36,"byteLength":6}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","max":[1.0,1.0,0.0],"min":[0.0,0.0,0.0]}},{{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}],"meshes":[{{"name":"Tri","primitives":[{{"attributes":{{"POSITION":0}},"indices":1}}]}}]}}"#,
            bin.len()
        );
        let jb = json.into_bytes();
        let jpad = (jb.len() + 3) & !3;
        let mut json_padded = jb;
        while json_padded.len() < jpad {
            json_padded.push(b' ');
        }
        let total = 12 + 8 + json_padded.len() + 8 + bin.len();
        let mut glb = Vec::with_capacity(total);
        glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // glTF magic
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&(total as u32).to_le_bytes());
        glb.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // JSON
        glb.extend_from_slice(&json_padded);
        glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // BIN\0
        glb.extend_from_slice(&bin);
        glb
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn shadows_darken_ground_under_occluder() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let off = render_shadow_demo(&gpu, 512, 512, false);
        let on = render_shadow_demo(&gpu, 512, 512, true);
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/");
        crate::save_png(&format!("{dir}shadow_off.png"), 512, 512, &off).unwrap();
        crate::save_png(&format!("{dir}shadow_on.png"), 512, 512, &on).unwrap();
        // Shadows on → overall darker (occluded ground region) than shadows off.
        let lum = |r: &[u8]| -> u64 {
            r.chunks_exact(4).map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum()
        };
        assert!(lum(&on) < lum(&off), "shadowed frame should be darker overall");
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn hidden_component_skips_rendering() {
        use voltex_renderer_ecs::Hidden;
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let render = |hide: bool| -> Vec<u8> {
            let target = OffscreenTarget::new(&gpu.device, 128, 128);
            let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
            let mut assets = RenderAssets::new();
            let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
            let mat_h = assets.add_material(MaterialUniform::new());
            let mut world = World::new();
            let e = world.spawn();
            world.add(e, Transform::from_position(Vec3::ZERO));
            world.add(e, MeshHandle(mesh_h.0));
            world.add(e, MaterialHandle(mat_h.0));
            if hide {
                world.add(e, Hidden);
            }
            voltex_ecs::propagate_transforms(&mut world);
            let mut camera = Camera::new(Vec3::new(2.0, 1.5, 2.5), 1.0);
            camera.yaw = -0.7;
            camera.pitch = -0.4;
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            renderer.render(
                &gpu.queue,
                &mut encoder,
                &target.color_view,
                &target.depth_view,
                wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
                &mut world,
                &assets,
                camera.view_projection(),
                camera.position,
                LightUniform::new(),
            );
            gpu.queue.submit(Some(encoder.finish()));
            target.read_rgba8(&gpu.device, &gpu.queue)
        };
        assert!(!ImageStats::analyze(&render(false)).looks_blank(), "visible cube should render");
        assert!(ImageStats::analyze(&render(true)).looks_blank(), "Hidden cube should not render");
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn sphere_front_hemisphere_is_lit() {
        // Regression for generate_sphere winding: with a front directional
        // light the camera-facing hemisphere must be lit (bright center). When
        // the winding was reversed the near hemisphere was culled and the far
        // (unlit) inside showed through → a uniformly dark sphere.
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // render_cube_point_lit draws a sphere with a directional light coming
        // from +Z (toward the camera); no point lights.
        let rgba = render_cube_point_lit(&gpu, 256, 256, &[]);
        // Center pixel = sphere's front pole, facing the light → should be bright.
        let idx = ((128 * 256 + 128) * 4) as usize;
        let center_sum = rgba[idx] as u32 + rgba[idx + 1] as u32 + rgba[idx + 2] as u32;
        assert!(
            center_sum > 180,
            "front-lit sphere center should be bright, got sum={center_sum}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn point_lights_add_colored_illumination() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // mean luminance over lit (non-near-black) pixels.
        fn mean_lum(rgba: &[u8]) -> f64 {
            let (mut s, mut n) = (0u64, 0u64);
            for p in rgba.chunks_exact(4) {
                let sum = p[0] as u32 + p[1] as u32 + p[2] as u32;
                if sum > 24 {
                    s += sum as u64;
                    n += 1;
                }
            }
            s as f64 / n.max(1) as f64
        }

        let baseline = render_cube_point_lit(&gpu, 256, 256, &[]);
        // Surround the sphere with white point lights on all six axes, so the
        // visible hemisphere is lit regardless of its normal orientation.
        let pl = |p: [f32; 3]| voltex_renderer::PointLight::new(p, [1.0, 1.0, 1.0], 10.0, 2.0);
        let lights = [
            pl([3.0, 0.0, 0.0]),
            pl([-3.0, 0.0, 0.0]),
            pl([0.0, 3.0, 0.0]),
            pl([0.0, -3.0, 0.0]),
            pl([0.0, 0.0, 3.0]),
            pl([0.0, 0.0, -3.0]),
        ];
        let lit = render_cube_point_lit(&gpu, 256, 256, &lights);

        let (lb, ll) = (mean_lum(&baseline), mean_lum(&lit));
        assert!(
            ll > lb + 15.0,
            "surrounding point lights should brighten the sphere: baseline={lb} lit={ll}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn spotlight_lights_cone_footprint_not_corners() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let lum = |rgba: &[u8], x: usize, y: usize| -> u32 {
            let i = (y * 256 + x) * 4;
            rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32
        };

        // Spotlight above the plane aimed straight down — a tight cone hits the
        // center of the top-down view but not the corners.
        let down = voltex_renderer::PointLight::spot(
            [0.0, 5.0, 0.0],
            [0.0, -1.0, 0.0],
            [1.0, 1.0, 1.0],
            12.0,
            6.0,
            22.0,
        );
        let on = render_spot_lit(&gpu, 256, 256, &[down]);
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/");
        crate::save_png(&format!("{dir}spot_cone.png"), 256, 256, &on).unwrap();
        let center = lum(&on, 128, 128);
        let corner = lum(&on, 20, 20);
        assert!(
            center > 150,
            "spot cone center should be brightly lit, got {center}"
        );
        assert!(
            center > corner + 60,
            "inside the cone (center={center}) must be much brighter than the corner outside it (corner={corner})"
        );

        // Same light aimed horizontally — its cone no longer points at the ground
        // below, so the center stays dark (the cone gate culls it).
        let sideways = voltex_renderer::PointLight::spot(
            [0.0, 5.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            12.0,
            6.0,
            22.0,
        );
        let off = render_spot_lit(&gpu, 256, 256, &[sideways]);
        let center_off = lum(&off, 128, 128);
        assert!(
            center_off + 60 < center,
            "aiming the spot away from the ground should darken the center: aimed-down={center} aimed-away={center_off}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn particles_spread_outward_over_time() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // Mean distance of orange pixels from the image center — grows as the
        // radial burst expands.
        // Orange/amber test by channel ordering (r > g > b), robust to whether
        // the target is sRGB-encoded (which lifts the mid/low channels).
        let is_orange = |p: &[u8]| -> bool {
            p[0] > 150 && p[0] as i32 > p[1] as i32 + 20 && p[1] as i32 > p[2] as i32 + 20
        };
        let mean_radius = |rgba: &[u8]| -> f64 {
            let (mut sum, mut n) = (0.0f64, 0u64);
            for (i, p) in rgba.chunks_exact(4).enumerate() {
                if is_orange(p) {
                    let x = (i % 256) as f64 - 128.0;
                    let y = (i / 256) as f64 - 128.0;
                    sum += (x * x + y * y).sqrt();
                    n += 1;
                }
            }
            if n == 0 { 0.0 } else { sum / n as f64 }
        };

        let early = render_particle_burst(&gpu, 256, 256, 2, 0.1); // t≈0.2s
        let late = render_particle_burst(&gpu, 256, 256, 8, 0.1); // t≈0.8s
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/");
        crate::save_png(&format!("{dir}particle_burst.png"), 256, 256, &late).unwrap();

        let (re, rl) = (mean_radius(&early), mean_radius(&late));
        assert!(re > 1.0, "early burst should have visible particles, r={re:.1}");
        assert!(
            rl > re + 10.0,
            "particles should spread outward over time: early r={re:.1} late r={rl:.1}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn pbr_normal_map_perturbs_shading() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let flat = render_pbr_sphere_normalmap(&gpu, 256, 256, false);
        let mapped = render_pbr_sphere_normalmap(&gpu, 256, 256, true);
        let mut diff = 0u64;
        for (a, b) in flat.chunks_exact(4).zip(mapped.chunks_exact(4)) {
            diff += (a[0] as i64 - b[0] as i64).unsigned_abs();
            diff += (a[1] as i64 - b[1] as i64).unsigned_abs();
            diff += (a[2] as i64 - b[2] as i64).unsigned_abs();
        }
        let mean = diff as f64 / (256.0 * 256.0 * 3.0);
        assert!(mean > 2.0, "a normal map should perturb PBR shading, mean diff={mean:.2}");
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn basic_renderer_pbr_path_shades_differently_than_colored() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let colored = render_pbr_basic_renderer(&gpu, 256, 256, false);
        let pbr = render_pbr_basic_renderer(&gpu, 256, 256, true);
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/");
        crate::save_png(&format!("{dir}pbr_runtime.png"), 256, 256, &pbr).unwrap();

        assert!(!crate::ImageStats::analyze(&pbr).looks_blank(), "PBR frame blank");
        // Enabling PBR must visibly change shading vs the Blinn-Phong path.
        let mut diff = 0u64;
        for (a, b) in colored.chunks_exact(4).zip(pbr.chunks_exact(4)) {
            diff += (a[0] as i64 - b[0] as i64).unsigned_abs();
            diff += (a[1] as i64 - b[1] as i64).unsigned_abs();
            diff += (a[2] as i64 - b[2] as i64).unsigned_abs();
        }
        let mean = diff as f64 / (256.0 * 256.0 * 3.0);
        assert!(mean > 4.0, "PBR shading should differ from colored, mean channel diff={mean:.2}");
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn orthographic_footprint_is_depth_independent() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // Count lit (non-near-black) pixels — the cube's on-screen footprint.
        let footprint = |rgba: &[u8]| -> u32 {
            rgba.chunks_exact(4)
                .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30)
                .count() as u32
        };

        // Orthographic: a cube at z=0 vs z=-5 should cover ~the same pixels.
        let o_near = footprint(&render_cube_at_depth(&gpu, 256, 256, true, 0.0));
        let o_far = footprint(&render_cube_at_depth(&gpu, 256, 256, true, -5.0));
        let o_ratio = o_near.min(o_far) as f64 / o_near.max(o_far) as f64;
        assert!(
            o_near > 400 && o_ratio > 0.9,
            "orthographic footprint must be depth-independent: near={o_near} far={o_far} ratio={o_ratio:.2}"
        );

        // Perspective: the farther cube must be clearly smaller.
        let p_near = footprint(&render_cube_at_depth(&gpu, 256, 256, false, 0.0));
        let p_far = footprint(&render_cube_at_depth(&gpu, 256, 256, false, -5.0));
        assert!(
            (p_far as f64) < (p_near as f64) * 0.7,
            "perspective should foreshorten the farther cube: near={p_near} far={p_far}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn overlapping_sprites_blend_independent_of_spawn_order() {
        use voltex_renderer_ecs::Sprite;
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // A far blue + a near red sprite, both semi-transparent, overlapping at
        // screen center. Back-to-front sorting must make the composited result
        // the same no matter which order they were spawned in.
        let render = |near_first: bool| -> [u8; 3] {
            let target = OffscreenTarget::new(&gpu.device, 256, 256);
            let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
            let assets = RenderAssets::new();
            let mut world = World::new();
            let far = (Vec3::new(0.0, 0.0, -2.0), [0.1, 0.1, 1.0, 0.5]); // blue
            let near = (Vec3::new(0.0, 0.0, 2.0), [1.0, 0.1, 0.1, 0.5]); // red
            let order = if near_first { [near, far] } else { [far, near] };
            for (pos, color) in order {
                let e = world.spawn();
                world.add(e, Transform::from_position(pos));
                world.add(e, Sprite::solid([3.0, 3.0], color));
            }
            voltex_ecs::propagate_transforms(&mut world);
            let camera = Camera::orthographic(Vec3::new(0.0, 0.0, 8.0), 1.0, 6.0);
            let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("capture sprite order"),
            });
            renderer.render(
                &gpu.queue,
                &mut encoder,
                &target.color_view,
                &target.depth_view,
                wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
                &mut world,
                &assets,
                camera.view_projection(),
                camera.position,
                LightUniform::new(),
            );
            gpu.queue.submit(Some(encoder.finish()));
            let rgba = target.read_rgba8(&gpu.device, &gpu.queue);
            let i = (128 * 256 + 128) * 4;
            [rgba[i], rgba[i + 1], rgba[i + 2]]
        };

        let a = render(true);
        let b = render(false);
        // Spawn-order independence: the two composites match closely.
        let diff = (a[0] as i32 - b[0] as i32).abs()
            + (a[1] as i32 - b[1] as i32).abs()
            + (a[2] as i32 - b[2] as i32).abs();
        assert!(diff < 12, "overlap must be spawn-order-independent: {a:?} vs {b:?}");
        // And the near (red) sprite composites on top → red beats blue.
        assert!(a[0] > a[2], "near red sprite should dominate the overlap: {a:?}");
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn render_assets_png_loader_roundtrips_into_a_sprite() {
        use voltex_renderer_ecs::Sprite;
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // Encode a 2×2 solid-red PNG, then load it via the convenience method.
        let red = [255u8, 0, 0, 255].repeat(4); // 2×2 RGBA red
        let png = crate::encode_rgba8(2, 2, &red);

        let target = OffscreenTarget::new(&gpu.device, 256, 256);
        let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
        let mut assets = RenderAssets::new();
        let tex = assets
            .add_texture_png(&gpu.device, &gpu.queue, &png)
            .expect("decode+upload PNG");

        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        world.add(e, Sprite::textured(tex, [3.0, 3.0]));
        voltex_ecs::propagate_transforms(&mut world);

        let camera = Camera::orthographic(Vec3::new(0.0, 0.0, 8.0), 1.0, 6.0);
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("capture png loader"),
        });
        renderer.render(
            &gpu.queue,
            &mut encoder,
            &target.color_view,
            &target.depth_view,
            wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
            &mut world,
            &assets,
            camera.view_projection(),
            camera.position,
            LightUniform::new(),
        );
        gpu.queue.submit(Some(encoder.finish()));
        let rgba = target.read_rgba8(&gpu.device, &gpu.queue);

        let i = (128 * 256 + 128) * 4;
        let c = [rgba[i], rgba[i + 1], rgba[i + 2]];
        assert!(
            c[0] > 150 && c[1] < 90 && c[2] < 90,
            "PNG-loaded red texture should render red, got {c:?}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn sprite_sheet_frame_selects_atlas_cell() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let center = |rgba: &[u8]| -> [u8; 3] {
            let i = (128 * 256 + 128) * 4;
            [rgba[i], rgba[i + 1], rgba[i + 2]]
        };
        // Frame 0 = TL red, 1 = TR green, 2 = BL blue, 3 = BR yellow.
        let f0 = center(&render_sprite_sheet_frame(&gpu, 256, 256, 0));
        let f1 = center(&render_sprite_sheet_frame(&gpu, 256, 256, 1));
        let f2 = center(&render_sprite_sheet_frame(&gpu, 256, 256, 2));
        let f3 = center(&render_sprite_sheet_frame(&gpu, 256, 256, 3));
        assert!(f0[0] > 150 && f0[1] < 90 && f0[2] < 90, "frame 0 should be red, got {f0:?}");
        assert!(f1[1] > 150 && f1[0] < 90 && f1[2] < 90, "frame 1 should be green, got {f1:?}");
        assert!(f2[2] > 150 && f2[0] < 90 && f2[1] < 90, "frame 2 should be blue, got {f2:?}");
        assert!(
            f3[0] > 150 && f3[1] > 150 && f3[2] < 90,
            "frame 3 should be yellow, got {f3:?}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn billboard_sprite_faces_camera_from_any_angle() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // Count magenta pixels (the sprite color).
        let magenta = |rgba: &[u8]| -> u32 {
            rgba.chunks_exact(4)
                .filter(|p| p[0] > 120 && p[2] > 120 && p[1] < 80)
                .count() as u32
        };
        use std::f32::consts::FRAC_PI_2;
        // Front: camera on +Z looking -Z. Side: camera on +X looking -X. Same
        // distance (4) → a true billboard keeps the same on-screen footprint.
        let front = render_billboard_sprite(&gpu, 256, 256, Vec3::new(0.0, 0.0, 4.0), 0.0);
        let side = render_billboard_sprite(&gpu, 256, 256, Vec3::new(4.0, 0.0, 0.0), -FRAC_PI_2);
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/");
        crate::save_png(&format!("{dir}sprite_billboard.png"), 256, 256, &front).unwrap();

        let (cf, cs) = (magenta(&front), magenta(&side));
        assert!(cf > 1500, "front view should show the sprite, got {cf} px");
        assert!(cs > 1500, "side view should show the sprite, got {cs} px");
        // Footprint should be nearly identical (it always faces the camera).
        let ratio = cf.min(cs) as f64 / cf.max(cs) as f64;
        assert!(
            ratio > 0.8,
            "billboard footprint must match across angles: front={cf} side={cs} ratio={ratio:.2}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn debug_lines_draw_colored_segments() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let px = |rgba: &[u8], x: usize, y: usize| -> [u8; 3] {
            let i = (y * 256 + x) * 4;
            [rgba[i], rgba[i + 1], rgba[i + 2]]
        };

        // No lines → black frame (control).
        let empty = render_debug_lines(&gpu, 256, 256, &[]);
        let c0 = px(&empty, 128, 128);
        assert!(
            c0[0] < 12 && c0[1] < 12 && c0[2] < 12,
            "no lines should leave the center black, got {c0:?}"
        );

        // A red horizontal segment through the world origin crosses screen
        // center; a green vertical one shares the center pixel.
        use voltex_renderer::LineVertex;
        let red = [1.0, 0.0, 0.0];
        let green = [0.0, 1.0, 0.0];
        let verts = [
            LineVertex::new([-1.5, 0.0, 0.0], red),
            LineVertex::new([1.5, 0.0, 0.0], red),
            LineVertex::new([0.0, -1.5, 0.0], green),
            LineVertex::new([0.0, 1.5, 0.0], green),
        ];
        let lit = render_debug_lines(&gpu, 256, 256, &verts);
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/");
        crate::save_png(&format!("{dir}debug_lines.png"), 256, 256, &lit).unwrap();

        // The horizontal line sits at world y=0 → near the center row; a 1px
        // line straddles the row boundary, so scan a small band for it.
        let brightest_in = |xs: std::ops::Range<usize>, ys: std::ops::Range<usize>| -> [u8; 3] {
            let mut best = [0u8; 3];
            for y in ys {
                for x in xs.clone() {
                    let p = px(&lit, x, y);
                    if p.iter().map(|&c| c as u32).sum::<u32>()
                        > best.iter().map(|&c| c as u32).sum::<u32>()
                    {
                        best = p;
                    }
                }
            }
            best
        };
        // A point on the red horizontal line, well left of the vertical one.
        let on_red = brightest_in(55..66, 120..137);
        assert!(
            on_red[0] > 150 && on_red[1] < 90 && on_red[2] < 90,
            "the horizontal segment should paint a red row, got {on_red:?}"
        );
        // A point on the green vertical line, well above the horizontal one.
        let on_green = brightest_in(120..137, 55..66);
        assert!(
            on_green[1] > 150 && on_green[0] < 90 && on_green[2] < 90,
            "the vertical segment should paint a green column, got {on_green:?}"
        );
        // A corner away from both lines stays black.
        let corner = px(&lit, 20, 20);
        assert!(
            corner.iter().all(|&c| c < 12),
            "corners away from the lines should stay black, got {corner:?}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn loads_gltf_mesh() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let glb = minimal_glb_triangle();
        let mesh = voltex_renderer::Mesh::from_gltf(&gpu.device, &glb).expect("parse+upload glTF");
        assert_eq!(mesh.num_indices, 3, "triangle should have 3 indices");
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn loads_png_into_texture_and_renders() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // Encode a 4×4 red image to PNG, decode via GpuTexture::from_png, render.
        let red: Vec<u8> = [200u8, 40, 40, 255].iter().cycle().take(4 * 4 * 4).copied().collect();
        let png = voltex_image::png_encode::encode_rgba8(4, 4, &red);
        let layout = voltex_renderer::GpuTexture::bind_group_layout(&gpu.device);
        let tex = voltex_renderer::GpuTexture::from_png(&gpu.device, &gpu.queue, &png, &layout)
            .expect("decode PNG");
        let rgba = render_cube_with_texture(&gpu, 256, 256, tex);

        // The cube should be red-dominant (texture sampled through the runtime).
        let bg = [rgba[0], rgba[1], rgba[2]];
        let (mut r, mut b, mut n) = (0u64, 0u64, 0u64);
        for p in rgba.chunks_exact(4) {
            if (p[0] as i32 - bg[0] as i32).abs() > 20 || (p[2] as i32 - bg[2] as i32).abs() > 20 {
                r += p[0] as u64;
                b += p[2] as u64;
                n += 1;
            }
        }
        let n = n.max(1);
        assert!(r / n > b / n, "PNG-textured cube should be red-dominant: R={} B={}", r / n, b / n);
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn loads_and_renders_obj_model() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let rgba = render_obj_model(&gpu, 256, 256, CUBE_OBJ);
        let stats = ImageStats::analyze(&rgba);
        assert!(
            !stats.looks_blank(),
            "loaded OBJ model should render (not blank): {stats:?}"
        );
    }

    /// Count pixels that differ from the top-left (background) pixel — i.e. the
    /// rendered object's footprint.
    fn object_pixels(rgba: &[u8]) -> usize {
        let bg = [rgba[0], rgba[1], rgba[2]];
        rgba
            .chunks_exact(4)
            .filter(|p| {
                (p[0] as i32 - bg[0] as i32).abs() > 20
                    || (p[1] as i32 - bg[1] as i32).abs() > 20
                    || (p[2] as i32 - bg[2] as i32).abs() > 20
            })
            .count()
    }

    /// Regression for the "I can't see my boxes" report: a 1-unit object at the
    /// origin is a tiny speck under the runtime's default far camera
    /// `(0,10,20)`, but fills a large area once a framing camera is used. This
    /// is why `RuntimeConfig::with_camera` / `RuntimeApp::set_camera_fn` exist.
    /// Regression for "objects camouflage against the ground": BasicRenderer
    /// must honour `MaterialUniform.base_color`. With the old white-default
    /// path a framed scene of same-shaded cubes + ground showed only 2 colors
    /// (ground + background); distinct per-role colors add at least the blue
    /// player on the gray ground → 3+.
    #[test]
    #[ignore = "requires a GPU adapter"]
    fn basic_renderer_applies_material_base_color() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let rgba = render_demo_like_cam(&gpu, 320, 180, Vec3::new(0.0, 9.0, 13.0), 0.0, -0.55, true);
        let stats = ImageStats::analyze(&rgba);
        assert!(
            stats.distinct_colors >= 3,
            "per-object base_color should be visible against the ground: {stats:?}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn camera_framing_controls_visible_object_size() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let (w, h) = (320, 180);
        // Cubes only (no ground), so the footprint isolates the gameplay objects.
        let far = render_demo_like_cam(&gpu, w, h, Vec3::new(0.0, 10.0, 20.0), 0.0, -0.4, false);
        let close = render_demo_like_cam(&gpu, w, h, Vec3::new(0.0, 2.5, 6.0), 0.0, -0.3, false);
        let (far_px, close_px) = (object_pixels(&far), object_pixels(&close));
        assert!(far_px > 0, "the cube must render even when far (got {far_px})");
        assert!(
            close_px > far_px * 5,
            "framing camera should show the object far larger: far={far_px} close={close_px}"
        );
    }

    // GPU-gated: ignored by default (no GPU in CI). Run with:
    //   cargo test -p voltex_capture -- --ignored
    #[test]
    #[ignore]
    fn single_cube_is_not_blank() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let rgba = render_single_cube(&gpu, 256, 256);
        let stats = ImageStats::analyze(&rgba);
        assert!(
            !stats.looks_blank(),
            "single cube frame is blank: {stats:?}"
        );
        assert!(stats.mean_luminance > 0.01, "frame too dark: {stats:?}");
    }

    #[test]
    #[ignore]
    fn cube_grid_is_not_blank() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let rgba = render_cube_grid(&gpu, 320, 240);
        let stats = ImageStats::analyze(&rgba);
        assert!(!stats.looks_blank(), "cube grid frame is blank: {stats:?}");
    }

    #[test]
    #[ignore]
    fn materialized_primitives_render_nonblank() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let rgba = render_materialized_primitives(&gpu, 320, 240);
        let stats = ImageStats::analyze(&rgba);
        assert!(
            !stats.looks_blank(),
            "declarative primitive frame is blank — materialize_renderables \
             did not produce drawable meshes: {stats:?}"
        );
        assert!(stats.mean_luminance > 0.01, "frame too dark: {stats:?}");
    }

    #[test]
    #[ignore]
    fn materialize_dedupes_identical_meshes_and_attaches_bounds() {
        use voltex_ecs::Bounds;
        use voltex_renderer_ecs::{materialize_renderables, MaterialDesc, PrimitiveMesh};

        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let mut assets = RenderAssets::new();
        let mut world = World::new();

        // Three identical cubes + one sphere.
        let cubes: Vec<_> = (0..3)
            .map(|i| {
                let e = world.spawn();
                world.add(e, Transform::from_position(Vec3::new(i as f32, 0.0, 0.0)));
                world.add(e, PrimitiveMesh::cube(1.0));
                world.add(e, MaterialDesc::color([1.0, 1.0, 1.0, 1.0]));
                e
            })
            .collect();
        let sphere = world.spawn();
        world.add(sphere, Transform::from_position(Vec3::ZERO));
        world.add(sphere, PrimitiveMesh::sphere_default(0.5));
        world.add(sphere, MaterialDesc::color([1.0, 1.0, 1.0, 1.0]));

        materialize_renderables(&mut world, &mut assets, &gpu.device);

        // Every entity got a MeshHandle, a MaterialHandle, and Bounds.
        for &e in &cubes {
            assert!(world.get::<MeshHandle>(e).is_some(), "cube missing MeshHandle");
            assert!(world.get::<MaterialHandle>(e).is_some(), "cube missing MaterialHandle");
            assert!(world.get::<Bounds>(e).is_some(), "cube missing Bounds");
        }

        // The three identical cubes share ONE mesh; the sphere differs.
        let c0 = *world.get::<MeshHandle>(cubes[0]).unwrap();
        let c1 = *world.get::<MeshHandle>(cubes[1]).unwrap();
        let c2 = *world.get::<MeshHandle>(cubes[2]).unwrap();
        let sp = *world.get::<MeshHandle>(sphere).unwrap();
        assert_eq!(c0, c1, "identical cubes must dedupe to one mesh");
        assert_eq!(c1, c2, "identical cubes must dedupe to one mesh");
        assert_ne!(c0, sp, "a sphere must not share the cube's mesh");

        // Identical materials also dedupe to one handle.
        let m0 = *world.get::<MaterialHandle>(cubes[0]).unwrap();
        let m_sp = *world.get::<MaterialHandle>(sphere).unwrap();
        assert_eq!(m0, m_sp, "identical materials must dedupe to one handle");

        // Idempotent: a second call adds nothing (handles unchanged).
        materialize_renderables(&mut world, &mut assets, &gpu.device);
        assert_eq!(c0, *world.get::<MeshHandle>(cubes[0]).unwrap());
    }
}
