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
    let s = 0.5;
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        (
            [1.0, 0.0, 0.0],
            [[s, -s, -s], [s, -s, s], [s, s, s], [s, s, -s]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[-s, -s, s], [-s, -s, -s], [-s, s, -s], [-s, s, s]],
        ),
        (
            [0.0, 1.0, 0.0],
            [[-s, s, -s], [s, s, -s], [s, s, s], [-s, s, s]],
        ),
        (
            [0.0, -1.0, 0.0],
            [[-s, -s, s], [s, -s, s], [s, -s, -s], [-s, -s, -s]],
        ),
        (
            [0.0, 0.0, 1.0],
            [[s, -s, s], [-s, -s, s], [-s, s, s], [s, s, s]],
        ),
        (
            [0.0, 0.0, -1.0],
            [[-s, -s, -s], [s, -s, -s], [s, s, -s], [-s, s, -s]],
        ),
    ];
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut verts = Vec::with_capacity(24);
    for (normal, corners) in faces {
        for (corner, uv) in corners.iter().zip(uvs.iter()) {
            verts.push(MeshVertex {
                position: *corner,
                normal,
                uv: *uv,
                tangent: [1.0, 0.0, 0.0, 1.0],
            });
        }
    }
    let mut indices = Vec::with_capacity(36);
    for face in 0..6u32 {
        let base = face * 4;
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(device, &verts, &indices)
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
}
