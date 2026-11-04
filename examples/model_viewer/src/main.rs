use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_math::{Vec3, Mat4};
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::{GpuContext, Camera, FpsController, CameraUniform, LightUniform, Mesh, GpuTexture, pipeline, obj};
use wgpu::util::DeviceExt;

struct ModelViewerApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    pipeline: wgpu::RenderPipeline,
    mesh: Mesh,
    camera: Camera,
    fps_controller: FpsController,
    camera_uniform: CameraUniform,
    light_uniform: LightUniform,
    camera_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    camera_light_bind_group: wgpu::BindGroup,
    _texture: GpuTexture,
    input: InputState,
    timer: GameTimer,
    angle: f32,
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
                    has_dynamic_offset: false,
                    min_binding_size: None,
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

impl ApplicationHandler for ModelViewerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Model Viewer".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new(window.handle.clone()).expect("voltex: GPU initialization failed");

        // Parse OBJ
        let obj_src = include_str!("../../../assets/cube.obj");
        let obj_data = obj::parse_obj(obj_src);
        let mesh = Mesh::new(&gpu.device, &obj_data.vertices, &obj_data.indices);

        // Camera at (0, 1, 3) looking toward origin
        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
        let mut camera = Camera::new(Vec3::new(0.0, 1.0, 3.0), aspect);
        // Point camera toward origin: yaw=0 means forward is (0,0,-1) which is roughly toward origin from (0,1,3)
        // We need to look slightly down
        let dir = Vec3::new(0.0, -1.0, -3.0);
        camera.yaw = dir.x.atan2(-dir.z);
        camera.pitch = (dir.y / (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt()).asin();

        let fps_controller = FpsController::new();

        // Uniforms
        let camera_uniform = CameraUniform::new();
        let light_uniform = LightUniform::new();

        let camera_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Uniform Buffer"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layouts
        let cl_layout = camera_light_bind_group_layout(&gpu.device);
        let tex_layout = GpuTexture::bind_group_layout(&gpu.device);

        // Bind groups
        let camera_light_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera+Light Bind Group"),
            layout: &cl_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        // Default white texture
        let texture = GpuTexture::white_1x1(&gpu.device, &gpu.queue, &tex_layout);

        // Pipeline
        let render_pipeline = pipeline::create_mesh_pipeline(
            &gpu.device,
            gpu.surface_format,
            &cl_layout,
            &tex_layout,
        );

        self.state = Some(AppState {
            window,
            gpu,
            pipeline: render_pipeline,
            mesh,
            camera,
            fps_controller,
            camera_uniform,
            light_uniform,
            camera_buffer,
            light_buffer,
            camera_light_bind_group,
            _texture: texture,
            input: InputState::new(),
            timer: GameTimer::new(60),
            angle: 0.0,
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
                event: winit::event::KeyEvent {
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

            WindowEvent::MouseInput { state: btn_state, button, .. } => {
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
                // 1. Tick timer
                state.timer.tick();
                let dt = state.timer.frame_dt();

                // 2. Read input state BEFORE begin_frame clears it
                // Camera rotation via right-click drag
                if state.input.is_mouse_button_pressed(winit::event::MouseButton::Right) {
                    let (dx, dy) = state.input.mouse_delta();
                    state.fps_controller.process_mouse(&mut state.camera, dx, dy);
                }

                // WASD movement
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

                // 3. Clear per-frame input state for next frame
                state.input.begin_frame();

                // 4. Auto-rotate model
                state.angle += dt * 0.5;

                // Update uniforms
                state.camera_uniform.view_proj = state.camera.view_projection().cols;
                state.camera_uniform.model = Mat4::rotation_y(state.angle).cols;
                state.camera_uniform.camera_pos = [
                    state.camera.position.x,
                    state.camera.position.y,
                    state.camera.position.z,
                ];

                state.gpu.queue.write_buffer(
                    &state.camera_buffer,
                    0,
                    bytemuck::cast_slice(&[state.camera_uniform]),
                );
                state.gpu.queue.write_buffer(
                    &state.light_buffer,
                    0,
                    bytemuck::cast_slice(&[state.light_uniform]),
                );

                // 5. Render
                let output = match state.gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    wgpu::CurrentSurfaceTexture::Lost => {
                        let (w, h) = state.window.inner_size();
                        state.gpu.resize(w, h);
                        return;
                    }
                    _ => return,
                };

                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") },
                );

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Render Pass"),
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

                    render_pass.set_pipeline(&state.pipeline);
                    render_pass.set_bind_group(0, &state.camera_light_bind_group, &[]);
                    render_pass.set_bind_group(1, &state._texture.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, state.mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(state.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..state.mesh.num_indices, 0, 0..1);
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
    let mut app = ModelViewerApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
