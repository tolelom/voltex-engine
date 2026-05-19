use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::{GpuContext, Mesh, MeshVertex, CameraUniform, LightUniform, GpuTexture};
use voltex_editor::{
    UiContext, UiRenderer, DockTree, DockNode, Axis, Rect,
    OrbitCamera, ViewportTexture, ViewportRenderer, VIEWPORT_COLOR_FORMAT,
    hierarchy_panel, inspector_panel,
    AssetBrowser, asset_browser_panel,
    EditorSession, EditorCommand,
};
use voltex_ecs::entity::Entity;
use voltex_ecs::transform::Transform;
use voltex_ecs::scene::Tag;
use voltex_math::{Mat4, Vec3};

struct EditorDemoApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    input: InputState,
    timer: GameTimer,
    ui: UiContext,
    ui_renderer: UiRenderer,
    dock: DockTree,
    // Widget state
    counter: u32,
    speed: f32,
    show_grid: bool,
    // ECS — wrapped in EditorSession for command-based mutation + auto-save
    session: EditorSession,
    selected_entity: Option<Entity>,
    tag_buffer: String,
    // Viewport state
    orbit_cam: OrbitCamera,
    viewport_tex: ViewportTexture,
    viewport_renderer: ViewportRenderer,
    // 3D scene
    scene_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    camera_light_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    texture_layout: wgpu::BindGroupLayout,
    dummy_texture_bg: wgpu::BindGroup,
    scene_meshes: Vec<Mesh>,
    // Asset browser
    asset_browser: AssetBrowser,
    // Mouse tracking
    prev_mouse: (f32, f32),
    left_dragging: bool,
    middle_dragging: bool,
    scroll_delta: f32,
}

fn camera_light_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera+Light BGL"),
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

fn texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Texture BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn create_scene_pipeline(
    device: &wgpu::Device,
    cl_layout: &wgpu::BindGroupLayout,
    tex_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Scene Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../../crates/voltex_renderer/src/mesh_shader.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Scene Pipeline Layout"),
        bind_group_layouts: &[cl_layout, tex_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Scene Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[MeshVertex::LAYOUT],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: VIEWPORT_COLOR_FORMAT,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
        multiview_mask: None,
        cache: None,
    })
}

fn make_cube(device: &wgpu::Device, cx: f32, cy: f32, cz: f32, size: f32) -> Mesh {
    let s = size * 0.5;
    let mut verts = Vec::new();
    let mut indices = Vec::new();

    let mut add_face = |positions: [[f32; 3]; 4], normal: [f32; 3]| {
        let base = verts.len() as u32;
        for pos in &positions {
            verts.push(MeshVertex {
                position: [pos[0] + cx, pos[1] + cy, pos[2] + cz],
                normal,
                uv: [0.0, 0.0],
                tangent: [1.0, 0.0, 0.0, 1.0],
            });
        }
        indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    };

    // +Y (top)
    add_face([[-s,s,-s], [s,s,-s], [s,s,s], [-s,s,s]], [0.0, 1.0, 0.0]);
    // -Y (bottom)
    add_face([[-s,-s,s], [s,-s,s], [s,-s,-s], [-s,-s,-s]], [0.0, -1.0, 0.0]);
    // +Z (front)
    add_face([[-s,-s,s], [-s,s,s], [s,s,s], [s,-s,s]], [0.0, 0.0, 1.0]);
    // -Z (back)
    add_face([[s,-s,-s], [s,s,-s], [-s,s,-s], [-s,-s,-s]], [0.0, 0.0, -1.0]);
    // +X (right)
    add_face([[s,-s,s], [s,s,s], [s,s,-s], [s,-s,-s]], [1.0, 0.0, 0.0]);
    // -X (left)
    add_face([[-s,-s,-s], [-s,s,-s], [-s,s,s], [-s,-s,s]], [-1.0, 0.0, 0.0]);

    Mesh::new(device, &verts, &indices)
}

fn make_ground(device: &wgpu::Device) -> Mesh {
    let s = 5.0;
    let verts = vec![
        MeshVertex { position: [-s, 0.0, -s], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [s, 0.0, -s], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [s, 0.0, s], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s, 0.0, s], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
    ];
    let indices = vec![0, 1, 2, 0, 2, 3];
    Mesh::new(device, &verts, &indices)
}

impl ApplicationHandler for EditorDemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Editor Demo".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new(window.handle.clone());

        let mut ui = UiContext::new(gpu.config.width as f32, gpu.config.height as f32);
        let ui_renderer = UiRenderer::new(&gpu.device, &gpu.queue, gpu.surface_format, &ui.font);

        // Try to load TTF font
        let ttf_data = std::fs::read("C:/Windows/Fonts/arial.ttf")
            .or_else(|_| std::fs::read("C:/Windows/Fonts/consola.ttf"))
            .or_else(|_| std::fs::read("C:/Windows/Fonts/malgun.ttf"))
            .ok();
        if let Some(data) = ttf_data {
            if let Ok(font) = voltex_editor::TtfFont::new(&data, 14.0) {
                ui.ttf_font = Some(font);
            }
        }

        let dock = DockTree::new(
            DockNode::split(
                Axis::Horizontal, 0.25,
                DockNode::leaf(vec![0]),
                DockNode::split(
                    Axis::Vertical, 0.7,
                    DockNode::leaf(vec![1]),
                    DockNode::Leaf { tabs: vec![2, 3], active: 0 },
                ),
            ),
            vec!["Hierarchy", "Viewport", "Inspector", "Assets"],
        );

        let asset_browser = AssetBrowser::new(std::env::current_dir().unwrap_or_default());

        // Viewport
        let orbit_cam = OrbitCamera::new();
        let viewport_tex = ViewportTexture::new(&gpu.device, 640, 480);
        let viewport_renderer = ViewportRenderer::new(&gpu.device, gpu.surface_format);

        // 3D scene setup
        let cl_layout = camera_light_bind_group_layout(&gpu.device);
        let tex_layout = texture_bind_group_layout(&gpu.device);
        let scene_pipeline = create_scene_pipeline(&gpu.device, &cl_layout, &tex_layout);

        let camera_uniform = CameraUniform::new();
        let light_uniform = LightUniform::new();

        use wgpu::util::DeviceExt;
        let camera_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let light_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Buffer"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Dummy white texture for mesh shader group(1)
        let dummy_texture = GpuTexture::white_1x1(&gpu.device, &gpu.queue, &tex_layout);

        // Scene meshes
        let ground = make_ground(&gpu.device);
        let cube1 = make_cube(&gpu.device, 0.0, 0.5, 0.0, 1.0);
        let cube2 = make_cube(&gpu.device, 2.0, 0.5, 1.0, 1.0);
        let cube3 = make_cube(&gpu.device, -1.5, 0.5, -1.0, 1.0);

        // ECS Session setup — World lives inside the EditorSession so Toolbar
        // actions can be logged and auto-saved. Direct seeding (below) bypasses
        // the command log, which is fine for one-time scene init.
        std::fs::create_dir_all("scenes").ok();
        let mut session = EditorSession::new("scenes/editor_demo.vscn");

        let ground_e = session.world.spawn();
        session.world.add(ground_e, Transform::from_position(Vec3::new(0.0, 0.0, 0.0)));
        session.world.add(ground_e, Tag("Ground".to_string()));

        let cube1_e = session.world.spawn();
        session.world.add(cube1_e, Transform::from_position(Vec3::new(0.0, 0.5, 0.0)));
        session.world.add(cube1_e, Tag("Cube1".to_string()));

        let cube2_e = session.world.spawn();
        session.world.add(cube2_e, Transform::from_position(Vec3::new(2.0, 0.5, 1.0)));
        session.world.add(cube2_e, Tag("Cube2".to_string()));

        let cube3_e = session.world.spawn();
        session.world.add(cube3_e, Transform::from_position(Vec3::new(-1.5, 0.5, -1.0)));
        session.world.add(cube3_e, Tag("Cube3".to_string()));

        // Cube2 is child of Cube1
        voltex_ecs::hierarchy::add_child(&mut session.world, cube1_e, cube2_e);

        self.state = Some(AppState {
            window,
            gpu,
            input: InputState::new(),
            timer: GameTimer::new(60),
            ui,
            ui_renderer,
            dock,
            counter: 0,
            speed: 5.0,
            show_grid: true,
            orbit_cam,
            viewport_tex,
            viewport_renderer,
            scene_pipeline,
            camera_buffer,
            light_buffer,
            camera_light_layout: cl_layout,
            texture_layout: tex_layout,
            dummy_texture_bg: dummy_texture.bind_group,
            asset_browser,
            scene_meshes: vec![ground, cube1, cube2, cube3],
            session,
            selected_entity: None,
            tag_buffer: String::new(),
            prev_mouse: (0.0, 0.0),
            left_dragging: false,
            middle_dragging: false,
            scroll_delta: 0.0,
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
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.input.process_mouse_move(position.x, position.y);
            }

            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                let pressed = btn_state == winit::event::ElementState::Pressed;
                state.input.process_mouse_button(button, pressed);
                match button {
                    winit::event::MouseButton::Left => state.left_dragging = pressed,
                    winit::event::MouseButton::Middle => state.middle_dragging = pressed,
                    _ => {}
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                state.input.process_scroll(y);
                state.scroll_delta = y;
            }

            WindowEvent::RedrawRequested => {
                state.timer.tick();
                state.input.begin_frame();

                let (mx, my) = state.input.mouse_position();
                let mx = mx as f32;
                let my = my as f32;
                let mouse_down = state.input.is_mouse_button_pressed(winit::event::MouseButton::Left);

                let dt = state.timer.frame_dt();
                let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };

                state.ui.begin_frame(mx, my, mouse_down);

                let screen_w = state.gpu.config.width as f32;
                let screen_h = state.gpu.config.height as f32;

                // ── Toolbar (Spawn/Save/Load via EditorCommand → session log + auto-save) ──
                const TOOLBAR_H: f32 = 32.0;
                state.ui.begin_panel("Toolbar", 0.0, 0.0, screen_w, TOOLBAR_H);
                if state.ui.button("Spawn") {
                    if let Ok(e) = state.session.dispatch(EditorCommand::Spawn) {
                        state.selected_entity = Some(e);
                    }
                }
                if state.ui.button("Save") {
                    let path = state.session.scene_path.clone();
                    let _ = state.session.dispatch(EditorCommand::SaveScene { path });
                }
                if state.ui.button("Load") {
                    let path = state.session.scene_path.clone();
                    let _ = state.session.dispatch(EditorCommand::LoadScene { path });
                    state.selected_entity = None;
                }
                state.ui.end_panel();

                let areas = state.dock.layout(Rect { x: 0.0, y: TOOLBAR_H, w: screen_w, h: screen_h - TOOLBAR_H });
                state.dock.update(mx, my, mouse_down);
                state.dock.draw_chrome(&mut state.ui);

                // Find viewport rect for later 3D rendering
                let mut viewport_rect: Option<Rect> = None;

                for (panel_id, rect) in &areas {
                    match panel_id {
                        0 => {
                            // Hierarchy panel
                            hierarchy_panel(&mut state.ui, &state.session.world, &mut state.selected_entity, rect);
                        }
                        1 => {
                            // Viewport panel (keep existing 3D rendering code exactly as-is)
                            viewport_rect = Some(*rect);
                        }
                        2 => {
                            // Inspector panel (8-4: slider-based; mutates world directly — slider edits aren't logged yet)
                            inspector_panel(&mut state.ui, &mut state.session.world, state.selected_entity, rect, &mut state.tag_buffer);
                        }
                        3 => {
                            asset_browser_panel(&mut state.ui, &mut state.asset_browser, rect);
                        }
                        _ => {}
                    }
                }

                state.ui.end_frame();

                // Orbit camera input (only when mouse is over viewport)
                if let Some(vr) = &viewport_rect {
                    if vr.contains(mx, my) {
                        let dx = mx - state.prev_mouse.0;
                        let dy = my - state.prev_mouse.1;
                        if state.left_dragging { state.orbit_cam.orbit(dx, dy); }
                        if state.middle_dragging { state.orbit_cam.pan(dx, dy); }
                        if state.scroll_delta.abs() > 0.0 {
                            state.orbit_cam.zoom(state.scroll_delta);
                        }
                    }
                }
                state.prev_mouse = (mx, my);
                state.scroll_delta = 0.0;

                // Acquire surface texture
                let output = match state.gpu.surface.get_current_texture() {
                    Ok(t) => t,
                    Err(wgpu::SurfaceError::Lost) => {
                        let (w, h) = state.window.inner_size();
                        state.gpu.resize(w, h);
                        return;
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => { event_loop.exit(); return; }
                    Err(_) => return,
                };

                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("Editor Encoder") },
                );

                // Render 3D scene to viewport texture
                if let Some(vr) = &viewport_rect {
                    state.viewport_tex.ensure_size(&state.gpu.device, vr.w.max(1.0) as u32, vr.h.max(1.0) as u32);

                    // Update camera uniform
                    let aspect = vr.w / vr.h.max(1.0);
                    let vp = state.orbit_cam.view_projection(aspect);
                    let cam_pos = state.orbit_cam.position();

                    let camera_uniform = CameraUniform {
                        view_proj: vp.cols,
                        model: Mat4::IDENTITY.cols,
                        camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z],
                        _padding: 0.0,
                    };
                    state.gpu.queue.write_buffer(&state.camera_buffer, 0, bytemuck::cast_slice(&[camera_uniform]));

                    let light_uniform = LightUniform {
                        direction: [0.3, -1.0, -0.5],
                        _padding1: 0.0,
                        color: [1.0, 0.95, 0.9],
                        ambient_strength: 0.15,
                    };
                    state.gpu.queue.write_buffer(&state.light_buffer, 0, bytemuck::cast_slice(&[light_uniform]));

                    // Create bind group for this frame
                    let camera_light_bg = state.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Scene Camera+Light BG"),
                        layout: &state.camera_light_layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: state.camera_buffer.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 1, resource: state.light_buffer.as_entire_binding() },
                        ],
                    });

                    // Render scene to offscreen texture
                    {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Scene Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &state.viewport_tex.color_view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.15, g: 0.15, b: 0.2, a: 1.0 }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                view: &state.viewport_tex.depth_view,
                                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                                stencil_ops: None,
                            }),
                            occlusion_query_set: None,
                            timestamp_writes: None,
                            multiview_mask: None,
                        });

                        rpass.set_pipeline(&state.scene_pipeline);
                        rpass.set_bind_group(0, &camera_light_bg, &[]);
                        rpass.set_bind_group(1, &state.dummy_texture_bg, &[]);

                        for mesh in &state.scene_meshes {
                            rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                            rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            rpass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                        }
                    }
                }

                // Clear the surface background
                {
                    let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Clear Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.12, g: 0.12, b: 0.15, a: 1.0 }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                        multiview_mask: None,
                    });
                }

                // Blit viewport texture to surface
                if let Some(vr) = &viewport_rect {
                    state.viewport_renderer.render(
                        &state.gpu.device, &state.gpu.queue, &mut encoder, &view,
                        &state.viewport_tex.color_view,
                        screen_w, screen_h,
                        vr.x, vr.y, vr.w, vr.h,
                    );
                }

                // Render UI overlay
                state.ui_renderer.render(
                    &state.gpu.device, &state.gpu.queue, &mut encoder, &view,
                    &state.ui.draw_list, screen_w, screen_h,
                );

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
    let mut app = EditorDemoApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
