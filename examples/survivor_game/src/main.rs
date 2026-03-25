mod arena;
mod camera;

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
    GpuContext, CameraUniform, LightsUniform, LightData,
    Mesh, GpuTexture, MaterialUniform, create_pbr_pipeline,
    ShadowMap, ShadowUniform, ShadowPassUniform, SHADOW_MAP_SIZE,
    create_shadow_pipeline, shadow_pass_bind_group_layout,
    IblResources, pbr_texture_bind_group_layout, create_pbr_texture_bind_group,
};
use wgpu::util::DeviceExt;

use arena::{generate_arena, arena_model_matrices};
use camera::QuarterViewCamera;

const NUM_ENTITIES: usize = 5; // 1 floor + 4 obstacles

struct SurvivorApp {
    state: Option<AppState>,
}

struct RenderEntity {
    mesh: Mesh,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    pbr_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    entities: Vec<RenderEntity>,
    camera: QuarterViewCamera,
    // Color pass resources
    camera_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    material_buffer: wgpu::Buffer,
    camera_light_bind_group: wgpu::BindGroup,
    _albedo_tex: GpuTexture,
    _normal_tex: (wgpu::Texture, wgpu::TextureView, wgpu::Sampler),
    pbr_texture_bind_group: wgpu::BindGroup,
    material_bind_group: wgpu::BindGroup,
    // Shadow resources
    shadow_map: ShadowMap,
    shadow_uniform_buffer: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
    shadow_pass_buffer: wgpu::Buffer,
    shadow_pass_bind_group: wgpu::BindGroup,
    _ibl: IblResources,
    // Misc
    input: InputState,
    timer: GameTimer,
    cam_aligned_size: u32,
    mat_aligned_size: u32,
    shadow_pass_aligned_size: u32,
    // Arena data
    models: Vec<Mat4>,
    materials: Vec<([f32; 4], f32, f32)>,
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

impl ApplicationHandler for SurvivorApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex Survivor".to_string(),
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
        let shadow_pass_aligned_size =
            align_up(std::mem::size_of::<ShadowPassUniform>() as u32, alignment);

        // Generate arena geometry
        let arena_entities = generate_arena();
        let models = arena_model_matrices();

        // Collect materials and create GPU meshes
        let mut render_entities = Vec::new();
        let mut materials = Vec::new();

        for entity in &arena_entities {
            let mesh = Mesh::new(&gpu.device, &entity.vertices, &entity.indices);
            render_entities.push(RenderEntity { mesh });
            materials.push((entity.base_color, entity.metallic, entity.roughness));
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
            size: (cam_aligned_size as usize * NUM_ENTITIES) as u64,
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
            size: (mat_aligned_size as usize * NUM_ENTITIES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layouts
        let cl_layout = camera_light_bind_group_layout(&gpu.device);
        let pbr_tex_layout = pbr_texture_bind_group_layout(&gpu.device);
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

        // PBR texture bind group (white albedo + flat normal)
        let old_tex_layout = GpuTexture::bind_group_layout(&gpu.device);
        let albedo_tex = GpuTexture::white_1x1(&gpu.device, &gpu.queue, &old_tex_layout);
        let normal_tex = GpuTexture::flat_normal_1x1(&gpu.device, &gpu.queue);
        let pbr_texture_bind_group = create_pbr_texture_bind_group(
            &gpu.device,
            &pbr_tex_layout,
            &albedo_tex.view,
            &albedo_tex.sampler,
            &normal_tex.1,
            &normal_tex.2,
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
            size: (shadow_pass_aligned_size as usize * NUM_ENTITIES) as u64,
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

        self.state = Some(AppState {
            window,
            gpu,
            pbr_pipeline,
            shadow_pipeline,
            entities: render_entities,
            camera,
            camera_buffer,
            light_buffer,
            material_buffer,
            camera_light_bind_group,
            _albedo_tex: albedo_tex,
            _normal_tex: normal_tex,
            pbr_texture_bind_group,
            material_bind_group,
            shadow_map,
            shadow_uniform_buffer,
            shadow_bind_group,
            shadow_pass_buffer,
            shadow_pass_bind_group,
            _ibl: ibl,
            input: InputState::new(),
            timer: GameTimer::new(60),
            cam_aligned_size,
            mat_aligned_size,
            shadow_pass_aligned_size,
            models,
            materials,
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
                state.input.begin_frame();

                let aspect = state.gpu.config.width as f32 / state.gpu.config.height as f32;

                // ----- Compute light VP for shadows -----
                let light_dir = Vec3::new(-1.0, -2.0, -1.0).normalize();
                let light_pos = Vec3::ZERO - light_dir * 25.0;
                let light_view = Mat4::look_at(light_pos, Vec3::ZERO, Vec3::Y);
                let light_proj = Mat4::orthographic(-15.0, 15.0, -15.0, 15.0, 0.1, 60.0);
                let light_vp = light_proj * light_view;

                let cam_aligned = state.cam_aligned_size as usize;
                let mat_aligned = state.mat_aligned_size as usize;
                let sp_aligned = state.shadow_pass_aligned_size as usize;

                // ----- Build shadow pass staging data -----
                let sp_total = sp_aligned * NUM_ENTITIES;
                let mut sp_staging = vec![0u8; sp_total];
                for i in 0..NUM_ENTITIES {
                    let sp_uniform = ShadowPassUniform {
                        light_vp_model: (light_vp * state.models[i]).cols,
                    };
                    let bytes = bytemuck::bytes_of(&sp_uniform);
                    let offset = i * sp_aligned;
                    sp_staging[offset..offset + bytes.len()].copy_from_slice(bytes);
                }
                state
                    .gpu
                    .queue
                    .write_buffer(&state.shadow_pass_buffer, 0, &sp_staging);

                // ----- Build color pass staging data -----
                let view_proj = state.camera.view_projection(aspect);
                let eye = state.camera.eye_position();
                let cam_pos = [eye.x, eye.y, eye.z];

                let cam_total = cam_aligned * NUM_ENTITIES;
                let mat_total = mat_aligned * NUM_ENTITIES;
                let mut cam_staging = vec![0u8; cam_total];
                let mut mat_staging = vec![0u8; mat_total];

                for i in 0..NUM_ENTITIES {
                    let cam_uniform = CameraUniform {
                        view_proj: view_proj.cols,
                        model: state.models[i].cols,
                        camera_pos: cam_pos,
                        _padding: 0.0,
                    };
                    let bytes = bytemuck::bytes_of(&cam_uniform);
                    let offset = i * cam_aligned;
                    cam_staging[offset..offset + bytes.len()].copy_from_slice(bytes);

                    let (color, metallic, roughness) = state.materials[i];
                    let mat_uniform =
                        MaterialUniform::with_params(color, metallic, roughness);
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

                    for (i, entity) in state.entities.iter().enumerate() {
                        let offset = (i as u32) * state.shadow_pass_aligned_size;
                        shadow_pass
                            .set_bind_group(0, &state.shadow_pass_bind_group, &[offset]);
                        shadow_pass
                            .set_vertex_buffer(0, entity.mesh.vertex_buffer.slice(..));
                        shadow_pass.set_index_buffer(
                            entity.mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        shadow_pass.draw_indexed(0..entity.mesh.num_indices, 0, 0..1);
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
                    render_pass
                        .set_bind_group(1, &state.pbr_texture_bind_group, &[]);
                    render_pass.set_bind_group(3, &state.shadow_bind_group, &[]);

                    for (i, entity) in state.entities.iter().enumerate() {
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
                        render_pass
                            .set_vertex_buffer(0, entity.mesh.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(
                            entity.mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        render_pass.draw_indexed(0..entity.mesh.num_indices, 0, 0..1);
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
    let mut app = SurvivorApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
