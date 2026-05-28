mod arena;
mod camera;
mod components;
mod game;
mod systems;
mod wave;

use winit::{
    application::ApplicationHandler,
    event::{MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex::prelude::*;
use voltex::ecs::{MaterialHandle, MeshHandle, Transform};
use voltex::renderer::{
    create_pbr_full_texture_bind_group, create_pbr_pipeline, create_shadow_pipeline,
    generate_sphere, pbr_full_texture_bind_group_layout, shadow_pass_bind_group_layout,
    CameraUniform, GpuContext, GpuTexture, IblResources, LightData, LightsUniform,
    MaterialUniform, Mesh, ShadowMap, ShadowPassUniform, ShadowUniform, SHADOW_MAP_SIZE,
};
use voltex::editor::{UiContext, UiRenderer};
use voltex::audio::{AudioClip, AudioSystem};
use voltex_renderer_ecs::{collect_render_items, RenderAssets};
use wgpu::util::DeviceExt;

use arena::{arena_model_matrices, generate_arena};
use camera::QuarterViewCamera;
use game::{Game, RenderHandles};

const MAX_ENTITIES: usize = 100; // pre-allocate for player + projectiles + enemies + arena

fn generate_sine(freq: f32, duration: f32, sample_rate: u32) -> AudioClip {
    let samples = (sample_rate as f32 * duration) as usize;
    let mut data = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        data.push((t * freq * std::f32::consts::TAU).sin() * 0.3);
    }
    AudioClip::new(data, sample_rate, 1)
}

struct SurvivorApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    pbr_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    // ECS-driven rendering. RenderAssets owns the GPU meshes/materials;
    // Game owns the ECS World and the per-type render handles.
    render_assets: RenderAssets,
    camera: QuarterViewCamera,
    game: Game,
    // Color pass resources
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
    // Shadow resources
    shadow_map: ShadowMap,
    shadow_uniform_buffer: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
    shadow_pass_buffer: wgpu::Buffer,
    shadow_pass_bind_group: wgpu::BindGroup,
    _ibl: IblResources,
    // UI
    ui: UiContext,
    ui_renderer: UiRenderer,
    // Audio
    audio: Option<AudioSystem>,
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
    size.div_ceil(alignment) * alignment
}

/// Extract translation (last column) from a column-major Mat4.
fn extract_translation(m: &Mat4) -> Vec3 {
    Vec3::new(m.cols[3][0], m.cols[3][1], m.cols[3][2])
}

impl ApplicationHandler for SurvivorApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex Survivor".to_string(),
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
        let shadow_pass_aligned_size =
            align_up(std::mem::size_of::<ShadowPassUniform>() as u32, alignment);

        // ── Register all meshes + materials in RenderAssets ──
        let mut render_assets = RenderAssets::new();

        // Arena entities (5: floor + 4 obstacles)
        let arena_entities = generate_arena();
        let arena_models = arena_model_matrices();
        let mut arena_handles: Vec<(MeshHandle, MaterialHandle, Vec3)> = Vec::new();
        for (i, entity) in arena_entities.iter().enumerate() {
            let mesh = Mesh::new(&gpu.device, &entity.vertices, &entity.indices);
            let mh = render_assets.add_mesh(mesh);
            let mat = MaterialUniform::with_params(entity.base_color, entity.metallic, entity.roughness);
            let mat_h = render_assets.add_material(mat);
            let position = extract_translation(&arena_models[i]);
            arena_handles.push((mh, mat_h, position));
        }

        // Shared sphere mesh for player/projectile/enemy
        let (sphere_verts, sphere_indices) = generate_sphere(0.4, 16, 16);
        let sphere_mesh_h = render_assets.add_mesh(Mesh::new(&gpu.device, &sphere_verts, &sphere_indices));
        let player_mat_h = render_assets
            .add_material(MaterialUniform::with_params([0.2, 0.4, 1.0, 1.0], 0.3, 0.5));
        let projectile_mat_h = render_assets
            .add_material(MaterialUniform::with_params([1.0, 0.9, 0.1, 1.0], 0.1, 0.4));
        let enemy_mat_h = render_assets
            .add_material(MaterialUniform::with_params([1.0, 0.15, 0.1, 1.0], 0.2, 0.6));

        // Game owns the ECS World + render handles
        let handles = RenderHandles {
            player_mesh: sphere_mesh_h,
            player_material: player_mat_h,
            enemy_mesh: sphere_mesh_h,
            enemy_material: enemy_mat_h,
            projectile_mesh: sphere_mesh_h,
            projectile_material: projectile_mat_h,
        };
        let mut game = Game::new(handles);

        // Spawn arena entities into the same World (static — never despawned).
        for (mh, mat_h, position) in &arena_handles {
            let e = game.world.spawn();
            game.world.add(e, Transform::from_position(*position));
            game.world.add(e, MeshHandle(mh.0));
            game.world.add(e, MaterialHandle(mat_h.0));
        }

        // Quarter-view camera
        let camera = QuarterViewCamera::new();

        // Light: directional from upper-left
        let mut lights_uniform = LightsUniform::new();
        lights_uniform.ambient_color = [0.08, 0.08, 0.08];
        lights_uniform.add_light(LightData::directional(
            [-1.0, -2.0, -1.0],
            [1.0, 0.98, 0.95],
            2.0,
        ));

        // ---- Color pass buffers ----
        let camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Dynamic UBO"),
            size: (cam_aligned_size as usize * MAX_ENTITIES) as u64,
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
            size: (mat_aligned_size as usize * MAX_ENTITIES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layouts
        let cl_layout = camera_light_bind_group_layout(&gpu.device);
        let pbr_tex_layout = pbr_full_texture_bind_group_layout(&gpu.device);
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

        // PBR texture bind group (full set: albedo + normal + ORM + emissive).
        // 1x1 defaults route to the MaterialUniform values: white ORM keeps
        // material.roughness/metallic/ao unchanged; black emissive = no glow.
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
            label: Some("Material BG"),
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

        // ---- Shadow resources ----
        let shadow_map = ShadowMap::new(&gpu.device);
        let shadow_layout = ShadowMap::bind_group_layout(&gpu.device);

        let shadow_uniform = ShadowUniform {
            light_view_proj: Mat4::IDENTITY.cols,
            shadow_map_size: SHADOW_MAP_SIZE as f32,
            shadow_bias: 0.005,
            _padding: [0.0; 2],
            sun_direction: [0.5, -0.7, 0.5],
            turbidity: 3.0,
            sh_coefficients: [[0.0; 4]; 7],
        };
        let shadow_uniform_buffer =
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Shadow Uniform Buffer"),
                    contents: bytemuck::cast_slice(&[shadow_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
        let shadow_bind_group = shadow_map.create_bind_group(
            &gpu.device,
            &shadow_layout,
            &shadow_uniform_buffer,
            &ibl.brdf_lut_view,
            &ibl.brdf_lut_sampler,
        );

        // Shadow pass dynamic UBO
        let sp_layout = shadow_pass_bind_group_layout(&gpu.device);
        let shadow_pass_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Pass Dynamic UBO"),
            size: (shadow_pass_aligned_size as usize * MAX_ENTITIES) as u64,
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
                    size: wgpu::BufferSize::new(
                        std::mem::size_of::<ShadowPassUniform>() as u64,
                    ),
                }),
            }],
        });

        // ---- Pipelines ----
        let shadow_pipeline = create_shadow_pipeline(&gpu.device, &sp_layout);
        let pbr_pipeline = create_pbr_pipeline(
            &gpu.device,
            gpu.surface_format,
            &cl_layout,
            &pbr_tex_layout,
            &mat_layout,
            &shadow_layout,
        );

        // ---- UI ----
        let screen_w = gpu.config.width as f32;
        let screen_h = gpu.config.height as f32;
        let ui = UiContext::new(screen_w, screen_h);
        let ui_renderer = UiRenderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.surface_format,
            &ui.font,
        );

        // ---- Audio ----
        let shoot_clip = generate_sine(440.0, 0.05, 44100);
        let kill_clip = generate_sine(220.0, 0.1, 44100);
        let audio = AudioSystem::new(vec![shoot_clip, kill_clip]).ok();

        self.state = Some(AppState {
            window,
            gpu,
            pbr_pipeline,
            shadow_pipeline,
            render_assets,
            camera,
            game,
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
            shadow_map,
            shadow_uniform_buffer,
            shadow_bind_group,
            shadow_pass_buffer,
            shadow_pass_bind_group,
            _ibl: ibl,
            ui,
            ui_renderer,
            audio,
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
                state.input.begin_frame();

                // Restart on R when game over
                if state.game.game_over && state.input.is_key_just_pressed(KeyCode::KeyR) {
                    state.game.restart();
                }

                // Update game logic
                let events = state.game.update(&state.input, dt);

                // Mouse aim + shooting
                let aspect = state.gpu.config.width as f32 / state.gpu.config.height as f32;
                let (mx, my) = state.input.mouse_position();
                let screen_w = state.gpu.config.width as f32;
                let screen_h = state.gpu.config.height as f32;
                let mut shot_fired = false;
                if let Some(world_pos) =
                    mouse_to_world_xz(mx as f32, my as f32, screen_w, screen_h, &state.camera, aspect)
                {
                    let aim = world_pos - state.game.player_pos();
                    let aim_xz = Vec3::new(aim.x, 0.0, aim.z);
                    if aim_xz.length_squared() > 0.01 {
                        let aim_dir = aim_xz.normalize();
                        if state.input.is_mouse_button_pressed(MouseButton::Left) {
                            shot_fired = state.game.try_shoot(aim_dir);
                        }
                    }
                }

                // Play audio SFX
                if let Some(ref audio) = state.audio {
                    if shot_fired {
                        audio.play(0, 0.3, false);
                    }
                    if events.enemies_killed > 0 {
                        audio.play(1, 0.5, false);
                    }
                }

                // Camera follows player
                state.camera.target = state.game.player_pos();

                // HUD data captured before the borrow of state.game.world below.
                let (player_hp, player_max_hp) = state.game.player_health();
                let num_enemies = state.game.alive_enemy_count();

                // ----- Compute light VP for shadows -----
                let light_dir = Vec3::new(-1.0, -2.0, -1.0).normalize();
                let light_pos = Vec3::ZERO - light_dir * 25.0;
                let light_view = Mat4::look_at(light_pos, Vec3::ZERO, Vec3::Y);
                let light_proj = Mat4::orthographic(-15.0, 15.0, -15.0, 15.0, 0.1, 60.0);
                let light_vp = light_proj * light_view;

                let view_proj = state.camera.view_projection(aspect);
                let eye = state.camera.eye_position();
                let cam_pos = [eye.x, eye.y, eye.z];

                // ----- Collect RenderItems from ECS World (single source of truth) -----
                let items = collect_render_items(&mut state.game.world, &state.render_assets);
                let num_entities = items.len().min(MAX_ENTITIES);

                // ----- Build staging data for all three dynamic UBOs -----
                let cam_aligned = state.cam_aligned_size as usize;
                let mat_aligned = state.mat_aligned_size as usize;
                let sp_aligned = state.shadow_pass_aligned_size as usize;

                let mut cam_staging = vec![0u8; cam_aligned * num_entities];
                let mut mat_staging = vec![0u8; mat_aligned * num_entities];
                let mut sp_staging = vec![0u8; sp_aligned * num_entities];

                for (i, item) in items.iter().take(num_entities).enumerate() {
                    let cam = CameraUniform {
                        view_proj: view_proj.cols,
                        model: item.model.cols,
                        camera_pos: cam_pos,
                        _padding: 0.0,
                    };
                    let bytes = bytemuck::bytes_of(&cam);
                    cam_staging[i * cam_aligned..i * cam_aligned + bytes.len()]
                        .copy_from_slice(bytes);

                    let mat_bytes = bytemuck::bytes_of(&item.material);
                    mat_staging[i * mat_aligned..i * mat_aligned + mat_bytes.len()]
                        .copy_from_slice(mat_bytes);

                    let sp = ShadowPassUniform {
                        light_vp_model: (light_vp * item.model).cols,
                    };
                    let sp_bytes = bytemuck::bytes_of(&sp);
                    sp_staging[i * sp_aligned..i * sp_aligned + sp_bytes.len()]
                        .copy_from_slice(sp_bytes);
                }

                if num_entities > 0 {
                    state.gpu.queue.write_buffer(&state.camera_buffer, 0, &cam_staging);
                    state.gpu.queue.write_buffer(&state.material_buffer, 0, &mat_staging);
                    state.gpu.queue.write_buffer(&state.shadow_pass_buffer, 0, &sp_staging);
                }

                // Update shadow uniform with light VP
                let shadow_uniform = ShadowUniform {
                    light_view_proj: light_vp.cols,
                    shadow_map_size: SHADOW_MAP_SIZE as f32,
                    shadow_bias: 0.005,
                    _padding: [0.0; 2],
                    sun_direction: [0.5, -0.7, 0.5],
                    turbidity: 3.0,
                    sh_coefficients: [[0.0; 4]; 7],
                };
                state.gpu.queue.write_buffer(
                    &state.shadow_uniform_buffer,
                    0,
                    bytemuck::cast_slice(&[shadow_uniform]),
                );

                // Refresh light UBO each frame.
                let mut lights_uniform = LightsUniform::new();
                lights_uniform.ambient_color = [0.08, 0.08, 0.08];
                lights_uniform.add_light(LightData::directional(
                    [-1.0, -2.0, -1.0],
                    [1.0, 0.98, 0.95],
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

                let color_view =
                    output
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("Survivor Encoder"),
                    },
                );

                // ===== Pass 1: Shadow =====
                {
                    let mut shadow_pass =
                        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Shadow Pass"),
                            color_attachments: &[],
                            depth_stencil_attachment: Some(
                                wgpu::RenderPassDepthStencilAttachment {
                                    view: &state.shadow_map.view,
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

                    shadow_pass.set_pipeline(&state.shadow_pipeline);

                    for (i, item) in items.iter().take(num_entities).enumerate() {
                        let mesh = state
                            .render_assets
                            .mesh(item.mesh)
                            .expect("collect_render_items already skipped stale handles");
                        let offset = (i as u32) * state.shadow_pass_aligned_size;
                        shadow_pass.set_bind_group(0, &state.shadow_pass_bind_group, &[offset]);
                        shadow_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                        shadow_pass.set_index_buffer(
                            mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        shadow_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                    }
                }

                // ===== Pass 2: Color (PBR) =====
                {
                    let mut render_pass =
                        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Color Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &color_view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.15,
                                        g: 0.15,
                                        b: 0.2,
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

                    render_pass.set_pipeline(&state.pbr_pipeline);
                    render_pass.set_bind_group(1, &state.pbr_texture_bind_group, &[]);
                    render_pass.set_bind_group(3, &state.shadow_bind_group, &[]);

                    for (i, item) in items.iter().take(num_entities).enumerate() {
                        let mesh = state
                            .render_assets
                            .mesh(item.mesh)
                            .expect("collect_render_items already skipped stale handles");
                        let cam_offset = (i as u32) * state.cam_aligned_size;
                        let mat_offset = (i as u32) * state.mat_aligned_size;
                        render_pass.set_bind_group(0, &state.camera_light_bind_group, &[cam_offset]);
                        render_pass.set_bind_group(2, &state.material_bind_group, &[mat_offset]);
                        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(
                            mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                    }
                }

                // ===== Pass 3: UI Overlay =====
                {
                    let mouse_down = state.input.is_mouse_button_pressed(MouseButton::Left);
                    state.ui.begin_frame(mx as f32, my as f32, mouse_down);

                    // Top-left: game info
                    state.ui.text(&format!("HP: {}/{}", player_hp, player_max_hp));
                    state.ui.text(&format!("Wave: {}", state.game.wave.wave_number));
                    state.ui.text(&format!("Score: {}", state.game.score));
                    state.ui.text(&format!("Enemies: {}", num_enemies));

                    if state.game.game_over {
                        state.ui.begin_panel(
                            "Game Over",
                            screen_w / 2.0 - 120.0,
                            screen_h / 2.0 - 60.0,
                            240.0,
                            120.0,
                        );
                        state.ui.text("GAME OVER");
                        state.ui.text(&format!("Final Score: {}", state.game.score));
                        state.ui.text("Press R to restart");
                        state.ui.end_panel();
                    }

                    state.ui.end_frame();

                    state.ui_renderer.render(
                        &state.gpu.device,
                        &state.gpu.queue,
                        &mut encoder,
                        &color_view,
                        &state.ui.draw_list,
                        screen_w,
                        screen_h,
                    );
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

fn mouse_to_world_xz(
    mouse_x: f32,
    mouse_y: f32,
    screen_w: f32,
    screen_h: f32,
    camera: &QuarterViewCamera,
    aspect: f32,
) -> Option<Vec3> {
    // Convert mouse to NDC (-1..1)
    let ndc_x = (mouse_x / screen_w) * 2.0 - 1.0;
    let ndc_y = 1.0 - (mouse_y / screen_h) * 2.0; // flip Y

    let cam_pos = camera.eye_position();

    // Construct ray direction from camera through screen point
    let fov = 45.0_f32.to_radians();
    let half_h = (fov / 2.0).tan();
    let half_w = half_h * aspect;

    // Camera basis vectors
    let forward = camera.target - cam_pos;
    if forward.length_squared() < 1e-12 {
        return None;
    }
    let forward = forward.normalize();
    let right_vec = forward.cross(Vec3::Y);
    if right_vec.length_squared() < 1e-12 {
        return None;
    }
    let right_vec = right_vec.normalize();
    let up = right_vec.cross(forward);

    let ray_dir_unnorm = forward + right_vec * (ndc_x * half_w) + up * (ndc_y * half_h);
    if ray_dir_unnorm.length_squared() < 1e-12 {
        return None;
    }
    let ray_dir = ray_dir_unnorm.normalize();

    // Intersect with Y=0 plane
    if ray_dir.y.abs() < 1e-6 {
        return None;
    }
    let t = -cam_pos.y / ray_dir.y;
    if t < 0.0 {
        return None;
    }

    Some(Vec3::new(
        cam_pos.x + ray_dir.x * t,
        0.0,
        cam_pos.z + ray_dir.z * t,
    ))
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = SurvivorApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
