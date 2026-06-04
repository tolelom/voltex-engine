use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::{GpuContext, LightUniform, MaterialUniform, Mesh, MeshVertex};
use voltex_renderer_ecs::{BasicRenderer, MeshPath, RenderAssets, TextureHandle, TexturePath};
use voltex_asset::FileWatcher;
use voltex_audio::{AudioClip, AudioSource, AudioSystem};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
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
use voltex_math::{Quat, Vec3, AABB};
use voltex_physics::{physics_step, Collider, PhysicsConfig, RigidBody};

/// Scene-persisted audio clip file path. The canonical type lives in
/// `voltex_runtime::audio_paths::AudioPath` (the demo doesn't link the
/// runtime); the wire contract is just the NAME + UTF-8 bytes, which the
/// shared `path_component!` macro pins on both sides.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AudioPath(pub String);

voltex_ecs::path_component!(AudioPath, "audio_path");

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
    // Last asset-pipeline action result, shown in the toolbar.
    status: String,
    // Hot reload: watch opened files and re-apply on disk change.
    watcher: FileWatcher,
    watched_textures: HashMap<PathBuf, TextureHandle>,
    watched_scene: Option<PathBuf>,
    watched_meshes: HashMap<PathBuf, MeshHandle>,
    watched_audio: HashMap<PathBuf, usize>,
    // Directory the asset browser is showing — watched so external file
    // adds/removes auto-refresh the listing (dir mtime moves on entry change).
    watched_browser_dir: Option<PathBuf>,
    // Selected-texture preview: path → UiRenderer texture slot (None = decode
    // failed; cached so broken files don't re-decode every frame).
    thumb_cache: HashMap<PathBuf, Option<u32>>,
    // Audio preview for browser-opened clips (starts empty; clips hot-loaded).
    audio: AudioSystem,
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

/// Local-space AABB over a vertex position iterator (assumes at least one —
/// both call sites guard against empty vertex lists).
fn aabb_from_positions(positions: impl Iterator<Item = [f32; 3]>) -> AABB {
    let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for [x, y, z] in positions {
        min = Vec3::new(min.x.min(x), min.y.min(y), min.z.min(z));
        max = Vec3::new(max.x.max(x), max.y.max(y), max.z.max(z));
    }
    AABB::new(min, max)
}

/// Decode a mesh file (.obj / .gltf / .glb) and upload it as a GPU mesh,
/// also returning its local-space AABB (for the entity's `Bounds`).
fn load_mesh_from_path(device: &wgpu::Device, path: &Path) -> Result<(Mesh, AABB), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("obj") => {
            let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let data = voltex_renderer::obj::parse_obj(&source);
            if data.vertices.is_empty() {
                return Err("OBJ has no vertices".to_string());
            }
            let aabb = aabb_from_positions(data.vertices.iter().map(|v| v.position));
            Ok((Mesh::new(device, &data.vertices, &data.indices), aabb))
        }
        Some("gltf" | "glb") => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let data = voltex_renderer::parse_gltf(&bytes).map_err(|e| e.to_string())?;
            let prim = data.meshes.first().ok_or("glTF has no meshes")?;
            if prim.vertices.is_empty() {
                return Err("glTF mesh has no vertices".to_string());
            }
            let aabb = aabb_from_positions(prim.vertices.iter().map(|v| v.position));
            Ok((Mesh::new(device, &prim.vertices, &prim.indices), aabb))
        }
        _ => Err(format!("unsupported mesh extension: {}", path.display())),
    }
}

/// Decode an audio file (.wav / .ogg) into an [`AudioClip`]. Thin wrapper over
/// [`voltex_audio::load_clip_file`] mapping the error into the demo's String.
fn load_clip_from_path(path: &Path) -> Result<AudioClip, String> {
    voltex_audio::load_clip_file(path).map_err(|e| e.to_string())
}

/// Re-attach `TextureHandle`s after a scene load: every entity carrying a
/// persisted [`TexturePath`] gets its file re-read, uploaded, and watched for
/// hot-reload. Handles are runtime-only, so this is what makes applied
/// textures survive Save → Load. Returns how many entities were resolved;
/// failures log and skip (the entity renders with the white default).
fn resolve_texture_paths(
    world: &mut voltex_ecs::World,
    render_assets: &mut RenderAssets,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    watcher: &mut FileWatcher,
    watched_textures: &mut HashMap<PathBuf, TextureHandle>,
) -> usize {
    let targets: Vec<(Entity, PathBuf)> = world
        .query::<TexturePath>()
        .map(|(e, p)| (e, PathBuf::from(&p.0)))
        .collect();
    let mut resolved = 0;
    for (entity, path) in targets {
        // Reuse the handle if this file was already loaded this run (several
        // entities sharing one texture must share one handle).
        let handle = match watched_textures.get(&path) {
            Some(&h) => Some(h),
            None => match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    render_assets
                        .add_texture_png(device, queue, &bytes)
                        .map_err(|e| e.to_string())
                }) {
                Ok(h) => {
                    watched_textures.insert(path.clone(), h);
                    watcher.watch(path.clone());
                    Some(h)
                }
                Err(e) => {
                    eprintln!("[scene] texture {} failed to resolve: {}", path.display(), e);
                    None
                }
            },
        };
        if let Some(h) = handle {
            world.add(entity, h);
            resolved += 1;
        }
    }
    resolved
}

/// Re-attach `MeshHandle`s after a scene load — the mesh counterpart of
/// [`resolve_texture_paths`]. `Bounds` is NOT touched: the scene serialized
/// it, and if the file changed since save it is stale (same v1 limitation as
/// mesh hot-reload). Returns how many entities were resolved.
fn resolve_mesh_paths(
    world: &mut voltex_ecs::World,
    render_assets: &mut RenderAssets,
    device: &wgpu::Device,
    watcher: &mut FileWatcher,
    watched_meshes: &mut HashMap<PathBuf, MeshHandle>,
) -> usize {
    let targets: Vec<(Entity, PathBuf)> = world
        .query::<MeshPath>()
        .map(|(e, p)| (e, PathBuf::from(&p.0)))
        .collect();
    let mut resolved = 0;
    for (entity, path) in targets {
        // Entities sharing one file share one handle.
        let handle = match watched_meshes.get(&path) {
            Some(&h) => Some(h),
            None => match load_mesh_from_path(device, &path) {
                Ok((mesh, _aabb)) => {
                    let h = render_assets.add_mesh(mesh);
                    watched_meshes.insert(path.clone(), h);
                    watcher.watch(path.clone());
                    Some(h)
                }
                Err(e) => {
                    eprintln!("[scene] mesh {} failed to resolve: {}", path.display(), e);
                    None
                }
            },
        };
        if let Some(h) = handle {
            world.add(entity, h);
            resolved += 1;
        }
    }
    resolved
}

/// Decode an image file (.png / .jpg) and upload it for UI preview, returning
/// the [`UiRenderer`] texture slot for [`asset_browser_panel`]'s thumbnail.
fn make_thumbnail(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    ui_renderer: &mut UiRenderer,
    path: &Path,
) -> Result<u32, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let (pixels, w, h) = match ext.as_deref() {
        Some("png") => voltex_renderer::parse_png(&bytes).map_err(|e| e.to_string())?,
        Some("jpg" | "jpeg") => voltex_renderer::parse_jpg(&bytes).map_err(|e| e.to_string())?,
        _ => return Err(format!("unsupported image extension: {}", path.display())),
    };
    let layout = voltex_renderer::GpuTexture::bind_group_layout(device);
    let tex = voltex_renderer::GpuTexture::from_rgba(device, queue, w, h, &pixels, &layout);
    Ok(ui_renderer.register_texture(device, &tex.view))
}

/// Cached thumbnail slot for `path`. Cache entries (success *and* failure)
/// are free; a miss decodes + registers only while `decode_budget > 0`
/// (decrementing it), so callers can cap per-frame decode work — the grid
/// view passes 1 to fill big folders over a few frames instead of hitching.
/// New entries are watched so a disk edit invalidates them (v1.0.95).
fn thumb_slot_for(state: &mut AppState, path: &Path, decode_budget: &mut usize) -> Option<u32> {
    if let Some(cached) = state.thumb_cache.get(path) {
        return *cached;
    }
    if *decode_budget == 0 {
        return None;
    }
    *decode_budget -= 1;
    state.watcher.watch(path.to_path_buf());
    let slot = match make_thumbnail(
        &state.gpu.device,
        &state.gpu.queue,
        &mut state.ui_renderer,
        path,
    ) {
        Ok(slot) => Some(slot),
        Err(e) => {
            eprintln!("[thumb] {}: {}", path.display(), e);
            None
        }
    };
    state.thumb_cache.insert(path.to_path_buf(), slot);
    slot
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
        let gpu = GpuContext::new(window.handle.clone()).expect("voltex: GPU initialization failed");

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
        // Persist applied-asset *paths* in scenes (handles are runtime-only;
        // the resolve_* passes re-attach them after every load).
        session.registry.register::<TexturePath>();
        session.registry.register::<MeshPath>();
        session.registry.register::<AudioPath>();

        let seed = |session: &mut EditorSession, pos: Vec3, tag: &str| -> Entity {
            let e = session.dispatch(EditorCommand::Spawn).unwrap();
            session.dispatch(EditorCommand::SetTransform {
                entity: e,
                position: pos,
                rotation: Quat::IDENTITY,
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
            status: String::new(),
            watcher: FileWatcher::new(Duration::from_millis(300)),
            watched_textures: HashMap::new(),
            watched_scene: None,
            watched_meshes: HashMap::new(),
            watched_audio: HashMap::new(),
            watched_browser_dir: None,
            thumb_cache: HashMap::new(),
            audio: AudioSystem::new(Vec::new())
                .expect("voltex: audio thread spawn failed"),
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
                    state.ui.same_line();
                    if state.ui.button("Spawn") {
                        if let Ok(e) = state.session.dispatch(EditorCommand::Spawn) {
                            let _ = state.session.dispatch(EditorCommand::SetTransform {
                                entity: e,
                                position: Vec3::new(0.0, 0.5, 0.0),
                                rotation: Quat::IDENTITY,
                                scale: Vec3::new(1.0, 1.0, 1.0),
                            });
                            state.session.world.add(e, MeshHandle(state.cube_mesh_h.0));
                            state.session.world.add(e, MaterialHandle(state.default_mat_h.0));
                            state.session.selected = Some(e);
                        }
                    }
                    state.ui.same_line();
                    if state.ui.button("Save") {
                        let path = state.session.scene_path.clone();
                        let _ = state.session.dispatch(EditorCommand::SaveScene { path });
                    }
                    state.ui.same_line();
                    if state.ui.button("Load") {
                        let path = state.session.scene_path.clone();
                        let _ = state.session.dispatch(EditorCommand::LoadScene { path });
                        state.session.selected = None;
                    }
                    state.ui.same_line();
                    if state.ui.button("Undo") { state.session.undo(); }
                    state.ui.same_line();
                    if state.ui.button("Redo") { state.session.redo(); }
                    state.ui.same_line();
                    if state.ui.button("Move") { state.gizmo_state.set_mode(GizmoMode::Translate); }
                    state.ui.same_line();
                    if state.ui.button("Rotate") { state.gizmo_state.set_mode(GizmoMode::Rotate); }
                    state.ui.same_line();
                    if state.ui.button("Scale") { state.gizmo_state.set_mode(GizmoMode::Scale); }
                }
                // Asset-pipeline status (set when a file is opened from the browser).
                if !state.status.is_empty() {
                    state.ui.same_line();
                    state.ui.text(&state.status);
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
                            // Selected texture → UI thumbnail slot. Decoded and
                            // registered once per path; failures cache None so a
                            // broken file doesn't re-decode every frame.
                            let mut preview = None;
                            if state.asset_browser.selected_kind()
                                == Some(voltex_editor::AssetKind::Texture)
                            {
                                if let Some(path) = state.asset_browser.selected_full_path() {
                                    preview = thumb_slot_for(state, &path, &mut 1);
                                }
                            }
                            // Grid view: thumbnail slots for every texture file
                            // in the current directory. At most one NEW decode
                            // per frame so a big folder fills over a few frames
                            // instead of hitching one (cached entries are free).
                            let mut grid_thumbs: HashMap<String, u32> = HashMap::new();
                            if state.asset_browser.view == voltex_editor::BrowserView::Grid {
                                let texture_files: Vec<String> = state
                                    .asset_browser
                                    .entries
                                    .iter()
                                    .filter(|e| {
                                        !e.is_dir && e.kind == voltex_editor::AssetKind::Texture
                                    })
                                    .map(|e| e.name.clone())
                                    .collect();
                                let mut decode_budget = 1usize;
                                for file_name in texture_files {
                                    let path = state.asset_browser.current.join(&file_name);
                                    if let Some(slot) =
                                        thumb_slot_for(state, &path, &mut decode_budget)
                                    {
                                        grid_thumbs.insert(file_name, slot);
                                    }
                                }
                            }
                            asset_browser_panel(
                                &mut state.ui,
                                &mut state.asset_browser,
                                rect,
                                preview,
                                Some(&grid_thumbs),
                            );
                        }
                        _ => {}
                    }
                }

                state.ui.end_frame();

                // ── Browser directory watch: follow navigation ──
                // Watching the directory's mtime detects external file
                // adds/removes (not content edits — per-file watches do that).
                if state.watched_browser_dir.as_ref() != Some(&state.asset_browser.current) {
                    if let Some(old) = state.watched_browser_dir.take() {
                        state.watcher.unwatch(&old);
                    }
                    let dir = state.asset_browser.current.clone();
                    state.watcher.watch(dir.clone());
                    state.watched_browser_dir = Some(dir);
                }

                // ── Asset pipeline: consume a browser "Open" activation ──
                // The editor crate is GPU-free, so the actual load/apply happens
                // here in the integration layer (it owns gpu + render_assets).
                if let Some((abs_path, kind)) = state.asset_browser.take_activated() {
                    // Canonical form for everything downstream — watch-map
                    // keys, persisted TexturePath/MeshPath, fs reads (relative
                    // to CWD = browser root) — so scenes stay portable and a
                    // file is never watched under two spellings.
                    let rel = state.asset_browser.relative_to_root(&abs_path);
                    let path = PathBuf::from(&rel);
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    match kind {
                        voltex_editor::AssetKind::Scene => {
                            let path_str = path.to_string_lossy().into_owned();
                            match state.session.dispatch(EditorCommand::LoadScene { path: path_str }) {
                                Ok(_) => {
                                    state.session.selected = None;
                                    // Persisted asset paths → fresh handles.
                                    let tex_n = resolve_texture_paths(
                                        &mut state.session.world,
                                        &mut state.render_assets,
                                        &state.gpu.device,
                                        &state.gpu.queue,
                                        &mut state.watcher,
                                        &mut state.watched_textures,
                                    );
                                    let mesh_n = resolve_mesh_paths(
                                        &mut state.session.world,
                                        &mut state.render_assets,
                                        &state.gpu.device,
                                        &mut state.watcher,
                                        &mut state.watched_meshes,
                                    );
                                    state.status = if tex_n + mesh_n > 0 {
                                        format!(
                                            "Loaded scene: {} ({} textures, {} meshes)",
                                            name, tex_n, mesh_n
                                        )
                                    } else {
                                        format!("Loaded scene: {}", name)
                                    };
                                    // Watch for live edits — only one scene at a time.
                                    if let Some(old) = state.watched_scene.replace(path.clone()) {
                                        if old != path { state.watcher.unwatch(&old); }
                                    }
                                    state.watcher.watch(path.clone());
                                }
                                Err(e) => state.status = format!("Load failed: {}", e),
                            }
                        }
                        voltex_editor::AssetKind::Texture => {
                            match std::fs::read(&path) {
                                Ok(bytes) => match state.render_assets.add_texture_png(
                                    &state.gpu.device,
                                    &state.gpu.queue,
                                    &bytes,
                                ) {
                                    Ok(handle) => {
                                        if let Some(sel) = state.session.selected {
                                            state.session.world.add(sel, handle);
                                            // Persist the *path* — handles are
                                            // runtime-only and die with the run.
                                            state.session.world.add(sel, TexturePath(rel.clone()));
                                            state.status = format!("Applied texture: {}", name);
                                            // Watch this file → hot-reload onto the same handle.
                                            state.watched_textures.insert(path.clone(), handle);
                                            state.watcher.watch(path.clone());
                                        } else {
                                            state.status = "Select an entity first".to_string();
                                        }
                                    }
                                    Err(e) => state.status = format!("Texture decode failed: {}", e),
                                },
                                Err(e) => state.status = format!("Read failed: {}", e),
                            }
                        }
                        voltex_editor::AssetKind::Mesh => {
                            if let Some(sel) = state.session.selected {
                                // Re-Open of a watched file replaces the mesh in
                                // place (no slot leak); first Open registers anew.
                                let result = load_mesh_from_path(&state.gpu.device, &path)
                                    .and_then(|(mesh, aabb)| {
                                        match state.watched_meshes.get(&path).copied() {
                                            Some(h) => state
                                                .render_assets
                                                .replace_mesh(h, mesh)
                                                .map(|_| (h, aabb))
                                                .map_err(|e| e.to_string()),
                                            None => Ok((state.render_assets.add_mesh(mesh), aabb)),
                                        }
                                    });
                                match result {
                                    Ok((handle, aabb)) => {
                                        state.session.world.add(sel, handle);
                                        state.session.world.add(sel, Bounds(aabb));
                                        // Persist the *path* — handles are
                                        // runtime-only and die with the run.
                                        state.session.world.add(sel, MeshPath(rel.clone()));
                                        state.status = format!("Applied mesh: {}", name);
                                        // Watch this file → hot-reload onto the same handle.
                                        state.watched_meshes.insert(path.clone(), handle);
                                        state.watcher.watch(path.clone());
                                    }
                                    Err(e) => state.status = format!("Mesh load failed: {}", e),
                                }
                            } else {
                                state.status = "Select an entity first".to_string();
                            }
                        }
                        voltex_editor::AssetKind::Audio => {
                            match load_clip_from_path(&path) {
                                Ok(clip) => {
                                    // Re-Open of a watched file replaces its clip
                                    // in place; first Open hot-loads a new one.
                                    let idx = match state.watched_audio.get(&path) {
                                        Some(&i) => {
                                            state.audio.replace_clip(i, clip);
                                            i
                                        }
                                        None => {
                                            let i = state.audio.add_clip(clip);
                                            state.watched_audio.insert(path.clone(), i);
                                            state.watcher.watch(path.clone());
                                            i
                                        }
                                    };
                                    state.audio.play(idx, 1.0, false);
                                    // With a selection, also apply to the entity —
                                    // persist the *path* (the clip index is
                                    // runtime-only; the game runtime re-resolves
                                    // AudioPath after scene load). No selection →
                                    // preview-only, the pre-v1.0.103 behavior.
                                    if let Some(sel) = state.session.selected {
                                        state
                                            .session
                                            .world
                                            .add(sel, AudioSource::new(idx as u32));
                                        state.session.world.add(sel, AudioPath(rel.clone()));
                                        state.status = format!("Applied audio: {}", name);
                                    } else {
                                        state.status = format!("Playing audio: {}", name);
                                    }
                                }
                                Err(e) => state.status = format!("Audio decode failed: {}", e),
                            }
                        }
                        voltex_editor::AssetKind::Other => {
                            state.status = "Other import not yet wired".to_string();
                        }
                    }
                    eprintln!("[asset] {}", state.status);
                }

                // ── Hot reload: re-apply opened files that changed on disk ──
                for changed in state.watcher.poll_changes() {
                    let name = changed
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    // Thumbnail invalidation runs independent of the branches
                    // below — an Opened texture can also be the previewed one.
                    // Next frame's selection lookup re-decodes + re-registers
                    // (old slot stays per the permanent-slot contract).
                    state.thumb_cache.remove(&changed);
                    if state.watched_scene.as_deref() == Some(changed.as_path()) {
                        let path_str = changed.to_string_lossy().into_owned();
                        match state.session.dispatch(EditorCommand::LoadScene { path: path_str }) {
                            Ok(_) => {
                                state.session.selected = None;
                                let tex_n = resolve_texture_paths(
                                    &mut state.session.world,
                                    &mut state.render_assets,
                                    &state.gpu.device,
                                    &state.gpu.queue,
                                    &mut state.watcher,
                                    &mut state.watched_textures,
                                );
                                let mesh_n = resolve_mesh_paths(
                                    &mut state.session.world,
                                    &mut state.render_assets,
                                    &state.gpu.device,
                                    &mut state.watcher,
                                    &mut state.watched_meshes,
                                );
                                state.status = if tex_n + mesh_n > 0 {
                                    format!(
                                        "Reloaded scene: {} ({} textures, {} meshes)",
                                        name, tex_n, mesh_n
                                    )
                                } else {
                                    format!("Reloaded scene: {}", name)
                                };
                            }
                            Err(e) => state.status = format!("Scene reload failed: {}", e),
                        }
                    } else if let Some(&handle) = state.watched_textures.get(&changed) {
                        match std::fs::read(&changed) {
                            Ok(bytes) => match state.render_assets.replace_texture_png(
                                &state.gpu.device,
                                &state.gpu.queue,
                                handle,
                                &bytes,
                            ) {
                                Ok(()) => state.status = format!("Reloaded texture: {}", name),
                                Err(e) => state.status = format!("Texture reload failed: {}", e),
                            },
                            Err(e) => state.status = format!("Reread failed: {}", e),
                        }
                    } else if let Some(&handle) = state.watched_meshes.get(&changed) {
                        // Bounds is NOT refreshed on reload — the handle may be
                        // shared by entities this frame doesn't track (v1 limit).
                        match load_mesh_from_path(&state.gpu.device, &changed) {
                            Ok((mesh, _aabb)) => {
                                match state.render_assets.replace_mesh(handle, mesh) {
                                    Ok(()) => state.status = format!("Reloaded mesh: {}", name),
                                    Err(e) => state.status = format!("Mesh reload failed: {}", e),
                                }
                            }
                            Err(e) => state.status = format!("Mesh reload failed: {}", e),
                        }
                    } else if let Some(&idx) = state.watched_audio.get(&changed) {
                        match load_clip_from_path(&changed) {
                            Ok(clip) => {
                                state.audio.replace_clip(idx, clip);
                                // Re-preview so the change is audible immediately.
                                state.audio.play(idx, 1.0, false);
                                state.status = format!("Reloaded audio: {}", name);
                            }
                            Err(e) => state.status = format!("Audio reload failed: {}", e),
                        }
                    } else if state.watched_browser_dir.as_deref() == Some(changed.as_path()) {
                        // Entry added/removed in the visible directory →
                        // re-list. refresh() clears selection/activation by
                        // design (the old selection may no longer exist).
                        state.asset_browser.refresh();
                        state.status = format!("Refreshed folder: {}", name);
                    }
                    eprintln!("[hot-reload] {}", state.status);
                }

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
