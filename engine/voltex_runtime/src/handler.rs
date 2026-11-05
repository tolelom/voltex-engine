//! winit event-loop adapter. Lives in its own module so `app/mod.rs` stays
//! focused on subsystem ownership and `tick_systems_only`.
//!
//! One frame = [`redraw`]: frame clock → fly camera → `tick_systems_only` →
//! GPU resource sync → game callbacks → window title → input edge clear →
//! render (+ HUD overlay) → present.

use voltex_math::Mat4;
use voltex_platform::InputState;
use voltex_renderer::{Camera, FpsController, GpuContext, LightUniform};
use voltex_renderer_ecs::{BasicRenderer, FrameTarget, FrameView};
use voltex_ecs::World;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowId;

use crate::app::{GpuWindowState, HudFn, RuntimeApp};

/// Entity capacity passed to [`BasicRenderer::new`] (`max_entities`).
const RENDERER_MAX_ENTITIES: usize = 1024;
/// Orthographic cameras get a symmetric depth range of ±this around the eye,
/// so objects on either side of the eye plane stay visible.
const ORTHO_DEPTH_HALF_RANGE: f32 = 100.0;
/// Background colour the 3D pass clears to (near-black blue).
const CLEAR_COLOR: wgpu::Color = wgpu::Color { r: 0.02, g: 0.02, b: 0.05, a: 1.0 };

pub(crate) struct RuntimeHandler {
    pub(crate) app: Option<RuntimeApp>,
}

impl ApplicationHandler for RuntimeHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(app) = self.app.as_mut() else { return };
        if app.gpu_window.is_some() {
            return;
        }
        let window = voltex_platform::VoltexWindow::new(event_loop, &app.config.window);
        // Seed InputState with the real window size in PHYSICAL pixels —
        // same unit as CursorMoved positions, so games can do screen→NDC
        // math that stays correct at any DPI scale (the config size is
        // logical; at 125% scale physical = logical × 1.25).
        {
            let (w, h) = window.inner_size();
            app.input.set_window_size(w, h);
        }
        let gpu = match GpuContext::new(window.handle.clone()) {
            Ok(gpu) => gpu,
            Err(e) => {
                // GPU acquisition failed (headless / unsupported GPU). Surface a
                // clear message and exit the loop cleanly rather than panicking
                // mid-`resumed`.
                eprintln!("voltex_runtime: {e}");
                event_loop.exit();
                return;
            }
        };
        crate::crash::set_crash_gpu_adapter(format!(
            "{} ({:?})",
            gpu.adapter_info.name, gpu.adapter_info.backend
        ));
        let mut renderer = BasicRenderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.surface_format,
            RENDERER_MAX_ENTITIES,
        );
        renderer.set_shadows(app.config.enable_shadows);
        renderer.set_pbr_enabled(app.config.enable_pbr);
        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
        let mut camera = Camera::new(app.config.camera_position, aspect);
        camera.yaw = app.config.camera_yaw;
        camera.pitch = app.config.camera_pitch;
        camera.fov_y = app.config.camera_fov_y;
        camera.ortho_height = app.config.camera_ortho_height;
        if camera.ortho_height.is_some() {
            camera.near = -ORTHO_DEPTH_HALF_RANGE;
            camera.far = ORTHO_DEPTH_HALF_RANGE;
        }
        let fps_controller = FpsController::new();
        let light = LightUniform {
            direction: app.config.light_direction,
            _padding1: 0.0,
            color: app.config.light_color,
            ambient_strength: app.config.light_ambient,
        };
        // Construct the in-window HUD context + GPU renderer only when enabled.
        let (ui_context, ui_renderer) = if app.config.enable_hud {
            let ui = voltex_editor::UiContext::new(
                gpu.config.width as f32,
                gpu.config.height as f32,
            );
            let ui_renderer = voltex_editor::UiRenderer::new(
                &gpu.device,
                &gpu.queue,
                gpu.surface_format,
                &ui.font,
            );
            (Some(ui), Some(ui_renderer))
        } else {
            (None, None)
        };
        app.gpu_window = Some(GpuWindowState {
            window,
            gpu,
            renderer,
            camera,
            fps_controller,
            light,
            ui_context,
            ui_renderer,
        });
        // Run the one-shot gpu_init callback now that the GPU exists.
        if let Some(init) = app.config.gpu_init.take() {
            let gpu_window_ptr: *const GpuWindowState =
                app.gpu_window.as_ref().expect("just inserted") as *const _;
            // SAFETY: `init` gets `&mut RuntimeApp` *and* `&Device`/`&Queue`
            // that live inside `app.gpu_window` — an alias the borrow checker
            // cannot see. This relies on `init` not replacing or dropping
            // `app.gpu_window` (that would free the device under the borrow);
            // the device/queue refs do not outlive the call.
            unsafe {
                let gpu_window = &*gpu_window_ptr;
                init(app, &gpu_window.gpu.device, &gpu_window.gpu.queue);
            }
        }
        // Runs after gpu_init so primitives added in either the scene builder
        // or gpu_init are covered.
        sync_gpu_resources(app);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let Some(app) = self.app.as_mut() else { return };
        let Some(gpu_window) = app.gpu_window.as_mut() else { return };

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
                // Window minimisation produces a 0x0 resize event on most
                // platforms. wgpu's `configure_surface` panics on a zero-sized
                // surface, so we skip both the GPU resize and the aspect
                // update until the user restores the window.
                if size.width > 0 && size.height > 0 {
                    gpu_window.gpu.resize(size.width, size.height);
                    gpu_window.camera.aspect = size.width as f32 / size.height as f32;
                    // Keep InputState's physical window size current so
                    // screen→NDC math in game systems tracks resizes. The
                    // 0x0 minimise event is skipped with the GPU resize —
                    // the last valid size is kept.
                    app.input.set_window_size(size.width, size.height);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                app.input.process_mouse_move(position.x, position.y);
            }
            WindowEvent::MouseInput { state: button_state, button, .. } => {
                app.input
                    .process_mouse_button(button, button_state == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                app.input.process_scroll(y);
            }
            WindowEvent::RedrawRequested => {
                redraw(app);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(app) = self.app.as_ref() {
            if let Some(gpu_window) = app.gpu_window.as_ref() {
                gpu_window.window.request_redraw();
            }
        }
    }
}

/// One frame. Each step is a helper below; the order matters (see the module
/// doc and `docs/architecture.md` "한 프레임").
fn redraw(app: &mut RuntimeApp) {
    let dt = advance_frame_clock(app);
    update_fly_camera(app, dt);
    app.tick_systems_only(dt);
    sync_gpu_resources(app);
    run_frame_callbacks(app, dt);
    update_window_title(app, dt);
    // End-of-frame: clear just_pressed / just_released / mouse_delta AFTER
    // user systems have observed them. Previously this ran before
    // tick_systems_only, so `is_key_just_pressed` was always false in user
    // systems — fixed in v1.0.1.
    app.input.clear_frame_edges();
    render_frame(app);
}

/// Tick the wallclock frame timer and pick this frame's `dt`: the forced
/// `config.fixed_frame_dt` if set, else the measured (clamped) frame time.
fn advance_frame_clock(app: &mut RuntimeApp) -> f32 {
    app.frame_timer.tick();
    app.config
        .fixed_frame_dt
        .unwrap_or_else(|| app.frame_timer.frame_dt())
}

/// Default scene-view fly camera — both mouse look AND WASD/Space/Shift
/// movement are gated on RMB-held, matching the established scene-view
/// convention (Bevy / Unity / Godot). With RMB released, games'
/// `add_input_system` user systems are free to use WASD for player movement
/// without conflict. A `set_camera_fn` updater later in the frame overrides it.
fn update_fly_camera(app: &mut RuntimeApp, dt: f32) {
    let gpu_window = app.gpu_window.as_mut().expect("redraw needs gpu_window");
    let input = &app.input;
    if !input.is_mouse_button_pressed(MouseButton::Right) {
        return;
    }
    let (dx, dy) = input.mouse_delta();
    gpu_window.fps_controller.process_mouse(&mut gpu_window.camera, dx, dy);
    let axis = |positive: KeyCode, negative: KeyCode| -> f32 {
        let mut value = 0.0;
        if input.is_key_pressed(positive) { value += 1.0; }
        if input.is_key_pressed(negative) { value -= 1.0; }
        value
    };
    let forward = axis(KeyCode::KeyW, KeyCode::KeyS);
    let right = axis(KeyCode::KeyD, KeyCode::KeyA);
    let up = axis(KeyCode::Space, KeyCode::ShiftLeft);
    gpu_window
        .fps_controller
        .process_movement(&mut gpu_window.camera, forward, right, up, dt);
}

/// Give GPU resources to entities that need them. Idempotent and cheap when
/// nothing is pending, so it runs once at GPU init and then every frame
/// (entities spawned at runtime by Lua / projectile factories / … would
/// otherwise never get GPU resources):
///
/// 1. `materialize_renderables` — declarative `PrimitiveMesh` / `MaterialDesc`
///    → mesh + material handles.
/// 2. `resolve_asset_paths` — scene-persisted `TexturePath` / `MeshPath` →
///    handles (failure-cached).
fn sync_gpu_resources(app: &mut RuntimeApp) {
    // Disjoint field borrows: the device/queue are read from `gpu_window`
    // while `world` / `assets` / `asset_path_cache` are mutated.
    let gpu = &app.gpu_window.as_ref().expect("needs gpu_window").gpu;
    voltex_renderer_ecs::materialize_renderables(&mut app.world, &mut app.assets, &gpu.device);
    crate::asset_paths::resolve_asset_paths(
        &mut app.world,
        &mut app.assets,
        &mut app.asset_path_cache,
        &gpu.device,
        &gpu.queue,
        app.config.vfs.as_ref(),
        app.asset_root.as_deref(),
    );
}

/// Game-registered per-frame updaters, after the tick so they see this
/// frame's world: camera (overrides the fly camera) → directional light →
/// point lights (the `Vec` uploaded in [`render_frame`]) → debug gizmos
/// (buffer cleared first).
fn run_frame_callbacks(app: &mut RuntimeApp, dt: f32) {
    if let Some(camera_fn) = app.camera_fn.as_mut() {
        if let Some(gpu_window) = app.gpu_window.as_mut() {
            camera_fn(&mut gpu_window.camera, &app.world, dt);
        }
    }
    if let Some(light_fn) = app.light_fn.as_mut() {
        if let Some(gpu_window) = app.gpu_window.as_mut() {
            light_fn(&mut gpu_window.light, &app.world, dt);
        }
    }
    if let Some(point_lights_fn) = app.point_lights_fn.as_mut() {
        point_lights_fn(&mut app.point_lights, &app.world, dt);
    }
    if let Some(debug_draw_fn) = app.debug_draw_fn.as_mut() {
        app.debug_lines.clear();
        debug_draw_fn(&mut app.debug_lines, &app.world, dt);
    }
}

/// Feed the HUD this frame's dt and push a new OS window title when its
/// update interval elapses. `Hud::tick` returns `None` either when the
/// interval has not elapsed or when fps_in_title is disabled — both leave the
/// title alone. Also records the FPS for crash reports.
fn update_window_title(app: &mut RuntimeApp, dt: f32) {
    if let Some(new_title) = app.hud.tick(dt) {
        if let Some(gpu_window) = app.gpu_window.as_ref() {
            gpu_window.window.handle.set_title(&new_title);
        }
    }
    crate::crash::set_crash_fps(app.hud.average_fps());
}

/// Acquire the swapchain texture, draw the 3D scene (+ HUD overlay) into it
/// and present. A lost surface is reconfigured and the frame skipped; any
/// other acquire failure just skips the frame.
fn render_frame(app: &mut RuntimeApp) {
    let gpu_window = app.gpu_window.as_mut().expect("redraw needs gpu_window");
    let output = match gpu_window.gpu.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Lost => {
            let (w, h) = gpu_window.window.inner_size();
            gpu_window.gpu.resize(w, h);
            return;
        }
        _ => return,
    };
    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder =
        gpu_window.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("RuntimeApp Render Encoder"),
        });

    let view_proj: Mat4 = gpu_window.camera.view_projection();
    let camera_position = gpu_window.camera.position;

    gpu_window.renderer.set_point_lights(&gpu_window.gpu.queue, &app.point_lights);
    gpu_window.renderer.set_debug_lines(&gpu_window.gpu.queue, app.debug_lines.vertices());
    gpu_window.renderer.render_frame(
        &gpu_window.gpu.queue,
        &mut encoder,
        FrameTarget {
            color_view: &view,
            depth_view: &gpu_window.gpu.depth_view,
            clear_color: CLEAR_COLOR,
        },
        &mut app.world,
        &app.assets,
        &FrameView {
            view_proj,
            camera_pos: camera_position,
            light: gpu_window.light,
        },
    );

    if app.config.enable_hud {
        draw_hud_overlay(gpu_window, &mut app.hud_fn, &app.input, &app.world, &mut encoder, &view);
    }

    gpu_window.gpu.queue.submit(std::iter::once(encoder.finish()));
    gpu_window.gpu.queue.present(output);
}

/// In-window HUD overlay pass — drawn over the 3D scene (LoadOp::Load) into
/// the same encoder, before present. No-op without a registered `hud_fn` or
/// without the UI context/renderer (`with_hud(true)` creates them).
fn draw_hud_overlay(
    gpu_window: &mut GpuWindowState,
    hud_fn: &mut Option<HudFn>,
    input: &InputState,
    world: &World,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
) {
    let Some(hud_fn) = hud_fn.as_mut() else { return };
    let (mouse_x, mouse_y) = input.mouse_position();
    let mouse_down = input.is_mouse_button_pressed(MouseButton::Left);
    let surface_width = gpu_window.gpu.config.width as f32;
    let surface_height = gpu_window.gpu.config.height as f32;
    if let (Some(ui), Some(ui_renderer)) =
        (gpu_window.ui_context.as_mut(), gpu_window.ui_renderer.as_mut())
    {
        ui.begin_frame(mouse_x as f32, mouse_y as f32, mouse_down);
        hud_fn(ui, world);
        ui.end_frame();
        ui_renderer.render(
            &gpu_window.gpu.device,
            &gpu_window.gpu.queue,
            encoder,
            view,
            &ui.draw_list,
            surface_width,
            surface_height,
        );
    }
}
