use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::GpuContext;
use voltex_editor::{
    UiContext, UiRenderer,
    EditorSession,
    command::EditorCommand,
    panels::{hierarchy::hierarchy_panel, inspector::{inspector_panel, InspectorState}},
};

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
    session: EditorSession,
    inspector_state: InspectorState,
}

impl ApplicationHandler for EditorDemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Editor".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new(window.handle.clone());

        let ui = UiContext::new(
            gpu.config.width as f32,
            gpu.config.height as f32,
        );
        let ui_renderer = UiRenderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.surface_format,
            &ui.font,
        );

        let scene_path = "scenes/editor_scene.vscn".to_string();
        std::fs::create_dir_all("scenes").ok();
        let session = EditorSession::new(scene_path);

        self.state = Some(AppState {
            window,
            gpu,
            input: InputState::new(),
            timer: GameTimer::new(60),
            ui,
            ui_renderer,
            session,
            inspector_state: InspectorState::default(),
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
                state.ui.screen_width = size.width as f32;
                state.ui.screen_height = size.height as f32;
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
                state.input.begin_frame();

                let (mx, my) = state.input.mouse_position();
                let mouse_down = state.input.is_mouse_button_pressed(
                    winit::event::MouseButton::Left,
                );

                state.ui.begin_frame(mx as f32, my as f32, mouse_down);

                let screen_w = state.gpu.config.width as f32;
                let screen_h = state.gpu.config.height as f32;

                // ── Toolbar ──
                state.ui.begin_panel("Toolbar", 0.0, 0.0, screen_w, 36.0);
                if state.ui.button("Spawn") {
                    let _ = state.session.dispatch(EditorCommand::Spawn);
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
                state.ui.end_panel();

                // ── Hierarchy panel (left) ──
                hierarchy_panel(
                    &mut state.ui,
                    &mut state.session,
                    0.0, 36.0,
                    280.0, screen_h - 36.0,
                );

                // ── Inspector panel (right) ──
                inspector_panel(
                    &mut state.ui,
                    &mut state.session,
                    &mut state.inspector_state,
                    screen_w - 320.0, 36.0,
                    320.0, screen_h - 36.0,
                );

                state.ui.end_frame();

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

                let view = output.texture.create_view(
                    &wgpu::TextureViewDescriptor::default(),
                );
                let mut encoder = state.gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("Editor Encoder"),
                    },
                );

                {
                    let _clear_pass = encoder.begin_render_pass(
                        &wgpu::RenderPassDescriptor {
                            label: Some("Clear Pass"),
                            color_attachments: &[Some(
                                wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    depth_slice: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color {
                                            r: 0.08,
                                            g: 0.09,
                                            b: 0.10,
                                            a: 1.0,
                                        }),
                                        store: wgpu::StoreOp::Store,
                                    },
                                },
                            )],
                            depth_stencil_attachment: None,
                            occlusion_query_set: None,
                            timestamp_writes: None,
                            multiview_mask: None,
                        },
                    );
                }

                state.ui_renderer.render(
                    &state.gpu.device,
                    &state.gpu.queue,
                    &mut encoder,
                    &view,
                    &state.ui.draw_list,
                    screen_w,
                    screen_h,
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
