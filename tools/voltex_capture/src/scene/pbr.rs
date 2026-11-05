//! PBR scenes: the raw Cook-Torrance sphere grid and the `BasicRenderer`
//! PBR path (metallic row, normal map).

use voltex_ecs::{MaterialHandle, MeshHandle, Transform, World};
use voltex_math::{Mat4, Vec3};
use voltex_renderer::{
    create_pbr_full_texture_bind_group, create_pbr_pipeline, generate_sphere,
    pbr_full_texture_bind_group_layout, Camera, CameraUniform, GpuTexture, IblResources, LightData,
    LightUniform, LightsUniform, MaterialUniform, Mesh, ShadowMap, ShadowUniform,
};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};
use wgpu::util::DeviceExt;

use super::{align_up, camera_light_bgl, draw_and_read};
use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

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
