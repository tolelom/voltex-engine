use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_math::Vec3;
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::{
    GpuContext, Camera, FpsController, CameraUniform, LightUniform, Mesh, GpuTexture, pipeline, obj,
};
use voltex_ecs::{World, Entity, Transform, propagate_transforms, WorldTransform};
use voltex_asset::{Assets, Handle};
use wgpu::util::DeviceExt;

/// Component: a handle into the asset system pointing at a Mesh.
struct MeshRef(#[allow(dead_code)] Handle<Mesh>);

const MAX_ENTITIES: usize = 1024;

struct AssetDemoApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    pipeline: wgpu::RenderPipeline,
    assets: Assets,
    mesh_handle: Handle<Mesh>,
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
    world: World,
    time: f32,
    uniform_alignment: u32,
    r_was_pressed: bool,
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

impl ApplicationHandler for AssetDemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Asset Demo".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new(window.handle.clone());

        // Dynamic uniform buffer alignment
        let uniform_alignment = gpu.device.limits().min_uniform_buffer_offset_alignment;
        let uniform_size = std::mem::size_of::<CameraUniform>() as u32;
        let aligned_size = uniform_size.div_ceil(uniform_alignment) * uniform_alignment;

        // Parse OBJ and create Mesh
        let obj_src = include_str!("../../../assets/cube.obj");
        let obj_data = obj::parse_obj(obj_src);
        let mesh = Mesh::new(&gpu.device, &obj_data.vertices, &obj_data.indices);

        // Insert mesh into asset system
        let mut assets = Assets::new();
        let mesh_handle = assets.insert(mesh);

        // Camera: position (0, 10, 18), pitch=-0.4
        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
        let mut camera = Camera::new(Vec3::new(0.0, 10.0, 18.0), aspect);
        camera.pitch = -0.4;

        let fps_controller = FpsController::new();

        // Uniforms
        let camera_uniform = CameraUniform::new();
        let light_uniform = LightUniform::new();

        // Dynamic uniform buffer: room for MAX_ENTITIES camera uniforms
        let camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Dynamic Uniform Buffer"),
            size: (aligned_size as usize * MAX_ENTITIES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Uniform Buffer"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layouts
        let cl_layout = camera_light_bind_group_layout(&gpu.device);
        let tex_layout = GpuTexture::bind_group_layout(&gpu.device);

        // Bind group
        let camera_light_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera+Light Bind Group"),
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

        let texture = GpuTexture::white_1x1(&gpu.device, &gpu.queue, &tex_layout);

        let render_pipeline = pipeline::create_mesh_pipeline(
            &gpu.device,
            gpu.surface_format,
            &cl_layout,
            &tex_layout,
        );

        // ECS: spawn 100 entities in a 10x10 grid, spacing 2.0
        let mut world = World::new();
        let spacing = 2.0_f32;
        let offset = (10.0 - 1.0) * spacing * 0.5;
        for row in 0..10 {
            for col in 0..10 {
                let x = col as f32 * spacing - offset;
                let z = row as f32 * spacing - offset;
                let entity = world.spawn();
                world.add(entity, Transform::from_position(Vec3::new(x, 0.0, z)));
                world.add(entity, MeshRef(mesh_handle));
            }
        }

        self.state = Some(AppState {
            window,
            gpu,
            pipeline: render_pipeline,
            assets,
            mesh_handle,
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
            world,
            time: 0.0,
            uniform_alignment: aligned_size,
            r_was_pressed: false,
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

                // R key: remove 10 random entities (on press, not hold)
                let r_pressed = state.input.is_key_pressed(KeyCode::KeyR);
                if r_pressed && !state.r_was_pressed {
                    let entities: Vec<Entity> = state.world.query2::<Transform, MeshRef>()
                        .iter()
                        .map(|(e, _, _)| *e)
                        .collect();
                    let remove_count = entities.len().min(10);
                    for &entity in entities.iter().take(remove_count) {
                        state.world.despawn(entity);
                    }
                    log::info!(
                        "Removed {} entities. Remaining: {}, Mesh assets: {}",
                        remove_count,
                        state.world.entity_count(),
                        state.assets.count::<Mesh>(),
                    );
                }
                state.r_was_pressed = r_pressed;

                state.input.begin_frame();
                state.time += dt;

                // Propagate transforms to compute WorldTransform
                propagate_transforms(&mut state.world);

                // Update window title with entity and asset counts
                let entity_count = state.world.query2::<WorldTransform, MeshRef>()
                    .len();
                let mesh_count = state.assets.count::<Mesh>();
                state.window.handle.set_title(&format!(
                    "Voltex - Asset Demo | Entities: {}, Mesh assets: {}",
                    entity_count, mesh_count,
                ));

                // Pre-compute all entity uniforms and write to dynamic buffer
                let view_proj = state.camera.view_projection();
                let cam_pos = [
                    state.camera.position.x,
                    state.camera.position.y,
                    state.camera.position.z,
                ];

                let entities = state.world.query2::<WorldTransform, MeshRef>();
                let aligned = state.uniform_alignment as usize;

                // Build staging data: one CameraUniform per entity, padded to alignment
                let total_bytes = entities.len() * aligned;
                let mut staging = vec![0u8; total_bytes];

                for (i, (_, world_transform, _mesh_ref)) in entities.iter().enumerate() {
                    let mut uniform = state.camera_uniform;
                    uniform.view_proj = view_proj.cols;
                    uniform.camera_pos = cam_pos;
                    uniform.model = world_transform.0.cols;

                    let bytes = bytemuck::bytes_of(&uniform);
                    let offset = i * aligned;
                    staging[offset..offset + bytes.len()].copy_from_slice(bytes);
                }

                state.gpu.queue.write_buffer(&state.camera_buffer, 0, &staging);

                // Write light uniform
                state.gpu.queue.write_buffer(
                    &state.light_buffer,
                    0,
                    bytemuck::cast_slice(&[state.light_uniform]),
                );

                // Get mesh from asset system
                let mesh = match state.assets.get(state.mesh_handle) {
                    Some(m) => m,
                    None => return,
                };

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

                    render_pass.set_pipeline(&state.pipeline);
                    render_pass.set_bind_group(1, &state._texture.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );

                    // Draw each entity with its dynamic offset
                    for (i, _) in entities.iter().enumerate() {
                        let dynamic_offset = (i as u32) * state.uniform_alignment;
                        render_pass.set_bind_group(
                            0,
                            &state.camera_light_bind_group,
                            &[dynamic_offset],
                        );
                        render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
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
    let mut app = AssetDemoApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
