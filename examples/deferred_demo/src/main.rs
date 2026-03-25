use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_math::{Vec3, Mat4};
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::{
    GpuContext, Camera, FpsController, CameraUniform, LightsUniform, LightData,
    Mesh, GpuTexture, MaterialUniform, generate_sphere,
    ShadowMap, ShadowUniform, IblResources,
    GBuffer, create_fullscreen_vertex_buffer,
    create_gbuffer_pipeline, create_lighting_pipeline,
    gbuffer_camera_bind_group_layout,
    lighting_gbuffer_bind_group_layout, lighting_lights_bind_group_layout,
    lighting_shadow_bind_group_layout,
    pbr_full_texture_bind_group_layout, create_pbr_full_texture_bind_group,
    SsgiResources, SsgiUniform,
    ssgi_gbuffer_bind_group_layout, ssgi_data_bind_group_layout, create_ssgi_pipeline,
    RtAccel, RtInstance, BlasMeshData, RtShadowResources, RtShadowUniform,
    rt_shadow_gbuffer_bind_group_layout, rt_shadow_data_bind_group_layout, create_rt_shadow_pipeline,
    HdrTarget, HDR_FORMAT,
    BloomResources, BloomUniform, mip_sizes, BLOOM_MIP_COUNT,
    TonemapUniform,
    bloom_bind_group_layout, create_bloom_downsample_pipeline, create_bloom_upsample_pipeline,
    tonemap_bind_group_layout, create_tonemap_pipeline,
};
use wgpu::util::DeviceExt;
use bytemuck::{Pod, Zeroable};

const GRID_SIZE: usize = 5;
const NUM_SPHERES: usize = GRID_SIZE * GRID_SIZE;
const SPACING: f32 = 1.2;
const NUM_POINT_LIGHTS: usize = 8;

/// Camera position uniform for the lighting pass (16 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct CameraPositionUniform {
    camera_pos: [f32; 3],
    _padding: f32,
}

fn align_up(size: u32, alignment: u32) -> u32 {
    ((size + alignment - 1) / alignment) * alignment
}

struct DeferredDemoApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    gbuffer: GBuffer,

    // G-Buffer pass resources
    gbuffer_pipeline: wgpu::RenderPipeline,
    mesh: Mesh,
    camera: Camera,
    fps_controller: FpsController,
    camera_buffer: wgpu::Buffer,
    material_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    pbr_texture_bind_group: wgpu::BindGroup,
    material_bind_group: wgpu::BindGroup,

    // SSGI pass resources
    ssgi: SsgiResources,
    ssgi_pipeline: wgpu::RenderPipeline,
    ssgi_gbuffer_bind_group: wgpu::BindGroup,
    ssgi_data_bind_group: wgpu::BindGroup,
    ssgi_gb_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    ssgi_data_layout: wgpu::BindGroupLayout,

    // Lighting pass resources
    lighting_pipeline: wgpu::RenderPipeline,
    fullscreen_vb: wgpu::Buffer,
    gbuffer_bind_group: wgpu::BindGroup,
    lights_bind_group: wgpu::BindGroup,
    shadow_bind_group: wgpu::BindGroup,
    light_buffer: wgpu::Buffer,
    cam_pos_buffer: wgpu::Buffer,

    // RT Shadow resources
    rt_accel: RtAccel,
    rt_shadow: RtShadowResources,
    rt_shadow_pipeline: wgpu::ComputePipeline,
    rt_shadow_gb_bg: wgpu::BindGroup,
    rt_shadow_data_bg: wgpu::BindGroup,
    rt_shadow_gb_layout: wgpu::BindGroupLayout,
    rt_shadow_data_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    vertex_count: u32,

    // HDR + Bloom + Tonemap resources
    hdr_target: HdrTarget,
    bloom: BloomResources,
    bloom_down_pipeline: wgpu::RenderPipeline,
    bloom_up_pipeline: wgpu::RenderPipeline,
    bloom_layout: wgpu::BindGroupLayout,
    bloom_down_bind_groups: Vec<wgpu::BindGroup>,
    bloom_up_bind_groups: Vec<wgpu::BindGroup>,
    bloom_threshold_buffer: wgpu::Buffer,
    bloom_no_threshold_buffer: wgpu::Buffer,
    linear_sampler: wgpu::Sampler,
    tonemap_pipeline: wgpu::RenderPipeline,
    tonemap_layout: wgpu::BindGroupLayout,
    tonemap_bind_group: wgpu::BindGroup,
    tonemap_uniform_buffer: wgpu::Buffer,

    // Layouts needed for rebuild on resize
    gbuffer_layout: wgpu::BindGroupLayout,
    shadow_layout: wgpu::BindGroupLayout,

    // Keep textures alive
    _albedo_tex: GpuTexture,
    _normal_tex: (wgpu::Texture, wgpu::TextureView, wgpu::Sampler),
    _orm_tex: GpuTexture,
    _emissive_tex: GpuTexture,
    _shadow_map: ShadowMap,
    _ibl: IblResources,
    _shadow_uniform_buffer: wgpu::Buffer,

    input: InputState,
    timer: GameTimer,
    cam_aligned_size: u32,
    mat_aligned_size: u32,
    time_elapsed: f32,
}

impl ApplicationHandler for DeferredDemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Deferred Rendering Demo".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new_with_features(
            window.handle.clone(),
            wgpu::Features::EXPERIMENTAL_RAY_QUERY,
        );

        // Dynamic uniform buffer alignment
        let alignment = gpu.device.limits().min_uniform_buffer_offset_alignment;
        let cam_aligned_size = align_up(std::mem::size_of::<CameraUniform>() as u32, alignment);
        let mat_aligned_size = align_up(std::mem::size_of::<MaterialUniform>() as u32, alignment);

        // Generate sphere mesh (with BLAS_INPUT usage for ray tracing)
        let (vertices, indices) = generate_sphere(0.4, 32, 16);
        let vertex_count = vertices.len() as u32;
        let mesh = Mesh::new_with_usage(
            &gpu.device,
            &vertices,
            &indices,
            wgpu::BufferUsages::BLAS_INPUT,
        );

        // Camera
        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
        let camera = Camera::new(Vec3::new(0.0, 0.0, 8.0), aspect);
        let fps_controller = FpsController::new();

        // G-Buffer
        let gbuffer = GBuffer::new(&gpu.device, gpu.config.width, gpu.config.height);

        // ---------------------------------------------------------------
        // G-Buffer pass bind group layouts
        // ---------------------------------------------------------------
        let gbuf_cam_layout = gbuffer_camera_bind_group_layout(&gpu.device);
        let pbr_tex_layout = pbr_full_texture_bind_group_layout(&gpu.device);
        let mat_layout = MaterialUniform::bind_group_layout(&gpu.device);

        // Camera dynamic uniform buffer (one CameraUniform per sphere)
        let camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GBuf Camera Dynamic UBO"),
            size: (cam_aligned_size as usize * NUM_SPHERES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Material dynamic uniform buffer
        let material_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GBuf Material Dynamic UBO"),
            size: (mat_aligned_size as usize * NUM_SPHERES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Camera bind group (dynamic offset, group 0)
        let camera_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GBuf Camera Bind Group"),
            layout: &gbuf_cam_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &camera_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<CameraUniform>() as u64),
                }),
            }],
        });

        // PBR textures: white albedo + flat normal + ORM + emissive
        let old_tex_layout = GpuTexture::bind_group_layout(&gpu.device);
        let albedo_tex = GpuTexture::white_1x1(&gpu.device, &gpu.queue, &old_tex_layout);
        let normal_tex = GpuTexture::flat_normal_1x1(&gpu.device, &gpu.queue);
        let orm_tex = GpuTexture::white_1x1(&gpu.device, &gpu.queue, &old_tex_layout);
        let emissive_tex = GpuTexture::black_1x1(&gpu.device, &gpu.queue, &old_tex_layout);
        let pbr_texture_bind_group = create_pbr_full_texture_bind_group(
            &gpu.device,
            &pbr_tex_layout,
            &albedo_tex.view,
            &albedo_tex.sampler,
            &normal_tex.1,
            &normal_tex.2,
            &orm_tex.view,
            &orm_tex.sampler,
            &emissive_tex.view,
            &emissive_tex.sampler,
        );

        // Material bind group (dynamic offset, group 2)
        let material_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GBuf Material Bind Group"),
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

        // G-Buffer pipeline
        let gbuffer_pipeline = create_gbuffer_pipeline(
            &gpu.device,
            &gbuf_cam_layout,
            &pbr_tex_layout,
            &mat_layout,
        );

        // ---------------------------------------------------------------
        // SSGI pass resources
        // ---------------------------------------------------------------
        let ssgi = SsgiResources::new(&gpu.device, &gpu.queue, gpu.config.width, gpu.config.height);
        let ssgi_gb_layout = ssgi_gbuffer_bind_group_layout(&gpu.device);
        let ssgi_data_layout = ssgi_data_bind_group_layout(&gpu.device);
        let ssgi_pipeline = create_ssgi_pipeline(&gpu.device, &ssgi_gb_layout, &ssgi_data_layout);

        // SSGI G-Buffer bind group (group 0): position, normal, albedo, non-filtering sampler
        let ssgi_nearest_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSGI GBuffer Nearest Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let ssgi_gbuffer_bind_group = create_ssgi_gbuffer_bind_group(
            &gpu.device,
            &ssgi_gb_layout,
            &gbuffer,
            &ssgi_nearest_sampler,
        );

        // SSGI data bind group (group 1): uniform, kernel, noise texture, noise sampler
        let ssgi_data_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSGI Data Bind Group"),
            layout: &ssgi_data_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ssgi.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ssgi.kernel_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&ssgi.noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&ssgi.noise_sampler),
                },
            ],
        });

        // ---------------------------------------------------------------
        // RT Shadow: build acceleration structures
        // ---------------------------------------------------------------
        let blas_mesh = BlasMeshData {
            vertex_buffer: &mesh.vertex_buffer,
            index_buffer: &mesh.index_buffer,
            vertex_count,
            index_count: mesh.num_indices,
        };

        // Build TLAS instances — one per sphere in the grid, all sharing the same BLAS
        let half_grid = (GRID_SIZE as f32 - 1.0) * SPACING * 0.5;
        let rt_instances: Vec<RtInstance> = (0..GRID_SIZE)
            .flat_map(|row| {
                (0..GRID_SIZE).map(move |col| {
                    let x = col as f32 * SPACING - half_grid;
                    let y = row as f32 * SPACING - half_grid;
                    let model = Mat4::translation(x, y, 0.0);
                    RtInstance {
                        transform: *model.as_slice(),
                        blas_index: 0,
                    }
                })
            })
            .collect();

        let mut rt_encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("RT Accel Build Encoder"),
        });
        let rt_accel = RtAccel::new(&gpu.device, &mut rt_encoder, &[blas_mesh], &rt_instances);
        gpu.queue.submit(std::iter::once(rt_encoder.finish()));

        // RT Shadow resources
        let rt_shadow = RtShadowResources::new(&gpu.device, gpu.config.width, gpu.config.height);

        // RT Shadow pipeline + bind groups
        let rt_shadow_gb_layout = rt_shadow_gbuffer_bind_group_layout(&gpu.device);
        let rt_shadow_data_layout = rt_shadow_data_bind_group_layout(&gpu.device);
        let rt_shadow_pipeline = create_rt_shadow_pipeline(&gpu.device, &rt_shadow_gb_layout, &rt_shadow_data_layout);

        let rt_shadow_gb_bg = create_rt_shadow_gbuffer_bg(
            &gpu.device,
            &rt_shadow_gb_layout,
            &gbuffer,
        );
        let rt_shadow_data_bg = create_rt_shadow_data_bg(
            &gpu.device,
            &rt_shadow_data_layout,
            &rt_accel,
            &rt_shadow,
        );

        // ---------------------------------------------------------------
        // Lighting pass bind group layouts
        // ---------------------------------------------------------------
        let gbuffer_layout = lighting_gbuffer_bind_group_layout(&gpu.device);
        let lights_layout = lighting_lights_bind_group_layout(&gpu.device);
        let shadow_layout = lighting_shadow_bind_group_layout(&gpu.device);

        // Fullscreen triangle vertex buffer
        let fullscreen_vb = create_fullscreen_vertex_buffer(&gpu.device);

        // GBuffer bind group (group 0) — non-filtering sampler for Rgba32Float position
        let nearest_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("GBuffer Nearest Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let gbuffer_bind_group = create_gbuffer_bind_group(
            &gpu.device,
            &gbuffer_layout,
            &gbuffer,
            &nearest_sampler,
        );

        // Lights uniform buffer
        let light_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lights Uniform Buffer"),
            size: std::mem::size_of::<LightsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Camera position uniform buffer (for lighting pass)
        let cam_pos_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Position Uniform Buffer"),
            size: std::mem::size_of::<CameraPositionUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Lights bind group (group 1)
        let lights_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lighting Lights Bind Group"),
            layout: &lights_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cam_pos_buffer.as_entire_binding(),
                },
            ],
        });

        // Shadow + IBL + SSGI bind group (group 2) — now 7 entries
        let shadow_map = ShadowMap::new(&gpu.device);
        let ibl = IblResources::new(&gpu.device, &gpu.queue);
        let shadow_uniform = ShadowUniform {
            light_view_proj: [[0.0; 4]; 4],
            shadow_map_size: 0.0,
            shadow_bias: 0.0,
            _padding: [0.0; 2],
        };
        let shadow_uniform_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Uniform Buffer"),
            contents: bytemuck::cast_slice(&[shadow_uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let ssgi_filtering_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSGI Filtering Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let rt_shadow_nearest_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("RT Shadow Nearest Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let shadow_bind_group = create_shadow_bind_group(
            &gpu.device,
            &shadow_layout,
            &shadow_map,
            &shadow_uniform_buffer,
            &ibl,
            &ssgi,
            &ssgi_filtering_sampler,
            &rt_shadow,
            &rt_shadow_nearest_sampler,
        );

        // Lighting pipeline — now renders to HDR format instead of surface
        let lighting_pipeline = create_lighting_pipeline(
            &gpu.device,
            HDR_FORMAT,
            &gbuffer_layout,
            &lights_layout,
            &shadow_layout,
        );

        // ---------------------------------------------------------------
        // HDR Target
        // ---------------------------------------------------------------
        let hdr_target = HdrTarget::new(&gpu.device, gpu.config.width, gpu.config.height);

        // ---------------------------------------------------------------
        // Bloom resources + pipelines
        // ---------------------------------------------------------------
        let bloom = BloomResources::new(&gpu.device, gpu.config.width, gpu.config.height);
        let bloom_layout_bg = bloom_bind_group_layout(&gpu.device);
        let bloom_down_pipeline = create_bloom_downsample_pipeline(&gpu.device, &bloom_layout_bg);
        let bloom_up_pipeline = create_bloom_upsample_pipeline(&gpu.device, &bloom_layout_bg);

        let linear_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Two bloom uniform buffers: one with threshold, one without
        let bloom_threshold_uniform = BloomUniform::default(); // threshold=1.0
        let bloom_threshold_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bloom Threshold UBO"),
            contents: bytemuck::bytes_of(&bloom_threshold_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bloom_no_threshold_uniform = BloomUniform {
            threshold: 0.0,
            soft_threshold: 0.0,
            _padding: [0.0; 2],
        };
        let bloom_no_threshold_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bloom No-Threshold UBO"),
            contents: bytemuck::bytes_of(&bloom_no_threshold_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bloom bind groups
        let bloom_down_bind_groups = create_bloom_down_bind_groups(
            &gpu.device, &bloom_layout_bg, &hdr_target, &bloom,
            &linear_sampler, &bloom_threshold_buffer, &bloom_no_threshold_buffer,
        );
        let bloom_up_bind_groups = create_bloom_up_bind_groups(
            &gpu.device, &bloom_layout_bg, &bloom,
            &linear_sampler, &bloom_no_threshold_buffer,
        );

        // ---------------------------------------------------------------
        // Tonemap pipeline + bind group
        // ---------------------------------------------------------------
        let tonemap_layout_bg = tonemap_bind_group_layout(&gpu.device);
        let tonemap_pipeline = create_tonemap_pipeline(&gpu.device, gpu.surface_format, &tonemap_layout_bg);

        let tonemap_uniform = TonemapUniform::default();
        let tonemap_uniform_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tonemap Uniform Buffer"),
            contents: bytemuck::bytes_of(&tonemap_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let tonemap_bind_group = create_tonemap_bind_group(
            &gpu.device, &tonemap_layout_bg, &hdr_target, &bloom,
            &linear_sampler, &tonemap_uniform_buffer,
        );

        self.state = Some(AppState {
            window,
            gpu,
            gbuffer,
            gbuffer_pipeline,
            mesh,
            camera,
            fps_controller,
            camera_buffer,
            material_buffer,
            camera_bind_group,
            pbr_texture_bind_group,
            material_bind_group,
            ssgi,
            ssgi_pipeline,
            ssgi_gbuffer_bind_group,
            ssgi_data_bind_group,
            ssgi_gb_layout,
            ssgi_data_layout,
            rt_accel,
            rt_shadow,
            rt_shadow_pipeline,
            rt_shadow_gb_bg,
            rt_shadow_data_bg,
            rt_shadow_gb_layout,
            rt_shadow_data_layout,
            vertex_count,
            lighting_pipeline,
            fullscreen_vb,
            gbuffer_bind_group,
            lights_bind_group,
            shadow_bind_group,
            light_buffer,
            cam_pos_buffer,
            hdr_target,
            bloom,
            bloom_down_pipeline,
            bloom_up_pipeline,
            bloom_layout: bloom_layout_bg,
            bloom_down_bind_groups,
            bloom_up_bind_groups,
            bloom_threshold_buffer,
            bloom_no_threshold_buffer,
            linear_sampler,
            tonemap_pipeline,
            tonemap_layout: tonemap_layout_bg,
            tonemap_bind_group,
            tonemap_uniform_buffer,
            gbuffer_layout,
            shadow_layout,
            _albedo_tex: albedo_tex,
            _normal_tex: normal_tex,
            _orm_tex: orm_tex,
            _emissive_tex: emissive_tex,
            _shadow_map: shadow_map,
            _ibl: ibl,
            _shadow_uniform_buffer: shadow_uniform_buffer,
            input: InputState::new(),
            timer: GameTimer::new(60),
            cam_aligned_size,
            mat_aligned_size,
            time_elapsed: 0.0,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state: key_state,
                        ..
                    },
                ..
            } => {
                let pressed = key_state == winit::event::ElementState::Pressed;
                state.input.process_key(key_code, pressed);
                if key_code == KeyCode::Escape && pressed {
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                state.gpu.resize(size.width, size.height);
                if size.width > 0 && size.height > 0 {
                    state.camera.aspect = size.width as f32 / size.height as f32;

                    // Resize G-Buffer
                    state.gbuffer.resize(&state.gpu.device, size.width, size.height);

                    // Resize SSGI output texture
                    state.ssgi.resize(&state.gpu.device, size.width, size.height);

                    // Recreate gbuffer bind group with new texture views
                    let nearest_sampler = state.gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("GBuffer Nearest Sampler"),
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        address_mode_w: wgpu::AddressMode::ClampToEdge,
                        mag_filter: wgpu::FilterMode::Nearest,
                        min_filter: wgpu::FilterMode::Nearest,
                        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                        ..Default::default()
                    });
                    state.gbuffer_bind_group = create_gbuffer_bind_group(
                        &state.gpu.device,
                        &state.gbuffer_layout,
                        &state.gbuffer,
                        &nearest_sampler,
                    );

                    // Recreate SSGI G-Buffer bind group (gbuffer views changed)
                    let ssgi_nearest_sampler = state.gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("SSGI GBuffer Nearest Sampler"),
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        address_mode_w: wgpu::AddressMode::ClampToEdge,
                        mag_filter: wgpu::FilterMode::Nearest,
                        min_filter: wgpu::FilterMode::Nearest,
                        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                        ..Default::default()
                    });
                    state.ssgi_gbuffer_bind_group = create_ssgi_gbuffer_bind_group(
                        &state.gpu.device,
                        &state.ssgi_gb_layout,
                        &state.gbuffer,
                        &ssgi_nearest_sampler,
                    );

                    // Resize RT shadow output texture
                    state.rt_shadow.resize(&state.gpu.device, size.width, size.height);

                    // Recreate RT shadow bind groups (gbuffer + rt_shadow views changed)
                    state.rt_shadow_gb_bg = create_rt_shadow_gbuffer_bg(
                        &state.gpu.device,
                        &state.rt_shadow_gb_layout,
                        &state.gbuffer,
                    );
                    state.rt_shadow_data_bg = create_rt_shadow_data_bg(
                        &state.gpu.device,
                        &state.rt_shadow_data_layout,
                        &state.rt_accel,
                        &state.rt_shadow,
                    );

                    // Recreate shadow bind group (ssgi output_view + rt_shadow changed)
                    let ssgi_filtering_sampler = state.gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("SSGI Filtering Sampler"),
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        address_mode_w: wgpu::AddressMode::ClampToEdge,
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Linear,
                        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                        ..Default::default()
                    });
                    let rt_shadow_nearest_sampler = state.gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("RT Shadow Nearest Sampler"),
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        address_mode_w: wgpu::AddressMode::ClampToEdge,
                        mag_filter: wgpu::FilterMode::Nearest,
                        min_filter: wgpu::FilterMode::Nearest,
                        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                        ..Default::default()
                    });
                    state.shadow_bind_group = create_shadow_bind_group(
                        &state.gpu.device,
                        &state.shadow_layout,
                        &state._shadow_map,
                        &state._shadow_uniform_buffer,
                        &state._ibl,
                        &state.ssgi,
                        &ssgi_filtering_sampler,
                        &state.rt_shadow,
                        &rt_shadow_nearest_sampler,
                    );

                    // Resize HDR target
                    state.hdr_target.resize(&state.gpu.device, size.width, size.height);

                    // Resize bloom mip chain
                    state.bloom.resize(&state.gpu.device, size.width, size.height);

                    // Recreate bloom bind groups
                    state.bloom_down_bind_groups = create_bloom_down_bind_groups(
                        &state.gpu.device, &state.bloom_layout, &state.hdr_target, &state.bloom,
                        &state.linear_sampler, &state.bloom_threshold_buffer, &state.bloom_no_threshold_buffer,
                    );
                    state.bloom_up_bind_groups = create_bloom_up_bind_groups(
                        &state.gpu.device, &state.bloom_layout, &state.bloom,
                        &state.linear_sampler, &state.bloom_no_threshold_buffer,
                    );

                    // Recreate tonemap bind group
                    state.tonemap_bind_group = create_tonemap_bind_group(
                        &state.gpu.device, &state.tonemap_layout, &state.hdr_target, &state.bloom,
                        &state.linear_sampler, &state.tonemap_uniform_buffer,
                    );
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.input.process_mouse_move(position.x, position.y);
            }

            WindowEvent::MouseInput {
                state: btn_state,
                button,
                ..
            } => {
                let pressed = btn_state == winit::event::ElementState::Pressed;
                state.input.process_mouse_button(button, pressed);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                state.input.process_scroll(y);
            }

            WindowEvent::RedrawRequested => {
                state.timer.tick();
                let dt = state.timer.frame_dt();
                state.time_elapsed += dt;

                // Camera input
                if state.input.is_mouse_button_pressed(winit::event::MouseButton::Right) {
                    let (dx, dy) = state.input.mouse_delta();
                    state.fps_controller.process_mouse(&mut state.camera, dx, dy);
                }
                let mut forward = 0.0f32;
                let mut right = 0.0f32;
                let mut up = 0.0f32;
                if state.input.is_key_pressed(KeyCode::KeyW) { forward += 1.0; }
                if state.input.is_key_pressed(KeyCode::KeyS) { forward -= 1.0; }
                if state.input.is_key_pressed(KeyCode::KeyD) { right += 1.0; }
                if state.input.is_key_pressed(KeyCode::KeyA) { right -= 1.0; }
                if state.input.is_key_pressed(KeyCode::Space) { up += 1.0; }
                if state.input.is_key_pressed(KeyCode::ShiftLeft) { up -= 1.0; }
                state.fps_controller.process_movement(&mut state.camera, forward, right, up, dt);
                state.input.begin_frame();

                // Compute view-projection
                let view_proj = state.camera.view_projection();
                let cam_pos = [
                    state.camera.position.x,
                    state.camera.position.y,
                    state.camera.position.z,
                ];

                let cam_aligned = state.cam_aligned_size as usize;
                let mat_aligned = state.mat_aligned_size as usize;

                // Build staging data for camera and material uniforms
                let cam_total = NUM_SPHERES * cam_aligned;
                let mat_total = NUM_SPHERES * mat_aligned;
                let mut cam_staging = vec![0u8; cam_total];
                let mut mat_staging = vec![0u8; mat_total];

                let half_grid = (GRID_SIZE as f32 - 1.0) * SPACING * 0.5;

                for row in 0..GRID_SIZE {
                    for col in 0..GRID_SIZE {
                        let i = row * GRID_SIZE + col;

                        let x = col as f32 * SPACING - half_grid;
                        let y = row as f32 * SPACING - half_grid;

                        let model = Mat4::translation(x, y, 0.0);
                        let cam_uniform = CameraUniform {
                            view_proj: view_proj.cols,
                            model: model.cols,
                            camera_pos: cam_pos,
                            _padding: 0.0,
                        };
                        let bytes = bytemuck::bytes_of(&cam_uniform);
                        let offset = i * cam_aligned;
                        cam_staging[offset..offset + bytes.len()].copy_from_slice(bytes);

                        // Material: metallic varies with col, roughness with row
                        let metallic = col as f32 / (GRID_SIZE as f32 - 1.0);
                        let roughness = 0.05 + row as f32 * (0.95 / (GRID_SIZE as f32 - 1.0));
                        let mat_uniform = MaterialUniform::with_params(
                            [0.8, 0.2, 0.2, 1.0],
                            metallic,
                            roughness,
                        );
                        let bytes = bytemuck::bytes_of(&mat_uniform);
                        let offset = i * mat_aligned;
                        mat_staging[offset..offset + bytes.len()].copy_from_slice(bytes);
                    }
                }

                state.gpu.queue.write_buffer(&state.camera_buffer, 0, &cam_staging);
                state.gpu.queue.write_buffer(&state.material_buffer, 0, &mat_staging);

                // Build lights: 1 directional + 8 animated point lights
                let mut lights_uniform = LightsUniform::new();
                lights_uniform.add_light(LightData::directional(
                    [-0.5, -1.0, -0.5],
                    [1.0, 1.0, 1.0],
                    0.3,
                ));

                let t = state.time_elapsed;
                let orbit_radius = 3.5;
                for i in 0..NUM_POINT_LIGHTS {
                    let angle = t * 0.8 + (i as f32) * std::f32::consts::TAU / NUM_POINT_LIGHTS as f32;
                    let px = orbit_radius * angle.cos();
                    let pz = orbit_radius * angle.sin();
                    let py = 1.0 * ((t * 1.5 + i as f32 * 0.7).sin());

                    // Animated color cycling through hues
                    let hue = (i as f32 / NUM_POINT_LIGHTS as f32 + t * 0.1) % 1.0;
                    let (r, g, b) = hue_to_rgb(hue);

                    lights_uniform.add_light(LightData::point(
                        [px, py, pz],
                        [r, g, b],
                        2.0,
                        10.0,
                    ));
                }

                state.gpu.queue.write_buffer(
                    &state.light_buffer,
                    0,
                    bytemuck::cast_slice(&[lights_uniform]),
                );

                // Camera position for lighting pass
                let cam_pos_uniform = CameraPositionUniform {
                    camera_pos: cam_pos,
                    _padding: 0.0,
                };
                state.gpu.queue.write_buffer(
                    &state.cam_pos_buffer,
                    0,
                    bytemuck::cast_slice(&[cam_pos_uniform]),
                );

                // Update SSGI uniform with current camera matrices
                let proj_mat = state.camera.projection_matrix();
                let view_mat = state.camera.view_matrix();
                let ssgi_uniform = SsgiUniform {
                    projection: *proj_mat.as_slice(),
                    view: *view_mat.as_slice(),
                    ..SsgiUniform::default()
                };
                state.gpu.queue.write_buffer(
                    &state.ssgi.uniform_buffer,
                    0,
                    bytemuck::bytes_of(&ssgi_uniform),
                );

                // Render
                let output = match state.gpu.surface.get_current_texture() {
                    Ok(t) => t,
                    Err(wgpu::SurfaceError::Lost) => {
                        let (w, h) = state.window.inner_size();
                        state.gpu.resize(w, h);
                        return;
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        event_loop.exit();
                        return;
                    }
                    Err(_) => return,
                };

                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("Deferred Render Encoder"),
                    },
                );

                // ---- Pass 1: G-Buffer ----
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("GBuffer Pass"),
                        color_attachments: &[
                            Some(wgpu::RenderPassColorAttachment {
                                view: &state.gbuffer.position_view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
                                    store: wgpu::StoreOp::Store,
                                },
                            }),
                            Some(wgpu::RenderPassColorAttachment {
                                view: &state.gbuffer.normal_view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
                                    store: wgpu::StoreOp::Store,
                                },
                            }),
                            Some(wgpu::RenderPassColorAttachment {
                                view: &state.gbuffer.albedo_view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
                                    store: wgpu::StoreOp::Store,
                                },
                            }),
                            Some(wgpu::RenderPassColorAttachment {
                                view: &state.gbuffer.material_view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
                                    store: wgpu::StoreOp::Store,
                                },
                            }),
                        ],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &state.gbuffer.depth_view,
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

                    rpass.set_pipeline(&state.gbuffer_pipeline);
                    rpass.set_bind_group(1, &state.pbr_texture_bind_group, &[]);
                    rpass.set_vertex_buffer(0, state.mesh.vertex_buffer.slice(..));
                    rpass.set_index_buffer(
                        state.mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );

                    for i in 0..NUM_SPHERES {
                        let cam_offset = (i as u32) * state.cam_aligned_size;
                        let mat_offset = (i as u32) * state.mat_aligned_size;
                        rpass.set_bind_group(0, &state.camera_bind_group, &[cam_offset]);
                        rpass.set_bind_group(2, &state.material_bind_group, &[mat_offset]);
                        rpass.draw_indexed(0..state.mesh.num_indices, 0, 0..1);
                    }
                }

                // ---- Pass 2: SSGI ----
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("SSGI Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &state.ssgi.output_view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                        multiview_mask: None,
                    });

                    rpass.set_pipeline(&state.ssgi_pipeline);
                    rpass.set_bind_group(0, &state.ssgi_gbuffer_bind_group, &[]);
                    rpass.set_bind_group(1, &state.ssgi_data_bind_group, &[]);
                    rpass.set_vertex_buffer(0, state.fullscreen_vb.slice(..));
                    rpass.draw(0..3, 0..1);
                }

                // ---- Pass 3: RT Shadow (compute) ----
                {
                    // Light direction from the directional light
                    let light_dir = Vec3::new(-0.5, -1.0, -0.5).normalize();
                    let rt_uniform = RtShadowUniform {
                        light_direction: [light_dir.x, light_dir.y, light_dir.z],
                        _pad0: 0.0,
                        width: state.gpu.config.width,
                        height: state.gpu.config.height,
                        _pad1: [0; 2],
                    };
                    state.gpu.queue.write_buffer(
                        &state.rt_shadow.uniform_buffer,
                        0,
                        bytemuck::bytes_of(&rt_uniform),
                    );

                    let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("RT Shadow Pass"),
                        ..Default::default()
                    });
                    cpass.set_pipeline(&state.rt_shadow_pipeline);
                    cpass.set_bind_group(0, &state.rt_shadow_gb_bg, &[]);
                    cpass.set_bind_group(1, &state.rt_shadow_data_bg, &[]);
                    cpass.dispatch_workgroups(
                        (state.gpu.config.width + 7) / 8,
                        (state.gpu.config.height + 7) / 8,
                        1,
                    );
                }

                // ---- Pass 4: Lighting → HDR target ----
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Lighting Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &state.hdr_target.view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.02,
                                    g: 0.02,
                                    b: 0.03,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                        multiview_mask: None,
                    });

                    rpass.set_pipeline(&state.lighting_pipeline);
                    rpass.set_bind_group(0, &state.gbuffer_bind_group, &[]);
                    rpass.set_bind_group(1, &state.lights_bind_group, &[]);
                    rpass.set_bind_group(2, &state.shadow_bind_group, &[]);
                    rpass.set_vertex_buffer(0, state.fullscreen_vb.slice(..));
                    rpass.draw(0..3, 0..1);
                }

                // ---- Pass 5: Bloom downsample chain ----
                {
                    let ms = mip_sizes(state.gpu.config.width, state.gpu.config.height);
                    for i in 0..BLOOM_MIP_COUNT {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Bloom Downsample"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &state.bloom.mip_views[i],
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            occlusion_query_set: None,
                            timestamp_writes: None,
                            multiview_mask: None,
                        });
                        let (mw, mh) = ms[i];
                        rpass.set_viewport(0.0, 0.0, mw as f32, mh as f32, 0.0, 1.0);
                        rpass.set_pipeline(&state.bloom_down_pipeline);
                        rpass.set_bind_group(0, &state.bloom_down_bind_groups[i], &[]);
                        rpass.set_vertex_buffer(0, state.fullscreen_vb.slice(..));
                        rpass.draw(0..3, 0..1);
                    }
                }

                // ---- Pass 6: Bloom upsample chain ----
                {
                    let ms = mip_sizes(state.gpu.config.width, state.gpu.config.height);
                    for j in 0..BLOOM_MIP_COUNT - 1 {
                        let target_mip = BLOOM_MIP_COUNT - 2 - j; // 3, 2, 1, 0
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Bloom Upsample"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &state.bloom.mip_views[target_mip],
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            occlusion_query_set: None,
                            timestamp_writes: None,
                            multiview_mask: None,
                        });
                        let (mw, mh) = ms[target_mip];
                        rpass.set_viewport(0.0, 0.0, mw as f32, mh as f32, 0.0, 1.0);
                        rpass.set_pipeline(&state.bloom_up_pipeline);
                        rpass.set_bind_group(0, &state.bloom_up_bind_groups[j], &[]);
                        rpass.set_vertex_buffer(0, state.fullscreen_vb.slice(..));
                        rpass.draw(0..3, 0..1);
                    }
                }

                // ---- Pass 7: Tonemap → surface ----
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Tonemap Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                        multiview_mask: None,
                    });

                    rpass.set_pipeline(&state.tonemap_pipeline);
                    rpass.set_bind_group(0, &state.tonemap_bind_group, &[]);
                    rpass.set_vertex_buffer(0, state.fullscreen_vb.slice(..));
                    rpass.draw(0..3, 0..1);
                }

                state.gpu.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

/// Helper: create the lighting pass G-Buffer bind group.
fn create_gbuffer_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    gbuffer: &GBuffer,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Lighting GBuffer Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&gbuffer.position_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&gbuffer.normal_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&gbuffer.albedo_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&gbuffer.material_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// Helper: create the SSGI pass G-Buffer bind group.
fn create_ssgi_gbuffer_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    gbuffer: &GBuffer,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("SSGI GBuffer Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&gbuffer.position_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&gbuffer.normal_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&gbuffer.albedo_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// Helper: create the shadow + IBL + SSGI + RT shadow bind group (9 entries).
fn create_shadow_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    shadow_map: &ShadowMap,
    shadow_uniform_buffer: &wgpu::Buffer,
    ibl: &IblResources,
    ssgi: &SsgiResources,
    ssgi_sampler: &wgpu::Sampler,
    rt_shadow: &RtShadowResources,
    rt_shadow_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Lighting Shadow+IBL+SSGI+RTShadow Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&shadow_map.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&shadow_map.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: shadow_uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&ibl.brdf_lut_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&ibl.brdf_lut_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&ssgi.output_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::Sampler(ssgi_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(&rt_shadow.shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::Sampler(rt_shadow_sampler),
            },
        ],
    })
}

/// Helper: create the RT shadow G-Buffer bind group (position + normal).
fn create_rt_shadow_gbuffer_bg(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    gbuffer: &GBuffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("RT Shadow GBuffer Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&gbuffer.position_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&gbuffer.normal_view),
            },
        ],
    })
}

/// Helper: create the RT shadow data bind group (TLAS + shadow output + uniform).
fn create_rt_shadow_data_bg(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    rt_accel: &RtAccel,
    rt_shadow: &RtShadowResources,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("RT Shadow Data Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::AccelerationStructure(&rt_accel.tlas),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&rt_shadow.shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: rt_shadow.uniform_buffer.as_entire_binding(),
            },
        ],
    })
}

/// Helper: create bloom downsample bind groups (5 total).
/// Pass 0 reads from hdr_target, uses threshold buffer.
/// Passes 1-4 read from the previous mip, use no-threshold buffer.
fn create_bloom_down_bind_groups(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    hdr_target: &HdrTarget,
    bloom: &BloomResources,
    sampler: &wgpu::Sampler,
    threshold_buf: &wgpu::Buffer,
    no_threshold_buf: &wgpu::Buffer,
) -> Vec<wgpu::BindGroup> {
    (0..BLOOM_MIP_COUNT)
        .map(|i| {
            let input_view = if i == 0 { &hdr_target.view } else { &bloom.mip_views[i - 1] };
            let uniform_buf = if i == 0 { threshold_buf } else { no_threshold_buf };
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Bloom Down BG {}", i)),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform_buf.as_entire_binding(),
                    },
                ],
            })
        })
        .collect()
}

/// Helper: create bloom upsample bind groups (4 total).
/// Each reads from mip[i+1] and renders additively into mip[i].
/// Order: j=0 → input=mip[4] target=mip[3], j=1 → input=mip[3] target=mip[2], etc.
fn create_bloom_up_bind_groups(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    bloom: &BloomResources,
    sampler: &wgpu::Sampler,
    no_threshold_buf: &wgpu::Buffer,
) -> Vec<wgpu::BindGroup> {
    (0..BLOOM_MIP_COUNT - 1)
        .map(|j| {
            let source_mip = BLOOM_MIP_COUNT - 1 - j; // 4, 3, 2, 1
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Bloom Up BG {}", j)),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&bloom.mip_views[source_mip]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: no_threshold_buf.as_entire_binding(),
                    },
                ],
            })
        })
        .collect()
}

/// Helper: create the tonemap bind group.
fn create_tonemap_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    hdr_target: &HdrTarget,
    bloom: &BloomResources,
    sampler: &wgpu::Sampler,
    uniform_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Tonemap Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&hdr_target.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&bloom.mip_views[0]),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: uniform_buffer.as_entire_binding(),
            },
        ],
    })
}

/// Convert HSV hue (0..1) to RGB.
fn hue_to_rgb(h: f32) -> (f32, f32, f32) {
    let h6 = h * 6.0;
    let f = h6 - h6.floor();
    let sector = h6.floor() as i32 % 6;
    match sector {
        0 => (1.0, f, 0.0),
        1 => (1.0 - f, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, 1.0 - f, 1.0),
        4 => (f, 0.0, 1.0),
        _ => (1.0, 0.0, 1.0 - f),
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = DeferredDemoApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
