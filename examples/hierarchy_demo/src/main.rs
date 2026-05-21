use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex::prelude::*;
use voltex::renderer::{
    GpuContext, Camera, FpsController, CameraUniform, LightUniform, Mesh, GpuTexture, pipeline, obj,
};
use voltex::ecs::{Tag, add_child, propagate_transforms, WorldTransform};
use wgpu::util::DeviceExt;

const MAX_ENTITIES: usize = 64;

/// Stores entity handle + orbit speed for animation.
struct OrbitalBody {
    entity: Entity,
    speed: f32,
}

struct HierarchyDemoApp {
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
    world: World,
    bodies: Vec<OrbitalBody>,
    time: f32,
    uniform_alignment: u32,
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

impl ApplicationHandler for HierarchyDemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Hierarchy Demo (Solar System)".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new(window.handle.clone());

        // Dynamic uniform buffer alignment
        let uniform_alignment = gpu.device.limits().min_uniform_buffer_offset_alignment;
        let uniform_size = std::mem::size_of::<CameraUniform>() as u32;
        let aligned_size =
            uniform_size.div_ceil(uniform_alignment) * uniform_alignment;

        // Parse OBJ
        let obj_src = include_str!("../../../assets/cube.obj");
        let obj_data = obj::parse_obj(obj_src);
        let mesh = Mesh::new(&gpu.device, &obj_data.vertices, &obj_data.indices);

        // Camera
        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
        let mut camera = Camera::new(Vec3::new(0.0, 10.0, 20.0), aspect);
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

        // Bind group: camera binding uses dynamic offset, size = one CameraUniform
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

        let texture = GpuTexture::white_1x1(&gpu.device, &gpu.queue, &tex_layout);

        let render_pipeline = pipeline::create_mesh_pipeline(
            &gpu.device,
            gpu.surface_format,
            &cl_layout,
            &tex_layout,
        );

        // ---- ECS: Build solar system hierarchy ----
        let mut world = World::new();
        let mut bodies: Vec<OrbitalBody> = Vec::new();

        // Sun: position(0,0,0), scale(1.5,1.5,1.5)
        let sun = world.spawn();
        world.add(
            sun,
            Transform::from_position_scale(Vec3::ZERO, Vec3::new(1.5, 1.5, 1.5)),
        );
        world.add(sun, Tag("sun".to_string()));
        bodies.push(OrbitalBody { entity: sun, speed: 0.2 });

        // Planet1: position(5,0,0), scale(0.5,0.5,0.5)
        let planet1 = world.spawn();
        world.add(
            planet1,
            Transform::from_position_scale(Vec3::new(5.0, 0.0, 0.0), Vec3::new(0.5, 0.5, 0.5)),
        );
        world.add(planet1, Tag("planet1".to_string()));
        add_child(&mut world, sun, planet1);
        bodies.push(OrbitalBody { entity: planet1, speed: 0.8 });

        // Moon1: position(1.5,0,0), scale(0.3,0.3,0.3) — child of Planet1
        let moon1 = world.spawn();
        world.add(
            moon1,
            Transform::from_position_scale(
                Vec3::new(1.5, 0.0, 0.0),
                Vec3::new(0.3, 0.3, 0.3),
            ),
        );
        world.add(moon1, Tag("moon1".to_string()));
        add_child(&mut world, planet1, moon1);
        bodies.push(OrbitalBody { entity: moon1, speed: 2.0 });

        // Planet2: position(9,0,0), scale(0.7,0.7,0.7)
        let planet2 = world.spawn();
        world.add(
            planet2,
            Transform::from_position_scale(Vec3::new(9.0, 0.0, 0.0), Vec3::new(0.7, 0.7, 0.7)),
        );
        world.add(planet2, Tag("planet2".to_string()));
        add_child(&mut world, sun, planet2);
        bodies.push(OrbitalBody { entity: planet2, speed: 0.5 });

        // Planet3: position(13,0,0), scale(0.4,0.4,0.4)
        let planet3 = world.spawn();
        world.add(
            planet3,
            Transform::from_position_scale(
                Vec3::new(13.0, 0.0, 0.0),
                Vec3::new(0.4, 0.4, 0.4),
            ),
        );
        world.add(planet3, Tag("planet3".to_string()));
        add_child(&mut world, sun, planet3);
        bodies.push(OrbitalBody { entity: planet3, speed: 0.3 });

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
            world,
            bodies,
            time: 0.0,
            uniform_alignment: aligned_size,
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

                if pressed {
                    match key_code {
                        KeyCode::Escape => event_loop.exit(),
                        KeyCode::KeyP => {
                            // Print serialized scene to stdout
                            let scene_str =
                                voltex::ecs::serialize_scene(&state.world);
                            println!("--- Scene Snapshot ---\n{}", scene_str);
                        }
                        _ => {}
                    }
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

                // ---- Input / Camera ----
                if state
                    .input
                    .is_mouse_button_pressed(winit::event::MouseButton::Right)
                {
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
                state
                    .fps_controller
                    .process_movement(&mut state.camera, forward, right, up, dt);
                state.input.begin_frame();

                state.time += dt;

                // ---- Animate: rotate each body around Y ----
                for body in &state.bodies {
                    if let Some(t) = state.world.get_mut::<Transform>(body.entity) {
                        t.rotation.y += dt * body.speed;
                    }
                }

                // ---- Propagate hierarchy transforms ----
                propagate_transforms(&mut state.world);

                // ---- Build per-entity uniforms using WorldTransform ----
                let view_proj = state.camera.view_projection();
                let cam_pos = [
                    state.camera.position.x,
                    state.camera.position.y,
                    state.camera.position.z,
                ];

                let world_transforms: Vec<(Entity, Mat4)> = state
                    .world
                    .query::<WorldTransform>()
                    .map(|(e, wt)| (e, wt.0))
                    .collect();

                let aligned = state.uniform_alignment as usize;
                let total_bytes = world_transforms.len() * aligned;
                let mut staging = vec![0u8; total_bytes];

                for (i, (_entity, world_mat)) in world_transforms.iter().enumerate() {
                    let mut uniform = state.camera_uniform;
                    uniform.view_proj = view_proj.cols;
                    uniform.camera_pos = cam_pos;
                    uniform.model = world_mat.cols;

                    let bytes = bytemuck::bytes_of(&uniform);
                    let offset = i * aligned;
                    staging[offset..offset + bytes.len()].copy_from_slice(bytes);
                }

                state
                    .gpu
                    .queue
                    .write_buffer(&state.camera_buffer, 0, &staging);

                // Write light uniform
                state.gpu.queue.write_buffer(
                    &state.light_buffer,
                    0,
                    bytemuck::cast_slice(&[state.light_uniform]),
                );

                // ---- Render ----
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
                        label: Some("Render Encoder"),
                    },
                );

                {
                    let mut render_pass =
                        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.02,
                                        g: 0.02,
                                        b: 0.05,
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
                    render_pass.set_bind_group(1, &state._texture.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, state.mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        state.mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );

                    // Draw each entity with its dynamic offset
                    for (i, _) in world_transforms.iter().enumerate() {
                        let dynamic_offset = (i as u32) * state.uniform_alignment;
                        render_pass.set_bind_group(
                            0,
                            &state.camera_light_bind_group,
                            &[dynamic_offset],
                        );
                        render_pass.draw_indexed(0..state.mesh.num_indices, 0, 0..1);
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
    let mut app = HierarchyDemoApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
