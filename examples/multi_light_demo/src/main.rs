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
    Mesh, GpuTexture, MaterialUniform, generate_sphere, create_pbr_pipeline, obj,
    ShadowMap, ShadowUniform,
    IblResources, pbr_full_texture_bind_group_layout, create_pbr_full_texture_bind_group,
};
use wgpu::util::DeviceExt;

const NUM_OBJECTS: usize = 6; // 5 spheres + 1 ground plane

struct MultiLightApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    pipeline: wgpu::RenderPipeline,
    sphere_mesh: Mesh,
    ground_mesh: Mesh,
    camera: Camera,
    fps_controller: FpsController,
    camera_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    material_buffer: wgpu::Buffer,
    camera_light_bind_group: wgpu::BindGroup,
    _albedo_tex: GpuTexture,
    _normal_tex: (wgpu::Texture, wgpu::TextureView, wgpu::Sampler),
    _orm_tex: GpuTexture,
    _emissive_tex: GpuTexture,
    pbr_texture_bind_group: wgpu::BindGroup,
    material_bind_group: wgpu::BindGroup,
    shadow_bind_group: wgpu::BindGroup,
    _shadow_map: ShadowMap,
    _ibl: IblResources,
    input: InputState,
    timer: GameTimer,
    cam_aligned_size: u32,
    mat_aligned_size: u32,
    time: f32,
}

fn camera_light_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera+Light Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<CameraUniform>() as u64,
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

fn align_up(size: u32, alignment: u32) -> u32 {
    size.div_ceil(alignment) * alignment
}

impl ApplicationHandler for MultiLightApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Multi-Light Demo".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new(window.handle.clone()).expect("voltex: GPU initialization failed");

        // Dynamic uniform buffer alignment
        let alignment = gpu.device.limits().min_uniform_buffer_offset_alignment;
        let cam_aligned_size = align_up(std::mem::size_of::<CameraUniform>() as u32, alignment);
        let mat_aligned_size = align_up(std::mem::size_of::<MaterialUniform>() as u32, alignment);

        // Generate sphere mesh (shared by all 5 spheres)
        let (vertices, indices) = generate_sphere(0.5, 32, 16);
        let sphere_mesh = Mesh::new(&gpu.device, &vertices, &indices);

        // Ground plane: cube.obj scaled to (10, 0.1, 10)
        let obj_src = include_str!("../../../assets/cube.obj");
        let obj_data = obj::parse_obj(obj_src);
        let ground_mesh = Mesh::new(&gpu.device, &obj_data.vertices, &obj_data.indices);

        // Camera at (0, 5, 12), looking down slightly
        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
        let mut camera = Camera::new(Vec3::new(0.0, 5.0, 12.0), aspect);
        camera.pitch = -0.3;
        let fps_controller = FpsController::new();

        // Initial lights uniform
        let mut lights_uniform = LightsUniform::new();
        lights_uniform.add_light(LightData::directional([0.0, -1.0, -0.5], [1.0, 1.0, 1.0], 0.3));
        // Point lights at initial positions (will be updated per frame)
        lights_uniform.add_light(LightData::point([5.0, 2.0, 0.0], [1.0, 0.0, 0.0], 15.0, 15.0));
        lights_uniform.add_light(LightData::point([0.0, 2.0, 5.0], [0.0, 1.0, 0.0], 15.0, 15.0));
        lights_uniform.add_light(LightData::point([-5.0, 2.0, 0.0], [0.0, 0.0, 1.0], 15.0, 15.0));
        lights_uniform.add_light(LightData::point([0.0, 2.0, -5.0], [1.0, 1.0, 0.0], 15.0, 15.0));
        lights_uniform.add_light(LightData::spot(
            [0.0, 5.0, 0.0], [0.0, -1.0, 0.0], [1.0, 1.0, 1.0],
            20.0, 10.0, 20.0, 35.0,
        ));

        // Camera dynamic uniform buffer (one CameraUniform per object)
        let camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Dynamic Uniform Buffer"),
            size: (cam_aligned_size as usize * NUM_OBJECTS) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Uniform Buffer"),
            contents: bytemuck::cast_slice(&[lights_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Material dynamic uniform buffer (one MaterialUniform per object)
        let material_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Material Dynamic Uniform Buffer"),
            size: (mat_aligned_size as usize * NUM_OBJECTS) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layouts
        let cl_layout = camera_light_bind_group_layout(&gpu.device);
        let pbr_tex_layout = pbr_full_texture_bind_group_layout(&gpu.device);
        let mat_layout = MaterialUniform::bind_group_layout(&gpu.device);

        // Camera+Light bind group
        let camera_light_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera+Light Bind Group"),
            layout: &cl_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &camera_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(
                            std::mem::size_of::<CameraUniform>() as u64,
                        ),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        // PBR texture bind group (albedo + normal + ORM + emissive)
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

        // IBL resources
        let ibl = IblResources::new(&gpu.device, &gpu.queue);

        // Material bind group
        let material_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &mat_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &material_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(
                        std::mem::size_of::<MaterialUniform>() as u64,
                    ),
                }),
            }],
        });

        // Shadow resources (dummy — shadows disabled)
        let shadow_map = ShadowMap::new(&gpu.device);
        let shadow_layout = ShadowMap::bind_group_layout(&gpu.device);
        let shadow_uniform = ShadowUniform {
            light_view_proj: [[0.0; 4]; 4],
            shadow_map_size: 0.0,
            shadow_bias: 0.0,
            _padding: [0.0; 2],
            sun_direction: [0.5, -0.7, 0.5],
            turbidity: 3.0,
            sh_coefficients: [[0.0; 4]; 7],
        };
        let shadow_uniform_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Uniform Buffer"),
            contents: bytemuck::cast_slice(&[shadow_uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let shadow_bind_group = shadow_map.create_bind_group(
            &gpu.device,
            &shadow_layout,
            &shadow_uniform_buffer,
            &ibl.brdf_lut_view,
            &ibl.brdf_lut_sampler,
        );

        // PBR pipeline
        let pipeline = create_pbr_pipeline(
            &gpu.device,
            gpu.surface_format,
            &cl_layout,
            &pbr_tex_layout,
            &mat_layout,
            &shadow_layout,
        );

        self.state = Some(AppState {
            window,
            gpu,
            pipeline,
            sphere_mesh,
            ground_mesh,
            camera,
            fps_controller,
            camera_buffer,
            light_buffer,
            material_buffer,
            camera_light_bind_group,
            _albedo_tex: albedo_tex,
            _normal_tex: normal_tex,
            _orm_tex: orm_tex,
            _emissive_tex: emissive_tex,
            pbr_texture_bind_group,
            material_bind_group,
            shadow_bind_group,
            _shadow_map: shadow_map,
            _ibl: ibl,
            input: InputState::new(),
            timer: GameTimer::new(60),
            cam_aligned_size,
            mat_aligned_size,
            time: 0.0,
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

                // Camera input
                if state
                    .input
                    .is_mouse_button_pressed(winit::event::MouseButton::Right)
                {
                    let (dx, dy) = state.input.mouse_delta();
                    state
                        .fps_controller
                        .process_mouse(&mut state.camera, dx, dy);
                }
                let mut forward = 0.0f32;
                let mut right = 0.0f32;
                let mut up = 0.0f32;
                if state.input.is_key_pressed(KeyCode::KeyW) {
                    forward += 1.0;
                }
                if state.input.is_key_pressed(KeyCode::KeyS) {
                    forward -= 1.0;
                }
                if state.input.is_key_pressed(KeyCode::KeyD) {
                    right += 1.0;
                }
                if state.input.is_key_pressed(KeyCode::KeyA) {
                    right -= 1.0;
                }
                if state.input.is_key_pressed(KeyCode::Space) {
                    up += 1.0;
                }
                if state.input.is_key_pressed(KeyCode::ShiftLeft) {
                    up -= 1.0;
                }
                state
                    .fps_controller
                    .process_movement(&mut state.camera, forward, right, up, dt);
                state.input.begin_frame();

                state.time += dt;

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
                let cam_total = NUM_OBJECTS * cam_aligned;
                let mat_total = NUM_OBJECTS * mat_aligned;
                let mut cam_staging = vec![0u8; cam_total];
                let mut mat_staging = vec![0u8; mat_total];

                // Object layout: indices 0..4 = spheres, index 5 = ground plane
                // Spheres at y=0, x = [-4, -2, 0, 2, 4], metallic varies 0.0..1.0
                for i in 0..5usize {
                    let x = -4.0 + i as f32 * 2.0;
                    let model = Mat4::translation(x, 0.0, 0.0);
                    let cam_uniform = CameraUniform {
                        view_proj: view_proj.cols,
                        model: model.cols,
                        camera_pos: cam_pos,
                        _padding: 0.0,
                    };
                    let bytes = bytemuck::bytes_of(&cam_uniform);
                    let offset = i * cam_aligned;
                    cam_staging[offset..offset + bytes.len()].copy_from_slice(bytes);

                    let metallic = i as f32 / 4.0;
                    let mat_uniform = MaterialUniform::with_params(
                        [0.8, 0.2, 0.2, 1.0],
                        metallic,
                        0.3,
                    );
                    let bytes = bytemuck::bytes_of(&mat_uniform);
                    let offset = i * mat_aligned;
                    mat_staging[offset..offset + bytes.len()].copy_from_slice(bytes);
                }

                // Ground plane at y=-0.5, scale (10, 0.1, 10)
                {
                    let i = 5;
                    let model = Mat4::translation(0.0, -0.5, 0.0)
                        .mul_mat4(&Mat4::scale(10.0, 0.1, 10.0));
                    let cam_uniform = CameraUniform {
                        view_proj: view_proj.cols,
                        model: model.cols,
                        camera_pos: cam_pos,
                        _padding: 0.0,
                    };
                    let bytes = bytemuck::bytes_of(&cam_uniform);
                    let offset = i * cam_aligned;
                    cam_staging[offset..offset + bytes.len()].copy_from_slice(bytes);

                    let mat_uniform = MaterialUniform::with_params(
                        [0.5, 0.5, 0.5, 1.0],
                        0.0,
                        0.8,
                    );
                    let bytes = bytemuck::bytes_of(&mat_uniform);
                    let offset = i * mat_aligned;
                    mat_staging[offset..offset + bytes.len()].copy_from_slice(bytes);
                }

                state
                    .gpu
                    .queue
                    .write_buffer(&state.camera_buffer, 0, &cam_staging);
                state
                    .gpu
                    .queue
                    .write_buffer(&state.material_buffer, 0, &mat_staging);

                // Update lights with orbiting point lights
                let radius = 5.0f32;
                let time = state.time;
                let mut lights_uniform = LightsUniform::new();

                // Directional fill light
                lights_uniform.add_light(LightData::directional(
                    [0.0, -1.0, -0.5], [1.0, 1.0, 1.0], 0.3,
                ));

                // 4 orbiting point lights
                let offsets = [0.0f32, std::f32::consts::FRAC_PI_2, std::f32::consts::PI, 3.0 * std::f32::consts::FRAC_PI_2];
                let colors = [
                    [1.0, 0.0, 0.0], // Red
                    [0.0, 1.0, 0.0], // Green
                    [0.0, 0.0, 1.0], // Blue
                    [1.0, 1.0, 0.0], // Yellow
                ];
                for j in 0..4 {
                    let angle = time + offsets[j];
                    let px = radius * angle.cos();
                    let pz = radius * angle.sin();
                    lights_uniform.add_light(LightData::point(
                        [px, 2.0, pz], colors[j], 15.0, 15.0,
                    ));
                }

                // Spot light from above
                lights_uniform.add_light(LightData::spot(
                    [0.0, 5.0, 0.0], [0.0, -1.0, 0.0], [1.0, 1.0, 1.0],
                    20.0, 10.0, 20.0, 35.0,
                ));

                state.gpu.queue.write_buffer(
                    &state.light_buffer,
                    0,
                    bytemuck::cast_slice(&[lights_uniform]),
                );

                // Render
                let output = match state.gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    wgpu::CurrentSurfaceTexture::Lost => {
                        let (w, h) = state.window.inner_size();
                        state.gpu.resize(w, h);
                        return;
                    }
                    _ => return,
                };

                let view =
                    output
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    },
                );

                {
                    let mut render_pass =
                        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Multi-Light Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.05,
                                        g: 0.05,
                                        b: 0.08,
                                        a: 1.0,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(
                                wgpu::RenderPassDepthStencilAttachment {
                                    view: &state.gpu.depth_view,
                                    depth_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(1.0),
                                        store: wgpu::StoreOp::Store,
                                    }),
                                    stencil_ops: None,
                                },
                            ),
                            occlusion_query_set: None,
                            timestamp_writes: None,
                            multiview_mask: None,
                        });

                    render_pass.set_pipeline(&state.pipeline);
                    render_pass.set_bind_group(1, &state.pbr_texture_bind_group, &[]);
                    render_pass.set_bind_group(3, &state.shadow_bind_group, &[]);

                    // Draw 5 spheres (objects 0..4)
                    render_pass.set_vertex_buffer(0, state.sphere_mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        state.sphere_mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    for i in 0..5u32 {
                        let cam_offset = i * state.cam_aligned_size;
                        let mat_offset = i * state.mat_aligned_size;
                        render_pass.set_bind_group(
                            0,
                            &state.camera_light_bind_group,
                            &[cam_offset],
                        );
                        render_pass.set_bind_group(
                            2,
                            &state.material_bind_group,
                            &[mat_offset],
                        );
                        render_pass.draw_indexed(0..state.sphere_mesh.num_indices, 0, 0..1);
                    }

                    // Draw ground plane (object 5)
                    render_pass.set_vertex_buffer(0, state.ground_mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        state.ground_mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    {
                        let cam_offset = 5u32 * state.cam_aligned_size;
                        let mat_offset = 5u32 * state.mat_aligned_size;
                        render_pass.set_bind_group(
                            0,
                            &state.camera_light_bind_group,
                            &[cam_offset],
                        );
                        render_pass.set_bind_group(
                            2,
                            &state.material_bind_group,
                            &[mat_offset],
                        );
                        render_pass.draw_indexed(0..state.ground_mesh.num_indices, 0, 0..1);
                    }
                }

                state.gpu.queue.submit(std::iter::once(encoder.finish()));
                state.gpu.queue.present(output);
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

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = MultiLightApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
