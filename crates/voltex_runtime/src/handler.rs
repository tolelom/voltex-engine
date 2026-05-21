//! winit event-loop adapter. Lives in its own module so `app.rs` stays
//! focused on subsystem ownership and `tick_systems_only`.

use voltex_math::{Mat4, Vec3};
use voltex_renderer::{Camera, FpsController, GpuContext, LightUniform};
use voltex_renderer_ecs::BasicRenderer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowId;

use crate::app::{RuntimeApp, RuntimeState};

pub(crate) struct RuntimeHandler {
    pub(crate) app: Option<RuntimeApp>,
}

impl ApplicationHandler for RuntimeHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(app) = self.app.as_mut() else { return };
        if app.runtime_state.is_some() {
            return;
        }
        let window = voltex_platform::VoltexWindow::new(event_loop, &app.config.window);
        let gpu = GpuContext::new(window.handle.clone());
        let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, gpu.surface_format, 1024);
        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
        let mut camera = Camera::new(Vec3::new(0.0, 10.0, 20.0), aspect);
        camera.pitch = -0.4;
        let fps_controller = FpsController::new();
        let light = LightUniform::new();
        app.runtime_state = Some(RuntimeState {
            window,
            gpu,
            renderer,
            camera,
            fps_controller,
            light,
        });
        // Run one-shot gpu_init callback now that the GPU exists. We have to
        // hand the device/queue out of runtime_state, but a re-borrow of the
        // whole app is safe because the callback signature takes &mut app.
        if let Some(init) = app.config.gpu_init.take() {
            // SAFETY: we briefly split the borrow of `app` to give the user
            // both `&mut RuntimeApp` and `&Device`/`&Queue`. We immediately
            // drop the device/queue refs after `init` returns.
            let state_ptr: *const RuntimeState =
                app.runtime_state.as_ref().expect("just inserted") as *const _;
            unsafe {
                let st = &*state_ptr;
                init(app, &st.gpu.device, &st.gpu.queue);
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let Some(app) = self.app.as_mut() else { return };
        let Some(state) = app.runtime_state.as_mut() else { return };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state: key_state,
                        ..
                    },
                ..
            } => {
                let pressed = key_state == ElementState::Pressed;
                app.input.process_key(key_code, pressed);
                if pressed && key_code == KeyCode::Escape {
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
                app.input.process_mouse_move(position.x, position.y);
            }
            WindowEvent::MouseInput { state: bs, button, .. } => {
                app.input.process_mouse_button(button, bs == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                app.input.process_scroll(y);
            }
            WindowEvent::RedrawRequested => {
                redraw(app, event_loop);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(app) = self.app.as_ref() {
            if let Some(state) = app.runtime_state.as_ref() {
                state.window.request_redraw();
            }
        }
    }
}

fn redraw(app: &mut RuntimeApp, event_loop: &ActiveEventLoop) {
    app.timer.tick();
    let dt = app
        .config
        .fixed_dt
        .unwrap_or_else(|| app.timer.frame_dt());

    // Camera update — both mouse look AND WASD/Space/Shift movement are gated
    // on RMB-held, matching the established scene-view convention (Bevy /
    // Unity / Godot). With WASD released, games' `add_input_system` user
    // systems are free to use WASD for player movement without conflict.
    let state = app.runtime_state.as_mut().expect("redraw needs runtime_state");
    if app.input.is_mouse_button_pressed(MouseButton::Right) {
        let (dx, dy) = app.input.mouse_delta();
        state.fps_controller.process_mouse(&mut state.camera, dx, dy);
        let mut fwd = 0.0;
        let mut right = 0.0;
        let mut up = 0.0;
        if app.input.is_key_pressed(KeyCode::KeyW) { fwd += 1.0; }
        if app.input.is_key_pressed(KeyCode::KeyS) { fwd -= 1.0; }
        if app.input.is_key_pressed(KeyCode::KeyD) { right += 1.0; }
        if app.input.is_key_pressed(KeyCode::KeyA) { right -= 1.0; }
        if app.input.is_key_pressed(KeyCode::Space) { up += 1.0; }
        if app.input.is_key_pressed(KeyCode::ShiftLeft) { up -= 1.0; }
        state.fps_controller.process_movement(&mut state.camera, fwd, right, up, dt);
    }

    app.tick_systems_only(dt);
    // End-of-frame: clear just_pressed / just_released / mouse_delta AFTER
    // user systems have observed them. Previously this ran before
    // tick_systems_only, so `is_key_just_pressed` was always false in user
    // systems — fixed in v1.0.1.
    app.input.begin_frame();

    // Acquire swapchain texture (graceful resize/recover).
    let state = app.runtime_state.as_mut().unwrap();
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
    let mut encoder = state.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("RuntimeApp Render Encoder"),
    });

    let view_proj: Mat4 = state.camera.view_projection();
    let cam_pos = state.camera.position;

    state.renderer.render(
        &state.gpu.queue,
        &mut encoder,
        &view,
        &state.gpu.depth_view,
        wgpu::Color { r: 0.02, g: 0.02, b: 0.05, a: 1.0 },
        &mut app.world,
        &app.assets,
        view_proj,
        cam_pos,
        state.light,
    );

    state.gpu.queue.submit(std::iter::once(encoder.finish()));
    output.present();
}

