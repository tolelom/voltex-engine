use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::GpuContext;
use voltex_editor::{UiContext, UiRenderer, DockTree, DockNode, Axis, Rect, LayoutState};

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
            vec!["Debug", "Viewport", "Properties", "Console"],
        );

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

                // Gather mouse state
                let (mx, my) = state.input.mouse_position();
                let mouse_down = state.input.is_mouse_button_pressed(
                    winit::event::MouseButton::Left,
                );

                let dt = state.timer.frame_dt();
                let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };

                // Begin UI frame
                state.ui.begin_frame(mx as f32, my as f32, mouse_down);


                // Dock layout
                let screen_w = state.gpu.config.width as f32;
                let screen_h = state.gpu.config.height as f32;
                let areas = state.dock.layout(Rect { x: 0.0, y: 0.0, w: screen_w, h: screen_h });
                state.dock.update(mx as f32, my as f32, mouse_down);
                state.dock.draw_chrome(&mut state.ui);

                for (panel_id, rect) in &areas {
                    state.ui.layout = LayoutState::new(rect.x + 4.0, rect.y + 4.0);
                    match panel_id {
                        0 => {
                            state.ui.text("Debug Panel");
                            state.ui.text(&format!("FPS: {:.0}", fps));
                            if state.ui.button("Click Me") {
                                state.counter += 1;
                            }
                            state.ui.text(&format!("Count: {}", state.counter));
                            state.speed = state.ui.slider("Speed", state.speed, 0.0, 10.0);
                            state.show_grid = state.ui.checkbox("Show Grid", state.show_grid);
                        }
                        1 => {
                            state.ui.text("Viewport");
                            state.ui.text("(3D scene here)");
                        }
                        2 => {
                            state.ui.text("Properties");
                            state.ui.text(&format!("Speed: {:.1}", state.speed));
                            state.ui.text(&format!("Grid: {}", state.show_grid));
                        }
                        3 => {
                            state.ui.text("Console");
                            state.ui.text("> Ready.");
                        }
                        _ => {}
                    }
                }

                state.ui.end_frame();

                // Acquire surface texture
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
                        label: Some("Editor Demo Encoder"),
                    },
                );

                // Clear the background
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
                                            r: 0.12,
                                            g: 0.12,
                                            b: 0.15,
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
                    // Pass drops here, clearing the surface
                }

                // Render UI overlay
                let screen_w = state.gpu.config.width as f32;
                let screen_h = state.gpu.config.height as f32;
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
