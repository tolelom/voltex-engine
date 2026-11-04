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
    Mesh, GpuTexture, MaterialUniform, generate_sphere, create_pbr_pipeline,
    ShadowMap, ShadowUniform,
    IblResources, pbr_full_texture_bind_group_layout, create_pbr_full_texture_bind_group,
};
use wgpu::util::DeviceExt;

const GRID_SIZE: usize = 7;
const NUM_SPHERES: usize = GRID_SIZE * GRID_SIZE;
const SPACING: f32 = 1.2;

struct IblDemoApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    pipeline: wgpu::RenderPipeline,
    mesh: Mesh,
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

impl ApplicationHandler for IblDemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - IBL Demo".to_string(),
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

        // Generate sphere mesh
        let (vertices, indices) = generate_sphere(0.4, 32, 16);
        let mesh = Mesh::new(&gpu.device, &vertices, &indices);

        // Camera at (0, 0, 12) looking toward origin
        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
        let camera = Camera::new(Vec3::new(0.0, 0.0, 12.0), aspect);
        let fps_controller = FpsController::new();

        // Mild directional light — IBL provides the primary illumination
        let mut lights_uniform = LightsUniform::new();
        lights_uniform.add_light(LightData::directional(
            [-1.0, -1.0, -1.0],
            [1.0, 1.0, 1.0],
            1.0,
        ));

        // Camera dynamic uniform buffer (one CameraUniform per sphere)
        let camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Dynamic Uniform Buffer"),
            size: (cam_aligned_size as usize * NUM_SPHERES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Uniform Buffer"),
            contents: bytemuck::cast_slice(&[lights_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Material dynamic uniform buffer (one MaterialUniform per sphere)
        let material_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Material Dynamic Uniform Buffer"),
            size: (mat_aligned_size as usize * NUM_SPHERES) as u64,
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
            mesh,
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

                        // Camera uniform: view_proj + model (translation) + camera_pos
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
                        // Reddish base color for IBL visibility
                        let metallic = col as f32 / (GRID_SIZE as f32 - 1.0);
                        let roughness =
                            0.05 + row as f32 * (0.95 / (GRID_SIZE as f32 - 1.0));
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

                state
                    .gpu
                    .queue
                    .write_buffer(&state.camera_buffer, 0, &cam_staging);
                state
                    .gpu
                    .queue
                    .write_buffer(&state.material_buffer, 0, &mat_staging);

                // Write light uniform (mild directional — IBL provides ambient)
                let mut lights_uniform = LightsUniform::new();
                lights_uniform.add_light(LightData::directional(
                    [-1.0, -1.0, -1.0],
                    [1.0, 1.0, 1.0],
                    1.0,
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
                            label: Some("IBL Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
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
                    render_pass.set_vertex_buffer(0, state.mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        state.mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );

                    // Draw each sphere with dynamic offsets for camera and material
                    for i in 0..NUM_SPHERES {
                        let cam_offset = (i as u32) * state.cam_aligned_size;
                        let mat_offset = (i as u32) * state.mat_aligned_size;
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
                        render_pass.draw_indexed(0..state.mesh.num_indices, 0, 0..1);
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
    let mut app = IblDemoApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
