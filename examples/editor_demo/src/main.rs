use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::{GpuContext, LightUniform, MaterialUniform, Mesh, MeshVertex};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};
use voltex_editor::{
    UiContext, UiRenderer, DockTree, DockNode, Axis, Rect,
    OrbitCamera, ViewportTexture, ViewportRenderer, VIEWPORT_COLOR_FORMAT, VIEWPORT_DEPTH_FORMAT,
    hierarchy_panel, inspector_panel, InspectorState,
    AssetBrowser, asset_browser_panel,
    EditorSession, EditorCommand,
    pick_entity,
    GizmoMode, GizmoRenderer, GizmoState,
    EditorMode, LifecycleEvent,
};
use voltex_ecs::entity::Entity;
use voltex_ecs::{Bounds, MaterialHandle, MeshHandle};
use voltex_math::{Vec3, AABB};
use voltex_physics::{physics_step, Collider, PhysicsConfig, RigidBody};

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
    // ECS — wrapped in EditorSession for command-based mutation + auto-save.
    // `session.selected` is the single source of truth for the selected entity.
    session: EditorSession,
    inspector_state: InspectorState,
    // Viewport state
    orbit_cam: OrbitCamera,
    viewport_tex: ViewportTexture,
    viewport_renderer: ViewportRenderer,
    // ECS-driven renderer (mesh_shader.wgsl). Owns pipeline + per-entity UBO.
    render_assets: RenderAssets,
    basic_renderer: BasicRenderer,
    cube_mesh_h: MeshHandle,
    #[allow(dead_code)]
    ground_mesh_h: MeshHandle,
    default_mat_h: MaterialHandle,
    // Gizmo state + renderer (E1)
    gizmo_state: GizmoState,
    gizmo_renderer: GizmoRenderer,
    gizmo_dragging: bool,
    // Physics config for Play mode (E2)
    physics_config: PhysicsConfig,
    // Asset browser
    asset_browser: AssetBrowser,
    // Mouse tracking
    prev_mouse: (f32, f32),
    left_dragging: bool,
    middle_dragging: bool,
    scroll_delta: f32,
    // Track left-button-down position so we can distinguish a click (pick)
    // from a drag (orbit). Set on press, consumed on release.
    left_down_pos: Option<(f32, f32)>,
    // Single-shot click event: Some((x, y)) on the frame the left button is
    // released without significant drag. Consumed by the picking step.
    pending_click: Option<(f32, f32)>,
}

fn make_unit_cube(device: &wgpu::Device) -> Mesh {
    let s = 0.5;
    let mut verts = Vec::new();
    let mut indices = Vec::new();
    let mut add_face = |positions: [[f32; 3]; 4], normal: [f32; 3]| {
        let base = verts.len() as u32;
        for pos in &positions {
            verts.push(MeshVertex {
                position: *pos,
                normal,
                uv: [0.0, 0.0],
                tangent: [1.0, 0.0, 0.0, 1.0],
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    add_face([[-s,  s, -s], [ s,  s, -s], [ s,  s,  s], [-s,  s,  s]], [0.0,  1.0,  0.0]);
    add_face([[-s, -s,  s], [ s, -s,  s], [ s, -s, -s], [-s, -s, -s]], [0.0, -1.0,  0.0]);
    add_face([[-s, -s,  s], [-s,  s,  s], [ s,  s,  s], [ s, -s,  s]], [0.0,  0.0,  1.0]);
    add_face([[ s, -s, -s], [ s,  s, -s], [-s,  s, -s], [-s, -s, -s]], [0.0,  0.0, -1.0]);
    add_face([[ s, -s,  s], [ s,  s,  s], [ s,  s, -s], [ s, -s, -s]], [1.0,  0.0,  0.0]);
    add_face([[-s, -s, -s], [-s,  s, -s], [-s,  s,  s], [-s, -s,  s]], [-1.0, 0.0,  0.0]);
    Mesh::new(device, &verts, &indices)
}

fn make_unit_ground(device: &wgpu::Device) -> Mesh {
    let s = 5.0;
    let verts = vec![
        MeshVertex { position: [-s, 0.0, -s], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s, 0.0, -s], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s, 0.0,  s], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s, 0.0,  s], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
    ];
    let indices = vec![0u32, 1, 2, 0, 2, 3];
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

        // ECS-driven renderer setup. RenderAssets owns the GPU meshes; BasicRenderer
        // owns the pipeline + dynamic UBO + bind group + default white texture.
        let mut render_assets = RenderAssets::new();
        let cube_mesh_h = render_assets.add_mesh(make_unit_cube(&gpu.device));
        let ground_mesh_h = render_assets.add_mesh(make_unit_ground(&gpu.device));
        let default_mat_h = render_assets.add_material(MaterialUniform::new());
        let basic_renderer = BasicRenderer::new(
            &gpu.device,
            &gpu.queue,
            VIEWPORT_COLOR_FORMAT,
            256,
        );

        let gizmo_renderer = GizmoRenderer::new(
            &gpu.device,
            VIEWPORT_COLOR_FORMAT,
            VIEWPORT_DEPTH_FORMAT,
        );
        let gizmo_state = GizmoState::new();

        // ECS Session setup — World lives inside the EditorSession so Toolbar
        // actions can be logged and auto-saved. Seed entities go through the
        // command dispatcher so they appear in the jsonl log and survive a
        // Save → Load round-trip.
        std::fs::create_dir_all("scenes").ok();
        let mut session = EditorSession::new("scenes/editor_demo.vscn");

        let seed = |session: &mut EditorSession, pos: Vec3, tag: &str| -> Entity {
            let e = session.dispatch(EditorCommand::Spawn).unwrap();
            session.dispatch(EditorCommand::SetTransform {
                entity: e,
                position: pos,
                rotation: Vec3::ZERO,
                scale: Vec3::new(1.0, 1.0, 1.0),
            }).unwrap();
            session.dispatch(EditorCommand::SetTag {
                entity: e,
                tag: tag.to_string(),
            }).unwrap();
            e
        };
        let ground_e = seed(&mut session, Vec3::new(0.0, 0.0, 0.0), "Ground");
        let cube1_e = seed(&mut session, Vec3::new(0.0, 0.5, 0.0), "Cube1");
        let cube2_e = seed(&mut session, Vec3::new(2.0, 0.5, 1.0), "Cube2");
        let cube3_e = seed(&mut session, Vec3::new(-1.5, 0.5, -1.0), "Cube3");

        // Attach render handles directly. E3 will introduce an AddComponent
        // EditorCommand; until then seed entities bypass the dispatcher for
        // these two components. The ComponentRegistry already handles
        // MeshHandle/MaterialHandle serialization (C0a), so Save/Load round
        // trips preserve them.
        session.world.add(ground_e, MeshHandle(ground_mesh_h.0));
        session.world.add(ground_e, MaterialHandle(default_mat_h.0));
        session.world.add(cube1_e, MeshHandle(cube_mesh_h.0));
        session.world.add(cube1_e, MaterialHandle(default_mat_h.0));
        session.world.add(cube2_e, MeshHandle(cube_mesh_h.0));
        session.world.add(cube2_e, MaterialHandle(default_mat_h.0));
        session.world.add(cube3_e, MeshHandle(cube_mesh_h.0));
        session.world.add(cube3_e, MaterialHandle(default_mat_h.0));

        // Bounds for AABB picking (E1). Local-space mesh AABB; world AABB is
        // rebuilt each frame inside pick_entity using WorldTransform.
        let cube_bounds = Bounds(AABB::new(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)));
        let ground_bounds = Bounds(AABB::new(Vec3::new(-5.0, -0.01, -5.0), Vec3::new(5.0, 0.01, 5.0)));
        session.world.add(ground_e, ground_bounds);
        session.world.add(cube1_e, cube_bounds);
        session.world.add(cube2_e, cube_bounds);
        session.world.add(cube3_e, cube_bounds);

        // Cube2 is child of Cube1 — through the command path so undo/redo
        // and the log capture the relationship.
        session.dispatch(EditorCommand::AddChild {
            parent: cube1_e,
            child: cube2_e,
        }).unwrap();

        // E2: lifecycle callback. On Play, attach RigidBody+Collider to the
        // cubes so physics_step pulls them down; ground gets a static body
        // with a Box collider. Stop restores via snapshot — these components
        // vanish because RigidBody/Collider are not in ComponentRegistry.
        let cube_ids = [cube1_e, cube2_e, cube3_e];
        let ground_id = ground_e;
        session.add_lifecycle(move |event, world| {
            if event == LifecycleEvent::PlayStart {
                for &cube in &cube_ids {
                    if world.is_alive(cube) {
                        world.add(cube, RigidBody::dynamic(1.0));
                        world.add(cube, Collider::Sphere { radius: 0.5 });
                    }
                }
                if world.is_alive(ground_id) {
                    world.add(ground_id, RigidBody::statik());
                    world.add(ground_id, Collider::Box {
                        half_extents: Vec3::new(5.0, 0.05, 5.0),
                    });
                }
            }
            // PlayStop: no-op. snapshot restore handles cleanup.
        });

        let physics_config = PhysicsConfig::default();

        self.state = Some(AppState {
            window,
            gpu,
            input: InputState::new(),
            timer: GameTimer::new(60),
            ui,
            ui_renderer,
            dock,
            orbit_cam,
            viewport_tex,
            viewport_renderer,
            render_assets,
            basic_renderer,
            cube_mesh_h,
            ground_mesh_h,
            default_mat_h,
            gizmo_state,
            gizmo_renderer,
            gizmo_dragging: false,
            physics_config,
            asset_browser,
            session,
            inspector_state: InspectorState::default(),
            prev_mouse: (0.0, 0.0),
            left_dragging: false,
            middle_dragging: false,
            scroll_delta: 0.0,
            left_down_pos: None,
            pending_click: None,
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
                    winit::event::MouseButton::Left => {
                        state.left_dragging = pressed;
                        let pos = state.input.mouse_position();
                        let p = (pos.0 as f32, pos.1 as f32);
                        if pressed {
                            state.left_down_pos = Some(p);
                            state.gizmo_dragging = false;
                        } else {
                            // Release: if a gizmo drag was active, end it.
                            if state.gizmo_dragging {
                                if let Some(sel) = state.session.selected {
                                    if let Some(drag) = state.gizmo_state.end_drag(&mut state.session.world, sel) {
                                        let _ = state.session.dispatch(EditorCommand::SetTransform {
                                            entity: sel,
                                            position: drag.final_transform.position,
                                            rotation: drag.final_transform.rotation,
                                            scale: drag.final_transform.scale,
                                        });
                                    }
                                }
                                state.gizmo_dragging = false;
                                state.left_down_pos = None;
                            } else if let Some(down) = state.left_down_pos.take() {
                                let dx = (p.0 - down.0).abs();
                                let dy = (p.1 - down.1).abs();
                                if dx + dy < 4.0 {
                                    state.pending_click = Some(p);
                                }
                            }
                        }
                    }
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

                // Physics: only ticks while in Play mode.
                if state.session.mode == EditorMode::Play {
                    physics_step(&mut state.session.world, &state.physics_config);
                }

                let (mx, my) = state.input.mouse_position();
                let mx = mx as f32;
                let my = my as f32;
                let mouse_down = state.input.is_mouse_button_pressed(winit::event::MouseButton::Left);

                state.ui.begin_frame(mx, my, mouse_down);

                let screen_w = state.gpu.config.width as f32;
                let screen_h = state.gpu.config.height as f32;

                // ── Toolbar (Play/Stop gates the other controls) ──
                const TOOLBAR_H: f32 = 32.0;
                state.ui.begin_panel("Toolbar", 0.0, 0.0, screen_w, TOOLBAR_H);
                let in_play = state.session.mode == EditorMode::Play;
                if in_play {
                    if state.ui.button("Stop") {
                        state.session.set_mode(EditorMode::Edit);
                        state.session.selected = None;
                    }
                } else {
                    if state.ui.button("Play") {
                        state.session.set_mode(EditorMode::Play);
                    }
                    if state.ui.button("Spawn") {
                        if let Ok(e) = state.session.dispatch(EditorCommand::Spawn) {
                            let _ = state.session.dispatch(EditorCommand::SetTransform {
                                entity: e,
                                position: Vec3::new(0.0, 0.5, 0.0),
                                rotation: Vec3::ZERO,
                                scale: Vec3::new(1.0, 1.0, 1.0),
                            });
                            state.session.world.add(e, MeshHandle(state.cube_mesh_h.0));
                            state.session.world.add(e, MaterialHandle(state.default_mat_h.0));
                            state.session.selected = Some(e);
                        }
                    }
                    if state.ui.button("Save") {
                        let path = state.session.scene_path.clone();
                        let _ = state.session.dispatch(EditorCommand::SaveScene { path });
                    }
                    if state.ui.button("Load") {
                        let path = state.session.scene_path.clone();
                        let _ = state.session.dispatch(EditorCommand::LoadScene { path });
                        state.session.selected = None;
                    }
                    if state.ui.button("Undo") { state.session.undo(); }
                    if state.ui.button("Redo") { state.session.redo(); }
                    if state.ui.button("Move") { state.gizmo_state.set_mode(GizmoMode::Translate); }
                    if state.ui.button("Rotate") { state.gizmo_state.set_mode(GizmoMode::Rotate); }
                    if state.ui.button("Scale") { state.gizmo_state.set_mode(GizmoMode::Scale); }
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
                            hierarchy_panel(&mut state.ui, &mut state.session, rect.x, rect.y, rect.w, rect.h);
                        }
                        1 => {
                            // Viewport panel (3D rendering happens below)
                            viewport_rect = Some(*rect);
                        }
                        2 => {
                            inspector_panel(
                                &mut state.ui,
                                &mut state.session,
                                &mut state.inspector_state,
                                rect.x, rect.y, rect.w, rect.h,
                            );
                        }
                        3 => {
                            asset_browser_panel(&mut state.ui, &mut state.asset_browser, rect);
                        }
                        _ => {}
                    }
                }

                state.ui.end_frame();

                // Orbit camera input (only when mouse is over viewport).
                // Gizmo dragging preempts orbit so the camera doesn't fight the gizmo.
                if let Some(vr) = &viewport_rect {
                    if vr.contains(mx, my) {
                        let dx = mx - state.prev_mouse.0;
                        let dy = my - state.prev_mouse.1;
                        if state.left_dragging && !state.gizmo_dragging { state.orbit_cam.orbit(dx, dy); }
                        if state.middle_dragging { state.orbit_cam.pan(dx, dy); }
                        if state.scroll_delta.abs() > 0.0 {
                            state.orbit_cam.zoom(state.scroll_delta);
                        }
                    }
                }

                // ── Gizmo drag begin / update ── (Edit mode only)
                if let (Some(sel), Some(vr)) = (state.session.selected, viewport_rect) {
                    let in_play = state.session.mode == EditorMode::Play;
                    let aspect = vr.w / vr.h.max(1.0);
                    let vp = state.orbit_cam.view_projection(aspect);
                    let cam_pos = state.orbit_cam.position();
                    let rect_tuple = (vr.x, vr.y, vr.w, vr.h);
                    let mouse = (mx, my);

                    if !in_play
                        && state.left_dragging
                        && !state.gizmo_dragging
                        && state.left_down_pos.map(|p| (p.0 - mx).abs() + (p.1 - my).abs() < 8.0).unwrap_or(false)
                        && vr.contains(mx, my)
                        && state.gizmo_state.try_begin_drag(
                            &mut state.session.world,
                            sel,
                            &vp,
                            cam_pos,
                            rect_tuple,
                            mouse,
                        ) {
                            state.gizmo_dragging = true;
                        }
                    if state.gizmo_dragging && !in_play {
                        state.gizmo_state.update_drag(
                            &mut state.session.world,
                            sel,
                            &vp,
                            cam_pos,
                            rect_tuple,
                            mouse,
                        );
                    }
                }

                // Viewport click → raycast pick → update session.selected.
                if let (Some(click), Some(vr)) = (state.pending_click.take(), viewport_rect) {
                    if vr.contains(click.0, click.1) {
                        let aspect = vr.w / vr.h.max(1.0);
                        let vp = state.orbit_cam.view_projection(aspect);
                        let cam_pos = state.orbit_cam.position();
                        let hit = pick_entity(
                            &mut state.session.world,
                            &vp,
                            cam_pos,
                            vr.x, vr.y, vr.w, vr.h,
                            click.0, click.1,
                            0.7,
                        );
                        state.session.selected = hit;
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

                // Render 3D scene to viewport texture via the ECS-driven path.
                if let Some(vr) = &viewport_rect {
                    state.viewport_tex.ensure_size(
                        &state.gpu.device,
                        vr.w.max(1.0) as u32,
                        vr.h.max(1.0) as u32,
                    );

                    let aspect = vr.w / vr.h.max(1.0);
                    let vp = state.orbit_cam.view_projection(aspect);
                    let cam_pos = state.orbit_cam.position();

                    let light = LightUniform {
                        direction: [0.3, -1.0, -0.5],
                        _padding1: 0.0,
                        color: [1.0, 0.95, 0.9],
                        ambient_strength: 0.15,
                    };

                    state.basic_renderer.render(
                        &state.gpu.queue,
                        &mut encoder,
                        &state.viewport_tex.color_view,
                        &state.viewport_tex.depth_view,
                        wgpu::Color { r: 0.15, g: 0.15, b: 0.2, a: 1.0 },
                        &mut state.session.world,
                        &state.render_assets,
                        vp,
                        cam_pos,
                        light,
                    );

                    // Gizmo overlay (depth-off, always on top of the scene).
                    // Hidden in Play mode — no editing during simulation.
                    if state.session.mode == EditorMode::Edit {
                        if let Some(sel) = state.session.selected {
                            if let Some(origin) = GizmoState::gizmo_origin(&state.session.world, sel) {
                                state.gizmo_renderer.render(
                                    &state.gpu.queue,
                                    &mut encoder,
                                    &state.viewport_tex.color_view,
                                    &state.viewport_tex.depth_view,
                                    vp,
                                    origin,
                                    state.gizmo_state.mode,
                                    state.gizmo_state.active_axis(),
                                );
                            }
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
