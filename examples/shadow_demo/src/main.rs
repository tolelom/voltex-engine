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
    ShadowMap, ShadowUniform, ShadowPassUniform, SHADOW_MAP_SIZE,
    create_shadow_pipeline, shadow_pass_bind_group_layout,
};
use wgpu::util::DeviceExt;

/// 6 objects: ground plane, 3 spheres, 2 cubes
const NUM_OBJECTS: usize = 6;

struct ShadowDemoApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    pbr_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    sphere_mesh: Mesh,
    cube_mesh: Mesh,
    camera: Camera,
    fps_controller: FpsController,
    // Color pass resources
    camera_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    material_buffer: wgpu::Buffer,
    camera_light_bind_group: wgpu::BindGroup,
    _texture: GpuTexture,
    material_bind_group: wgpu::BindGroup,
    // Shadow resources
    shadow_map: ShadowMap,
    shadow_uniform_buffer: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
    shadow_pass_buffer: wgpu::Buffer,
    shadow_pass_bind_group: wgpu::BindGroup,
    // Misc
    input: InputState,
    timer: GameTimer,
    cam_aligned_size: u32,
    mat_aligned_size: u32,
    shadow_pass_aligned_size: u32,
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
    ((size + alignment - 1) / alignment) * alignment
}

/// Model matrices for the 6 scene objects.
fn object_models() -> [Mat4; NUM_OBJECTS] {
    [
        // 0: ground plane — cube scaled (15, 0.1, 15) at y=-0.5
        Mat4::translation(0.0, -0.5, 0.0).mul_mat4(&Mat4::scale(15.0, 0.1, 15.0)),
        // 1-3: spheres (unit sphere radius 0.5)
        Mat4::translation(-3.0, 1.0, 0.0),
        Mat4::translation(0.0, 1.5, 0.0),
        Mat4::translation(3.0, 0.8, 0.0),
        // 4-5: cubes
        Mat4::translation(-1.5, 0.5, -2.0),
        Mat4::translation(1.5, 0.5, 2.0),
    ]
}

/// Material parameters for each object (base_color, metallic, roughness).
fn object_materials() -> [([ f32; 4], f32, f32); NUM_OBJECTS] {
    [
        ([0.7, 0.7, 0.7, 1.0], 0.0, 0.8),  // ground: light gray
        ([0.9, 0.2, 0.2, 1.0], 0.3, 0.4),  // sphere: red
        ([0.2, 0.9, 0.2, 1.0], 0.5, 0.3),  // sphere: green
        ([0.2, 0.2, 0.9, 1.0], 0.1, 0.6),  // sphere: blue
        ([0.9, 0.8, 0.2, 1.0], 0.7, 0.2),  // cube: yellow
        ([0.8, 0.3, 0.8, 1.0], 0.2, 0.5),  // cube: purple
    ]
}

/// Returns true if the object at index `i` uses the cube mesh; false → sphere.
fn is_cube(i: usize) -> bool {
    i == 0 || i == 4 || i == 5
}

impl ApplicationHandler for ShadowDemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Shadow Demo".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new(window.handle.clone());

        // Dynamic uniform buffer alignment
        let alignment = gpu.device.limits().min_uniform_buffer_offset_alignment;
        let cam_aligned_size = align_up(std::mem::size_of::<CameraUniform>() as u32, alignment);
        let mat_aligned_size = align_up(std::mem::size_of::<MaterialUniform>() as u32, alignment);
        let shadow_pass_aligned_size = align_up(std::mem::size_of::<ShadowPassUniform>() as u32, alignment);

        // Meshes
        let (sphere_verts, sphere_idx) = generate_sphere(0.5, 32, 16);
        let sphere_mesh = Mesh::new(&gpu.device, &sphere_verts, &sphere_idx);

        let obj_src = include_str!("../../../assets/cube.obj");
        let obj_data = obj::parse_obj(obj_src);
        let cube_mesh = Mesh::new(&gpu.device, &obj_data.vertices, &obj_data.indices);

        // Camera at (8, 8, 12) looking toward origin
        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
        let mut camera = Camera::new(Vec3::new(8.0, 8.0, 12.0), aspect);
        camera.pitch = -0.4;
        // Compute yaw to look toward origin
        let to_origin = Vec3::ZERO - camera.position;
        camera.yaw = to_origin.x.atan2(-to_origin.z);
        let fps_controller = FpsController::new();

        // Light
        let mut lights_uniform = LightsUniform::new();
        lights_uniform.ambient_color = [0.05, 0.05, 0.05];
        lights_uniform.add_light(LightData::directional(
            [-1.0, -2.0, -1.0],
            [1.0, 1.0, 1.0],
            2.0,
        ));

        // ---- Color pass buffers ----
        let camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Dynamic UBO"),
            size: (cam_aligned_size as usize * NUM_OBJECTS) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light UBO"),
            contents: bytemuck::cast_slice(&[lights_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let material_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Material Dynamic UBO"),
            size: (mat_aligned_size as usize * NUM_OBJECTS) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layouts
        let cl_layout = camera_light_bind_group_layout(&gpu.device);
        let tex_layout = GpuTexture::bind_group_layout(&gpu.device);
        let mat_layout = MaterialUniform::bind_group_layout(&gpu.device);

        // Camera+Light bind group
        let camera_light_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera+Light BG"),
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

        // Texture (white 1x1)
        let texture = GpuTexture::white_1x1(&gpu.device, &gpu.queue, &tex_layout);

        // Material bind group
        let material_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material BG"),
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

        // ---- Shadow resources ----
        let shadow_map = ShadowMap::new(&gpu.device);
        let shadow_layout = ShadowMap::bind_group_layout(&gpu.device);

        let shadow_uniform = ShadowUniform {
            light_view_proj: Mat4::IDENTITY.cols,
            shadow_map_size: SHADOW_MAP_SIZE as f32,
            shadow_bias: 0.005,
            _padding: [0.0; 2],
        };
        let shadow_uniform_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Uniform Buffer"),
            contents: bytemuck::cast_slice(&[shadow_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let shadow_bind_group = shadow_map.create_bind_group(
            &gpu.device,
            &shadow_layout,
            &shadow_uniform_buffer,
        );

        // Shadow pass dynamic UBO (one ShadowPassUniform per object)
        let sp_layout = shadow_pass_bind_group_layout(&gpu.device);
        let shadow_pass_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Pass Dynamic UBO"),
            size: (shadow_pass_aligned_size as usize * NUM_OBJECTS) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_pass_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Pass BG"),
            layout: &sp_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &shadow_pass_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ShadowPassUniform>() as u64),
                }),
            }],
        });

        // ---- Pipelines ----
        let shadow_pipeline = create_shadow_pipeline(&gpu.device, &sp_layout);
        let pbr_pipeline = create_pbr_pipeline(
            &gpu.device,
            gpu.surface_format,
            &cl_layout,
            &tex_layout,
            &mat_layout,
            &shadow_layout,
        );

        self.state = Some(AppState {
            window,
            gpu,
            pbr_pipeline,
            shadow_pipeline,
            sphere_mesh,
            cube_mesh,
            camera,
            fps_controller,
            camera_buffer,
            light_buffer,
            material_buffer,
            camera_light_bind_group,
            _texture: texture,
            material_bind_group,
            shadow_map,
            shadow_uniform_buffer,
            shadow_bind_group,
            shadow_pass_buffer,
            shadow_pass_bind_group,
            input: InputState::new(),
            timer: GameTimer::new(60),
            cam_aligned_size,
            mat_aligned_size,
            shadow_pass_aligned_size,
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

                // ----- Compute light VP -----
                let light_dir = Vec3::new(-1.0, -2.0, -1.0).normalize();
                let light_pos = Vec3::ZERO - light_dir * 20.0;
                let light_view = Mat4::look_at(light_pos, Vec3::ZERO, Vec3::Y);
                let light_proj = Mat4::orthographic(-15.0, 15.0, -15.0, 15.0, 0.1, 50.0);
                let light_vp = light_proj * light_view;

                let models = object_models();
                let materials = object_materials();

                let cam_aligned = state.cam_aligned_size as usize;
                let mat_aligned = state.mat_aligned_size as usize;
                let sp_aligned = state.shadow_pass_aligned_size as usize;

                // ----- Build shadow pass staging data -----
                let sp_total = sp_aligned * NUM_OBJECTS;
                let mut sp_staging = vec![0u8; sp_total];
                for i in 0..NUM_OBJECTS {
                    let sp_uniform = ShadowPassUniform {
                        light_vp_model: (light_vp * models[i]).cols,
                    };
                    let bytes = bytemuck::bytes_of(&sp_uniform);
                    let offset = i * sp_aligned;
                    sp_staging[offset..offset + bytes.len()].copy_from_slice(bytes);
                }
                state.gpu.queue.write_buffer(&state.shadow_pass_buffer, 0, &sp_staging);

                // ----- Build color pass staging data -----
                let view_proj = state.camera.view_projection();
                let cam_pos = [
                    state.camera.position.x,
                    state.camera.position.y,
                    state.camera.position.z,
                ];

                let cam_total = cam_aligned * NUM_OBJECTS;
                let mat_total = mat_aligned * NUM_OBJECTS;
                let mut cam_staging = vec![0u8; cam_total];
                let mut mat_staging = vec![0u8; mat_total];

                for i in 0..NUM_OBJECTS {
                    let cam_uniform = CameraUniform {
                        view_proj: view_proj.cols,
                        model: models[i].cols,
                        camera_pos: cam_pos,
                        _padding: 0.0,
                    };
                    let bytes = bytemuck::bytes_of(&cam_uniform);
                    let offset = i * cam_aligned;
                    cam_staging[offset..offset + bytes.len()].copy_from_slice(bytes);

                    let (color, metallic, roughness) = materials[i];
                    let mat_uniform = MaterialUniform::with_params(color, metallic, roughness);
                    let bytes = bytemuck::bytes_of(&mat_uniform);
                    let offset = i * mat_aligned;
                    mat_staging[offset..offset + bytes.len()].copy_from_slice(bytes);
                }

                state.gpu.queue.write_buffer(&state.camera_buffer, 0, &cam_staging);
                state.gpu.queue.write_buffer(&state.material_buffer, 0, &mat_staging);

                // Update shadow uniform with light VP
                let shadow_uniform = ShadowUniform {
                    light_view_proj: light_vp.cols,
                    shadow_map_size: SHADOW_MAP_SIZE as f32,
                    shadow_bias: 0.005,
                    _padding: [0.0; 2],
                };
                state.gpu.queue.write_buffer(
                    &state.shadow_uniform_buffer,
                    0,
                    bytemuck::cast_slice(&[shadow_uniform]),
                );

                // Write light uniform
                let mut lights_uniform = LightsUniform::new();
                lights_uniform.ambient_color = [0.05, 0.05, 0.05];
                lights_uniform.add_light(LightData::directional(
                    [-1.0, -2.0, -1.0],
                    [1.0, 1.0, 1.0],
                    2.0,
                ));
                state.gpu.queue.write_buffer(
                    &state.light_buffer,
                    0,
                    bytemuck::cast_slice(&[lights_uniform]),
                );

                // ----- Render -----
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

                let color_view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("Shadow Demo Encoder") },
                );

                // ===== Pass 1: Shadow =====
                {
                    let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Shadow Pass"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &state.shadow_map.view,
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

                    shadow_pass.set_pipeline(&state.shadow_pipeline);

                    for i in 0..NUM_OBJECTS {
                        let offset = (i as u32) * state.shadow_pass_aligned_size;
                        shadow_pass.set_bind_group(0, &state.shadow_pass_bind_group, &[offset]);

                        if is_cube(i) {
                            shadow_pass.set_vertex_buffer(0, state.cube_mesh.vertex_buffer.slice(..));
                            shadow_pass.set_index_buffer(state.cube_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            shadow_pass.draw_indexed(0..state.cube_mesh.num_indices, 0, 0..1);
                        } else {
                            shadow_pass.set_vertex_buffer(0, state.sphere_mesh.vertex_buffer.slice(..));
                            shadow_pass.set_index_buffer(state.sphere_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            shadow_pass.draw_indexed(0..state.sphere_mesh.num_indices, 0, 0..1);
                        }
                    }
                }

                // ===== Pass 2: Color (PBR) =====
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Color Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &color_view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1, g: 0.1, b: 0.15, a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &state.gpu.depth_view,
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

                    render_pass.set_pipeline(&state.pbr_pipeline);
                    render_pass.set_bind_group(1, &state._texture.bind_group, &[]);
                    render_pass.set_bind_group(3, &state.shadow_bind_group, &[]);

                    for i in 0..NUM_OBJECTS {
                        let cam_offset = (i as u32) * state.cam_aligned_size;
                        let mat_offset = (i as u32) * state.mat_aligned_size;
                        render_pass.set_bind_group(0, &state.camera_light_bind_group, &[cam_offset]);
                        render_pass.set_bind_group(2, &state.material_bind_group, &[mat_offset]);

                        if is_cube(i) {
                            render_pass.set_vertex_buffer(0, state.cube_mesh.vertex_buffer.slice(..));
                            render_pass.set_index_buffer(state.cube_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            render_pass.draw_indexed(0..state.cube_mesh.num_indices, 0, 0..1);
                        } else {
                            render_pass.set_vertex_buffer(0, state.sphere_mesh.vertex_buffer.slice(..));
                            render_pass.set_index_buffer(state.sphere_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            render_pass.draw_indexed(0..state.sphere_mesh.num_indices, 0, 0..1);
                        }
                    }
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

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = ShadowDemoApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
