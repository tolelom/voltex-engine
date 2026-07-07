//! `RuntimeApp` — owns every subsystem (World, registry, renderer, script
//! pool, audio bridge, etc.) and exposes a single `run()` entry point that
//! drives the winit event loop until the window closes.
//!
//! ## Drop order
//!
//! Field declaration order below is the actual drop order Rust uses
//! (top-down). It is deliberately:
//!
//! 1. `runtime_state` (GPU + window + renderer) — must release wgpu resources
//!    before the device they belong to is dropped.
//! 2. `user_systems` — closures that may capture handles into the World.
//! 3. `file_watcher`, `timer`, `input`, `physics_config` — pure data.
//! 4. `script_pool` — LuaState holds a raw `*mut World`; pool must drop
//!    before the World it points at.
//! 5. `assets`, `registry`, `world` — destroyed last.
//! 6. `config` — last, no resource cleanup.

use std::time::Duration;

use voltex_asset::FileWatcher;
use voltex_audio::AudioSystem;
use voltex_ecs::{propagate_transforms, ComponentRegistry, Entity, World};
use voltex_physics::{detect_collisions, ContactPoint, PhysicsConfig};
use crate::time::Time;
use voltex_platform::{GameTimer, InputMap, InputState};
use voltex_editor::UiContext;
use voltex_renderer::{Camera, LightUniform, PointLight};
use voltex_renderer_ecs::RenderAssets;
use voltex_script::ScriptInstancePool;

use crate::config::RuntimeConfig;
use crate::debug_draw::DebugLines;
use crate::error::{RuntimeError, RuntimeResult};

/// Live GPU state initialised in winit's `resumed` callback. Held in an
/// `Option<RuntimeState>` so the rest of `RuntimeApp` can exist (and be unit
/// tested) before any window is open.
pub(crate) struct RuntimeState {
    pub window: voltex_platform::VoltexWindow,
    pub gpu: voltex_renderer::GpuContext,
    pub renderer: voltex_renderer_ecs::BasicRenderer,
    pub camera: voltex_renderer::Camera,
    pub fps_controller: voltex_renderer::FpsController,
    pub light: voltex_renderer::LightUniform,
    /// Immediate-mode UI context + GPU renderer for the in-window HUD overlay.
    /// `Some` only when `config.enable_hud` was set (constructed on GPU init).
    /// Mirrors `renderer`'s placement — wgpu resource handles keep the device
    /// alive internally, so trailing-field drop order is safe.
    pub ui_context: Option<UiContext>,
    pub ui_renderer: Option<voltex_editor::UiRenderer>,
}

type UserSystem = Box<dyn FnMut(&mut World, f32) + 'static>;
/// User system that also gets read-only access to the per-frame InputState.
/// Added in D3 so a flagship game can drive entities from keyboard / mouse
/// without requiring a full `InputState as ECS resource` abstraction (v1.1).
type InputSystem = Box<dyn FnMut(&mut World, &InputState, f32) + 'static>;
/// User system that reads semantic actions/axes via the [`InputMap`] layer in
/// addition to the raw [`InputState`]. Lets a game rebind controls in one place
/// (`app.input_map`) and drive entities by action name.
type ActionSystem = Box<dyn FnMut(&mut World, &InputState, &InputMap, f32) + 'static>;
/// User system that receives the runtime [`Time`] (accumulated elapsed seconds,
/// per-frame delta, frame counter) instead of just a bare `dt`. For timers,
/// cooldowns, and animation phase that need wallclock-since-start.
type TimedSystem = Box<dyn FnMut(&mut World, &Time) + 'static>;
/// HUD callback invoked each frame between the 3D pass and present, with the
/// immediate-mode [`UiContext`] (already inside `begin_frame`/`end_frame`) and
/// read-only access to the World. Draw text / bars / panels here.
type HudFn = Box<dyn FnMut(&mut UiContext, &World) + 'static>;
/// Per-frame camera updater (see [`RuntimeApp::set_camera_fn`]). Runs after the
/// systems tick, with the live [`Camera`] and read-only World, so games can
/// implement follow / orbit / cinematic cameras. Overrides the default RMB
/// fly-camera for the frame.
type CameraFn = Box<dyn FnMut(&mut Camera, &World, f32) + 'static>;
/// Per-frame directional-light updater (see [`RuntimeApp::set_light_fn`]). Runs
/// after the systems tick with the live [`LightUniform`] and read-only World,
/// for day/night cycles, flicker, etc.
type LightFn = Box<dyn FnMut(&mut LightUniform, &World, f32) + 'static>;
/// Per-frame point/spot-light updater (see [`RuntimeApp::set_point_lights_fn`]).
/// Runs after the systems tick with the live point-light `Vec` and read-only
/// World, for moving torches, flickering lamps, projectile glows, etc. The
/// resulting set is uploaded to the renderer (capped at `MAX_LIGHTS`) the same
/// way a static [`RuntimeApp::set_point_lights`] call would be.
type PointLightsFn = Box<dyn FnMut(&mut Vec<PointLight>, &World, f32) + 'static>;
/// Per-frame collision callback (see [`RuntimeApp::set_collision_fn`]). Receives
/// the frame's contact pairs (between entities carrying a
/// [`Collider`](voltex_physics::Collider) and a Transform) plus the mutable
/// World, for trigger/hit reactions. Runs regardless of `enable_physics`, so it
/// doubles as a trigger-zone detector.
type CollisionFn = Box<dyn FnMut(&[ContactPoint], &mut World, f32) + 'static>;
/// Per-frame debug-draw callback (see [`RuntimeApp::set_debug_draw_fn`]). The
/// passed [`DebugLines`] is cleared each frame before the call; whatever the
/// game queues is rendered as `LineList` gizmos over the 3D scene.
type DebugFn = Box<dyn FnMut(&mut DebugLines, &World, f32) + 'static>;

/// The runtime's top-level application object. Owns the ECS world, asset
/// stores, schedulers, and the main loop integration with `winit`/`wgpu`.
pub struct RuntimeApp {
    pub(crate) runtime_state: Option<RuntimeState>,
    user_systems: Vec<UserSystem>,
    input_systems: Vec<InputSystem>,
    /// Optional per-frame HUD draw callback (see [`Self::set_hud_fn`]). Only
    /// invoked when `config.enable_hud` is set and the GPU UI was constructed.
    pub(crate) hud_fn: Option<HudFn>,
    /// Optional per-frame camera updater (see [`Self::set_camera_fn`]).
    pub(crate) camera_fn: Option<CameraFn>,
    /// Optional per-frame directional-light updater (see [`Self::set_light_fn`]).
    pub(crate) light_fn: Option<LightFn>,
    /// Optional per-frame collision/trigger callback (see [`Self::set_collision_fn`]).
    collision_fn: Option<CollisionFn>,
    /// Optional per-frame debug-draw callback (see [`Self::set_debug_draw_fn`]).
    pub(crate) debug_fn: Option<DebugFn>,
    /// Reused debug-line buffer (cleared + refilled by `debug_fn` each frame).
    pub(crate) debug_lines: DebugLines,
    /// Additive point lights uploaded to the renderer each frame.
    pub(crate) point_lights: Vec<PointLight>,
    /// Optional per-frame point/spot-light updater (see [`Self::set_point_lights_fn`]).
    pub(crate) point_lights_fn: Option<PointLightsFn>,
    /// Polling file watcher used for script / scene hot reload.
    pub file_watcher: FileWatcher,
    /// Fixed-step timer driving update / render dt.
    pub timer: GameTimer,
    /// Aggregated input state (keyboard + mouse + scroll), refreshed each frame.
    pub input: InputState,
    /// Semantic action/axis bindings (see [`Self::add_action_system`]). Empty
    /// by default; populate via [`RuntimeConfig::with_input_map`] or by mutating
    /// this field directly before `run`.
    pub input_map: InputMap,
    /// User systems that read resolved actions (world + input + input_map + dt).
    action_systems: Vec<ActionSystem>,
    /// Accumulated frame clock (elapsed seconds, delta, frame count). Advanced
    /// once per `tick_systems_only`; read by [`Self::add_timed_system`] systems.
    pub time: Time,
    /// User systems that read the [`Time`] clock.
    timed_systems: Vec<TimedSystem>,
    /// Physics tuning knobs (gravity, fixed_dt, solver iterations).
    pub physics_config: PhysicsConfig,
    /// Pool of loaded Lua interpreters keyed by script path.
    pub script_pool: ScriptInstancePool,
    /// Per-frame input snapshot published to Lua scripts (`input` table).
    /// Re-filled from [`Self::input`] before scripts run each frame.
    pub(crate) script_input: voltex_script::ScriptInput,
    /// Audio mixer, present only when `config.enable_audio` and init succeeded.
    /// Driven each frame by `audio_sync_system` from `AudioSource` components.
    pub audio: Option<AudioSystem>,
    /// Entity → mixer voice map maintained by `audio_sync_system`; stops
    /// orphan voices when their entity despawns without a stop edge.
    audio_voices: crate::audio_sync::AudioVoiceTracker,
    /// Resolved [`RuntimeConfig::asset_root`] (exe-dir anchored at boot);
    /// relative asset paths load from here instead of the cwd.
    pub(crate) asset_root: Option<std::path::PathBuf>,
    /// Path → handle cache for scene-persisted asset paths
    /// (`TexturePath`/`MeshPath`); see [`crate::asset_paths`].
    pub(crate) asset_path_cache: crate::asset_paths::AssetPathCache,
    /// Path → clip id cache for scene-persisted audio paths (`AudioPath`);
    /// see [`crate::audio_paths`].
    pub(crate) audio_clip_cache: crate::audio_paths::AudioClipCache,
    /// Watches audio files registered via [`Self::watch_audio_clip`]; separate
    /// from `file_watcher` because the script pool drains that one's changes.
    audio_watcher: FileWatcher,
    /// Path → clip index map driving audio hot-reload (see `watch_audio_clip`).
    watched_audio_clips: std::collections::HashMap<std::path::PathBuf, usize>,
    /// Entity whose `Transform` positions the 3D audio listener (camera/player).
    /// `None` leaves the listener unchanged.
    pub audio_listener: Option<Entity>,
    /// GPU-resident mesh + material storage shared with the renderer.
    pub assets: RenderAssets,
    /// Component registry used for scene (de)serialisation.
    pub registry: ComponentRegistry,
    /// The ECS world the runtime ticks each frame.
    pub world: World,
    /// HUD primitive — rolling FPS counter + user-controlled title segment,
    /// both surfaced through the OS window title. See [`crate::Hud`].
    pub hud: crate::Hud,
    /// The configuration passed to [`RuntimeApp::new`].
    pub config: RuntimeConfig,
}

impl RuntimeApp {
    /// Pre-window setup. Runs `register_components`, optionally loads the
    /// initial scene, then runs `build_scene` and `setup_systems`. Window /
    /// GPU init is deferred to the winit `resumed` callback inside `run()`.
    ///
    /// Side-effects (v1.0.18): when `config.enable_crash_log` is true
    /// (default), installs the panic hook and the env logger **here** —
    /// not in [`Self::run`] — so that panics during `build_scene` /
    /// `setup_systems` (which run inside this constructor) still produce
    /// a crash report. Both are `OnceLock`-guarded, so a later
    /// `RuntimeApp::run` call is a no-op.
    pub fn new(mut config: RuntimeConfig) -> RuntimeResult<Self> {
        if config.enable_crash_log {
            crate::crash::install_panic_hook(
                config.window.title.clone(),
                config.crash_log_dir.clone(),
            );
        }
        crate::log::init_from_env();
        crate::log_info!(
            "voltex_runtime::new: title={:?} size={}x{}",
            config.window.title,
            config.window.width,
            config.window.height
        );

        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        // AudioSource lives in voltex_audio (no ECS dep), so it is registered
        // here via the register_raw escape hatch rather than register_defaults.
        crate::audio_sync::register_audio_source(&mut registry);
        // AudioPath (scene-persisted clip file path) is runtime-owned.
        registry.register::<crate::audio_paths::AudioPath>();
        // Declarative renderables (PrimitiveMesh / MaterialDesc) live in
        // voltex_renderer_ecs (L3), so they register here rather than in the
        // L1 register_defaults. Materialized to GPU resources in gpu_init.
        voltex_renderer_ecs::register_renderable_components(&mut registry);
        if let Some(setup) = config.register_components.take() {
            setup(&mut registry);
        }

        let mut world = World::new();
        if let Some(scene_path) = config.initial_scene.as_ref() {
            // A broken initial scene degrades to an empty world (the scene
            // builder below may still populate it), but never silently — the
            // dist-exe E2E reads these lines as its load evidence.
            match load_scene_via_vfs_or_disk(
                &mut world,
                &registry,
                scene_path,
                config.vfs.as_ref(),
            ) {
                Ok(()) => crate::log_info!(
                    "voltex_runtime: initial scene `{}` loaded ({} entities)",
                    scene_path,
                    world.entity_count()
                ),
                Err(e) => crate::log_error!(
                    "voltex_runtime: initial scene `{}` failed to load: {e}",
                    scene_path
                ),
            }
        }
        let scene_builder = config.build_scene.take();
        if let Some(build) = scene_builder {
            build(&mut world, &mut registry);
        }

        // Resolve the configured asset root once (exe-dir anchoring involves a
        // syscall); every relative asset read below and per-frame goes through it.
        let asset_root = config.resolved_asset_root();

        // Build the audio mixer up front (it only spawns a thread, no GPU /
        // window needed). Failure degrades to silent rather than aborting boot.
        // `with_audio_files` paths are decoded here, appended after the
        // pre-loaded clips (ClipId = clip count + file position), and queued
        // for hot-reload watching (registered once the app struct exists).
        let mut audio_watch_pairs: Vec<(std::path::PathBuf, usize)> = Vec::new();
        let audio = if config.enable_audio {
            let mut clips = std::mem::take(&mut config.audio_clips);
            for (i, path) in std::mem::take(&mut config.audio_files).into_iter().enumerate() {
                let index = clips.len();
                // Relative paths anchor at the asset root (when configured and
                // the rooted file exists) — same contract as read_asset.
                let path = match asset_root.as_deref() {
                    Some(root) if path.is_relative() && root.join(&path).is_file() => {
                        root.join(&path)
                    }
                    _ => path,
                };
                match voltex_audio::load_clip_file(&path) {
                    Ok(clip) => clips.push(clip),
                    Err(e) => {
                        // Silent placeholder keeps every later file's ClipId
                        // stable; fixing the file hot-reloads it in place.
                        crate::log_error!(
                            "voltex_runtime: audio file {} failed to decode ({e}); \
                             clip {} is a silent placeholder until the file is fixed",
                            path.display(),
                            i
                        );
                        clips.push(voltex_audio::AudioClip::new(vec![0.0], 44_100, 1));
                    }
                }
                audio_watch_pairs.push((path, index));
            }
            match AudioSystem::new(clips) {
                Ok(a) => Some(a),
                Err(e) => {
                    crate::log_error!("voltex_runtime: audio init failed: {e}");
                    None
                }
            }
        } else {
            None
        };

        let hud = crate::Hud::new(config.window.title.clone());
        let mut app = Self {
            runtime_state: None,
            user_systems: Vec::new(),
            input_systems: Vec::new(),
            hud_fn: None,
            camera_fn: None,
            light_fn: None,
            collision_fn: None,
            debug_fn: None,
            debug_lines: DebugLines::new(),
            point_lights: Vec::new(),
            point_lights_fn: None,
            file_watcher: FileWatcher::new(Duration::from_millis(500)),
            timer: GameTimer::new(60),
            input: InputState::new(),
            input_map: config.input_map.take().unwrap_or_default(),
            action_systems: Vec::new(),
            time: Time::new(),
            timed_systems: Vec::new(),
            physics_config: PhysicsConfig::default(),
            script_pool: ScriptInstancePool::new(),
            script_input: voltex_script::ScriptInput::new(),
            audio,
            audio_voices: crate::audio_sync::AudioVoiceTracker::default(),
            asset_root,
            asset_path_cache: crate::asset_paths::AssetPathCache::new(),
            audio_clip_cache: crate::audio_paths::AudioClipCache::new(),
            audio_watcher: FileWatcher::new(Duration::from_millis(300)),
            watched_audio_clips: std::collections::HashMap::new(),
            audio_listener: None,
            assets: RenderAssets::new(),
            registry,
            world,
            hud,
            config,
        };
        // Register hot-reload watches for `with_audio_files` paths (decode
        // already happened above; this only wires the file watcher).
        for (path, index) in audio_watch_pairs {
            app.watch_audio_clip(path, index);
        }
        let system_setup = app.config.setup_systems.take();
        if let Some(setup) = system_setup {
            setup(&mut app);
        }
        Ok(app)
    }

    /// Register a per-frame user system. Systems run in registration order
    /// after the default systems (transform propagation, physics, scripts).
    pub fn add_system<F>(&mut self, system: F)
    where
        F: FnMut(&mut World, f32) + 'static,
    {
        self.user_systems.push(Box::new(system));
    }

    /// Number of registered user systems (excluding input systems).
    pub fn user_system_count(&self) -> usize {
        self.user_systems.len()
    }

    /// Register a per-frame system that also receives the current
    /// `InputState`. Runs after the regular user systems each frame.
    /// Use for camera control, player input, etc.
    pub fn add_input_system<F>(&mut self, system: F)
    where
        F: FnMut(&mut World, &InputState, f32) + 'static,
    {
        self.input_systems.push(Box::new(system));
    }

    /// Number of registered input systems.
    pub fn input_system_count(&self) -> usize {
        self.input_systems.len()
    }

    /// Register a per-frame system that receives the [`InputMap`] alongside the
    /// raw [`InputState`], so it can query semantic actions/axes
    /// (`map.is_pressed("jump", input)`, `map.vector("move_x", "move_y", input)`)
    /// instead of hard-coded keycodes. Runs after the raw input systems each
    /// frame. Bind controls via [`RuntimeConfig::with_input_map`] or by mutating
    /// `app.input_map` before `run`.
    pub fn add_action_system<F>(&mut self, system: F)
    where
        F: FnMut(&mut World, &InputState, &InputMap, f32) + 'static,
    {
        self.action_systems.push(Box::new(system));
    }

    /// Number of registered action systems.
    pub fn action_system_count(&self) -> usize {
        self.action_systems.len()
    }

    /// Register a per-frame system that receives the runtime [`Time`] clock
    /// (`time.elapsed_secs`, `time.delta_secs`, `time.frame_count`) so it can
    /// drive timers / cooldowns / animation phase without accumulating dt by
    /// hand. Runs after the action systems each frame.
    pub fn add_timed_system<F>(&mut self, system: F)
    where
        F: FnMut(&mut World, &Time) + 'static,
    {
        self.timed_systems.push(Box::new(system));
    }

    /// Number of registered timed systems.
    pub fn timed_system_count(&self) -> usize {
        self.timed_systems.len()
    }

    /// Register the per-frame HUD draw callback. Requires
    /// [`RuntimeConfig::with_hud(true)`](crate::RuntimeConfig::with_hud); the
    /// closure receives a [`UiContext`] already inside its frame and read-only
    /// access to the World. Replaces any previously set callback.
    ///
    /// ```ignore
    /// app.set_hud_fn(|ui, world| {
    ///     ui.text(&format!("Entities: {}", world.entity_count()));
    /// });
    /// ```
    pub fn set_hud_fn<F>(&mut self, f: F)
    where
        F: FnMut(&mut UiContext, &World) + 'static,
    {
        self.hud_fn = Some(Box::new(f));
    }

    /// `true` if a HUD callback has been registered.
    pub fn has_hud_fn(&self) -> bool {
        self.hud_fn.is_some()
    }

    /// Set the entity whose `Transform` positions the 3D audio listener
    /// (typically the camera or player). Only meaningful when audio is enabled.
    pub fn set_audio_listener(&mut self, entity: Entity) {
        self.audio_listener = Some(entity);
    }

    /// Hot-reload: watch an audio file and swap the clip at `clip_index`
    /// (the index into the `with_audio` clip Vec, i.e. the `ClipId` value)
    /// whenever the file changes on disk. Looping sounds pick the new data up
    /// on the next mix block; one-shots on their next play. Decode failures
    /// are logged and leave the old clip intact. No-op while audio is
    /// disabled (the watch is still registered for when it is).
    pub fn watch_audio_clip(&mut self, path: impl Into<std::path::PathBuf>, clip_index: usize) {
        let path = path.into();
        self.watched_audio_clips.insert(path.clone(), clip_index);
        self.audio_watcher.watch(path);
    }

    /// Register a per-frame camera updater for follow / orbit / cinematic
    /// cameras. It runs each frame after the systems tick with the live
    /// [`Camera`] and read-only World, and overrides the default RMB
    /// fly-camera. Replaces any previously set updater.
    ///
    /// ```ignore
    /// app.set_camera_fn(|cam, world, _dt| {
    ///     if let Some(p) = player_position(world) {
    ///         cam.position = p + Vec3::new(0.0, 8.0, 12.0);
    ///         cam.pitch = -0.5;
    ///     }
    /// });
    /// ```
    pub fn set_camera_fn<F>(&mut self, f: F)
    where
        F: FnMut(&mut Camera, &World, f32) + 'static,
    {
        self.camera_fn = Some(Box::new(f));
    }

    /// `true` if a per-frame camera updater has been registered.
    pub fn has_camera_fn(&self) -> bool {
        self.camera_fn.is_some()
    }

    /// Register a per-frame directional-light updater (day/night, flicker, …).
    /// Runs each frame after the systems tick with the live [`LightUniform`]
    /// and read-only World. Replaces any previously set updater.
    ///
    /// ```ignore
    /// app.set_light_fn(|light, _world, dt| {
    ///     // slowly rotate the sun
    ///     light.direction = rotate_y(light.direction, 0.1 * dt);
    /// });
    /// ```
    pub fn set_light_fn<F>(&mut self, f: F)
    where
        F: FnMut(&mut LightUniform, &World, f32) + 'static,
    {
        self.light_fn = Some(Box::new(f));
    }

    /// `true` if a per-frame light updater has been registered.
    pub fn has_light_fn(&self) -> bool {
        self.light_fn.is_some()
    }

    /// Register a per-frame collision/trigger callback. Each frame the runtime
    /// runs broad+narrow phase over entities with a
    /// [`Collider`](voltex_physics::Collider) + Transform and hands the
    /// resulting contact pairs (plus mutable World) to `f` — react to hits,
    /// implement triggers, despawn on contact, etc. Runs even with physics
    /// disabled (pure trigger detection). Replaces any previous callback.
    ///
    /// ```ignore
    /// app.set_collision_fn(|contacts, world, _dt| {
    ///     for c in contacts { /* react to c.entity_a / c.entity_b */ }
    /// });
    /// ```
    pub fn set_collision_fn<F>(&mut self, f: F)
    where
        F: FnMut(&[ContactPoint], &mut World, f32) + 'static,
    {
        self.collision_fn = Some(Box::new(f));
    }

    /// `true` if a collision callback has been registered.
    pub fn has_collision_fn(&self) -> bool {
        self.collision_fn.is_some()
    }

    /// Register a per-frame debug-draw callback. Each frame the runtime clears
    /// a shared [`DebugLines`] buffer, calls `f` (with the read-only World and
    /// frame dt) to queue gizmo segments, then renders them as depth-tested
    /// `LineList` over the 3D scene. Replaces any previous callback.
    ///
    /// ```ignore
    /// app.set_debug_draw_fn(|d, world, _dt| {
    ///     for (_, t) in world.query::<Transform>() {
    ///         d.cross(t.position, 0.3, [1.0, 1.0, 0.0]); // mark every entity
    ///     }
    /// });
    /// ```
    pub fn set_debug_draw_fn<F>(&mut self, f: F)
    where
        F: FnMut(&mut DebugLines, &World, f32) + 'static,
    {
        self.debug_fn = Some(Box::new(f));
    }

    /// `true` if a debug-draw callback has been registered.
    pub fn has_debug_fn(&self) -> bool {
        self.debug_fn.is_some()
    }

    /// Replace the additive point lights (uploaded to the renderer each frame).
    /// Excess beyond [`MAX_POINT_LIGHTS`](voltex_renderer::MAX_POINT_LIGHTS) are
    /// ignored by the renderer.
    pub fn set_point_lights(&mut self, lights: Vec<PointLight>) {
        self.point_lights = lights;
    }

    /// Register a per-frame point/spot-light updater (moving torches, flickering
    /// lamps, projectile glows, …). Runs each frame after the systems tick with
    /// the live point-light `Vec` and read-only World; whatever it leaves in the
    /// `Vec` is uploaded to the renderer (capped at
    /// [`MAX_POINT_LIGHTS`](voltex_renderer::MAX_POINT_LIGHTS)). Mirrors
    /// [`Self::set_light_fn`] for the directional sun.
    ///
    /// ```no_run
    /// # use voltex_runtime::{RuntimeApp, RuntimeConfig};
    /// # use voltex_renderer::PointLight;
    /// # let mut app = RuntimeApp::new(RuntimeConfig::new()).unwrap();
    /// app.set_point_lights_fn(|lights, _world, dt| {
    ///     // single torch (position, color, range, intensity) that flickers
    ///     lights.clear();
    ///     let mut torch = PointLight::new([0.0, 2.0, 0.0], [1.0, 0.6, 0.2], 3.0, 8.0);
    ///     torch.intensity *= 0.9 + 0.1 * dt.sin();
    ///     lights.push(torch);
    /// });
    /// ```
    pub fn set_point_lights_fn<F>(&mut self, f: F)
    where
        F: FnMut(&mut Vec<PointLight>, &World, f32) + 'static,
    {
        self.point_lights_fn = Some(Box::new(f));
    }

    /// `true` if a per-frame point-light updater has been registered.
    pub fn has_point_lights_fn(&self) -> bool {
        self.point_lights_fn.is_some()
    }

    /// Serialize the current World to a JSON scene file. Only components
    /// registered on the runtime's `registry` (see `register_defaults` +
    /// `with_registry_setup`) are included.
    pub fn save_scene_json(&self, path: &str) -> RuntimeResult<()> {
        let json = voltex_ecs::serialize_scene_json(&self.world, &self.registry);
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Replace the World with a scene loaded from `path` (binary VSCN first,
    /// then JSON; via the configured `Vfs` if any, else disk). Existing
    /// entities are cleared first.
    pub fn load_scene(&mut self, path: &str) -> RuntimeResult<()> {
        self.world.clear();
        // Give previously-failed TexturePath/MeshPath/AudioPath resolves
        // another chance on every scene load (the file may exist by now). The
        // actual resolve happens in the render handler (GPU) / audio tick.
        self.asset_path_cache.clear_failures();
        self.audio_clip_cache.clear_failures();
        load_scene_via_vfs_or_disk(
            &mut self.world,
            &self.registry,
            path,
            self.config.vfs.as_ref(),
        )
    }

    /// Run one frame's worth of non-rendering systems. Used internally by the
    /// winit handler and directly by unit tests / headless harnesses that
    /// don't need a window.
    pub fn tick_systems_only(&mut self, dt: f32) {
        // Advance the frame clock first so every system this frame (and the HUD)
        // sees a consistent elapsed/delta/frame_count.
        self.time.advance(dt);
        propagate_transforms(&mut self.world);
        // Advance skeletal animations (no-op if no SkinnedPose entities).
        voltex_renderer_ecs::update_skinned_poses(&mut self.world, dt);
        // Advance CPU particles: move, fade, despawn (no-op if none).
        voltex_renderer_ecs::update_particles(&mut self.world, dt);
        // Advance sprite-sheet animations (no-op if no SpriteAnimation entities).
        voltex_renderer_ecs::update_sprite_animations(&mut self.world, dt);
        if self.config.enable_physics {
            voltex_physics::physics_step(&mut self.world, &self.physics_config);
        }
        // Collision/trigger events: detect contacts and hand them to the game.
        // Runs even without physics dynamics, so it works as a trigger detector.
        if let Some(mut cf) = self.collision_fn.take() {
            let contacts = detect_collisions(&self.world);
            cf(&contacts, &mut self.world, dt);
            self.collision_fn = Some(cf);
        }
        if self.config.enable_scripts {
            // Publish the current frame's input so the `input` Lua table reads
            // live state. Re-derive the pointer every frame (the app may have
            // moved) and clear it after, mirroring the `__voltex_world` rule.
            fill_script_input(&mut self.script_input, &self.input);
            voltex_script::set_current_input(&self.script_input as *const _);
            voltex_script::script_start_all(&mut self.world, &mut self.script_pool);
            voltex_script::script_update_all(&mut self.world, &mut self.script_pool, dt);
            voltex_script::set_current_input(std::ptr::null());
            let _ = self.script_pool.poll_reload(&mut self.file_watcher);
        }
        for sys in self.user_systems.iter_mut() {
            sys(&mut self.world, dt);
        }
        for sys in self.input_systems.iter_mut() {
            sys(&mut self.world, &self.input, dt);
        }
        for sys in self.action_systems.iter_mut() {
            sys(&mut self.world, &self.input, &self.input_map, dt);
        }
        for sys in self.timed_systems.iter_mut() {
            sys(&mut self.world, &self.time);
        }
        // Audio runs last so it observes playback toggles / listener moves made
        // by user systems this same frame (lowest latency).
        if let Some(audio) = self.audio.as_mut() {
            // Scene-persisted AudioPath → clip ids first, so a source bound
            // this frame can already play this frame. Idempotent +
            // failure-cached (mirrors the texture/mesh resolver in handler).
            crate::audio_paths::resolve_audio_paths(
                &mut self.world,
                audio,
                &mut self.audio_clip_cache,
                self.config.vfs.as_ref(),
                self.asset_root.as_deref(),
            );
            let audio = &*audio;
            crate::audio_sync::audio_sync_system(
                &mut self.world,
                audio,
                self.audio_listener,
                &mut self.audio_voices,
            );
            for (path, outcome) in crate::audio_sync::poll_audio_hot_reload(
                &mut self.audio_watcher,
                &self.watched_audio_clips,
                audio,
            ) {
                match outcome {
                    Ok(()) => crate::log_info!(
                        "voltex_runtime: hot-reloaded audio clip: {}",
                        path.display()
                    ),
                    Err(e) => crate::log_error!(
                        "voltex_runtime: audio hot-reload failed for {}: {e}",
                        path.display()
                    ),
                }
            }
        }
        // Despawn expired-lifetime entities after user systems have observed
        // them on their final frame (no-op if no Lifetime entities).
        crate::lifetime::update_lifetimes(&mut self.world, dt);
    }

    /// Take over the calling thread with a winit event loop until the window
    /// is closed. GPU resources are constructed lazily on the first `resumed`
    /// callback, so `tick_systems_only` may be called before this.
    ///
    /// Side-effect: when the config's `enable_crash_log` is true (default),
    /// installs a `std::panic::set_hook` that writes
    /// `<window_title>-crash-<UTC>.txt` alongside the default stderr panic.
    /// Idempotent across multiple `run` calls in the same process.
    pub fn run(self) -> RuntimeResult<()> {
        // The panic hook + logger were installed in `new`; both are
        // OnceLock-guarded so re-installing here would be a no-op. We do
        // log the "entering event loop" transition explicitly.
        crate::log_info!("voltex_runtime: entering event loop");
        let event_loop = winit::event_loop::EventLoop::new()
            .map_err(|e| RuntimeError::WindowInit(e.to_string()))?;
        let mut handler = crate::handler::RuntimeHandler { app: Some(self) };
        let result = event_loop
            .run_app(&mut handler)
            .map_err(|e| RuntimeError::WindowInit(e.to_string()));
        match &result {
            Ok(()) => crate::log_info!("voltex_runtime exited normally"),
            Err(e) => crate::log_error!("voltex_runtime exited with error: {}", e),
        }
        result
    }
}

/// Mapping from the string names exposed to Lua (`input.is_key_pressed("W")`)
/// to platform keycodes. Owning the table here keeps `voltex_script` free of
/// any windowing/winit dependency. Unlisted keys are simply unqueryable.
const SCRIPT_KEY_NAMES: &[(&str, winit::keyboard::KeyCode)] = {
    use winit::keyboard::KeyCode as K;
    &[
        ("A", K::KeyA), ("B", K::KeyB), ("C", K::KeyC), ("D", K::KeyD),
        ("E", K::KeyE), ("F", K::KeyF), ("G", K::KeyG), ("H", K::KeyH),
        ("I", K::KeyI), ("J", K::KeyJ), ("K", K::KeyK), ("L", K::KeyL),
        ("M", K::KeyM), ("N", K::KeyN), ("O", K::KeyO), ("P", K::KeyP),
        ("Q", K::KeyQ), ("R", K::KeyR), ("S", K::KeyS), ("T", K::KeyT),
        ("U", K::KeyU), ("V", K::KeyV), ("W", K::KeyW), ("X", K::KeyX),
        ("Y", K::KeyY), ("Z", K::KeyZ),
        ("0", K::Digit0), ("1", K::Digit1), ("2", K::Digit2), ("3", K::Digit3),
        ("4", K::Digit4), ("5", K::Digit5), ("6", K::Digit6), ("7", K::Digit7),
        ("8", K::Digit8), ("9", K::Digit9),
        ("Space", K::Space), ("Enter", K::Enter), ("Escape", K::Escape),
        ("Tab", K::Tab),
        ("Left", K::ArrowLeft), ("Right", K::ArrowRight),
        ("Up", K::ArrowUp), ("Down", K::ArrowDown),
        ("LShift", K::ShiftLeft), ("RShift", K::ShiftRight),
        ("LCtrl", K::ControlLeft), ("RCtrl", K::ControlRight),
        ("LAlt", K::AltLeft), ("RAlt", K::AltRight),
    ]
};

/// Translate the platform [`InputState`] into the windowing-agnostic
/// [`voltex_script::ScriptInput`] snapshot read by Lua.
fn fill_script_input(si: &mut voltex_script::ScriptInput, input: &InputState) {
    use winit::event::MouseButton;
    si.clear();
    for (name, code) in SCRIPT_KEY_NAMES {
        if input.is_key_pressed(*code) {
            si.set_key_pressed(name);
        }
        if input.is_key_just_pressed(*code) {
            si.set_key_just_pressed(name);
        }
    }
    let (mx, my) = input.mouse_position();
    let (dx, dy) = input.mouse_delta();
    si.set_mouse(mx, my, dx, dy);
    si.set_buttons(
        input.is_mouse_button_pressed(MouseButton::Left),
        input.is_mouse_button_pressed(MouseButton::Right),
        input.is_mouse_button_pressed(MouseButton::Middle),
    );
    si.set_scroll(input.mouse_scroll());
}

fn load_scene_via_vfs_or_disk(
    world: &mut World,
    registry: &ComponentRegistry,
    path: &str,
    vfs: Option<&voltex_asset::Vfs>,
) -> RuntimeResult<()> {
    let data = match vfs {
        Some(v) if v.exists(path) => v
            .read(path)
            .map_err(|e| RuntimeError::SceneLoad(format!("{}", e)))?,
        _ => std::fs::read(path).map_err(|e| RuntimeError::SceneLoad(e.to_string()))?,
    };
    // Try binary first, then JSON.
    if voltex_ecs::deserialize_scene_binary(world, &data, registry).is_ok() {
        return Ok(());
    }
    let text = std::str::from_utf8(&data).map_err(|e| RuntimeError::SceneLoad(e.to_string()))?;
    voltex_ecs::deserialize_scene_json(world, text, registry)
        .map_err(|e| RuntimeError::SceneLoad(format!("{}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;

    #[test]
    fn new_runs_register_components_and_scene_builder_in_order() {
        let order = Rc::new(Cell::new(Vec::<&'static str>::new()));
        let o1 = order.clone();
        let o2 = order.clone();
        let cfg = RuntimeConfig::new()
            .with_registry_setup(move |_reg| {
                let mut v = o1.take();
                v.push("registry");
                o1.set(v);
            })
            .with_scene_builder(move |world, _reg| {
                let mut v = o2.take();
                v.push("scene");
                o2.set(v);
                let e = world.spawn();
                world.add(e, Transform::from_position(Vec3::new(7.0, 0.0, 0.0)));
            });

        let app = RuntimeApp::new(cfg).expect("RuntimeApp::new");
        let order = order.take();
        assert_eq!(order, vec!["registry", "scene"]);
        assert_eq!(app.world.entity_count(), 1);
    }

    #[test]
    fn with_audio_files_decodes_and_watches_with_stable_indices() {
        let dir = std::env::temp_dir();
        let good = dir.join(format!("voltex_waf_good_{}.wav", std::process::id()));
        let bad = dir.join(format!("voltex_waf_bad_{}.wav", std::process::id()));
        std::fs::write(&good, voltex_audio::generate_wav_bytes(&[0.1, -0.1], 44100)).unwrap();
        std::fs::write(&bad, b"RIFFcorrupt").unwrap();

        // One pre-loaded clip (index 0), then files: bad → placeholder at 1,
        // good keeps its position at 2 (index stability is the contract).
        let cfg = RuntimeConfig::new()
            .with_audio(vec![voltex_audio::AudioClip::new(vec![0.0; 4], 44100, 1)])
            .with_audio_files(vec![bad.clone(), good.clone()]);
        let app = RuntimeApp::new(cfg).expect("decode failure must not abort boot");

        assert!(app.audio.is_some(), "audio system built");
        assert_eq!(app.watched_audio_clips.len(), 2, "both files watched");
        assert_eq!(app.watched_audio_clips.get(&bad), Some(&1));
        assert_eq!(app.watched_audio_clips.get(&good), Some(&2));

        let _ = std::fs::remove_file(&good);
        let _ = std::fs::remove_file(&bad);
    }

    #[test]
    fn with_audio_files_alone_starts_indices_at_zero() {
        let path = std::env::temp_dir()
            .join(format!("voltex_waf_solo_{}.wav", std::process::id()));
        std::fs::write(&path, voltex_audio::generate_wav_bytes(&[0.2], 22050)).unwrap();

        let cfg = RuntimeConfig::new().with_audio_files(vec![path.clone()]);
        let app = RuntimeApp::new(cfg).expect("RuntimeApp::new");
        assert_eq!(app.watched_audio_clips.get(&path), Some(&0));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn tick_systems_only_runs_user_systems_in_order() {
        let log = Rc::new(Cell::new(Vec::<u32>::new()));
        let l1 = log.clone();
        let l2 = log.clone();
        let cfg = RuntimeConfig::new()
            .disable_physics()
            .disable_scripts()
            .with_system_setup(move |app| {
                let la = l1.clone();
                app.add_system(move |_w, _dt| {
                    let mut v = la.take();
                    v.push(1);
                    la.set(v);
                });
                let lb = l2.clone();
                app.add_system(move |_w, _dt| {
                    let mut v = lb.take();
                    v.push(2);
                    lb.set(v);
                });
            });

        let mut app = RuntimeApp::new(cfg).expect("RuntimeApp::new");
        assert_eq!(app.user_system_count(), 2);
        app.tick_systems_only(0.016);
        app.tick_systems_only(0.016);
        let recorded = log.take();
        assert_eq!(recorded, vec![1, 2, 1, 2]);
    }

    #[test]
    fn tick_systems_only_propagates_transforms() {
        use voltex_ecs::{add_child, WorldTransform};
        let cfg = RuntimeConfig::new().disable_physics().disable_scripts();
        let mut app = RuntimeApp::new(cfg).unwrap();

        let parent = app.world.spawn();
        app.world.add(parent, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        let child = app.world.spawn();
        app.world.add(child, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));
        add_child(&mut app.world, parent, child);

        app.tick_systems_only(0.016);

        let wt = app.world.get::<WorldTransform>(child).expect("child WorldTransform");
        let translation = wt.0.cols[3];
        assert!((translation[0] - 11.0).abs() < 1e-5);
        assert!((translation[1] - 2.0).abs() < 1e-5);
        assert!((translation[2] - 3.0).abs() < 1e-5);
    }

    #[test]
    fn initial_scene_missing_path_is_silent() {
        let cfg = RuntimeConfig::new().with_initial_scene("/no/such/file.vscn");
        // Must not return an error — missing initial scene degrades gracefully so
        // a fresh project can boot before any scene is saved.
        let app = RuntimeApp::new(cfg).expect("RuntimeApp::new should not fail on missing scene");
        assert_eq!(app.world.entity_count(), 0);
    }

    // ── v1.0.18: hook + logger installed in new(), not run() ─────────────

    #[test]
    fn new_with_crash_log_disabled_still_succeeds() {
        // Asserts that opting out of the crash hook does not break the
        // `new` path. With v1.0.18 the hook install moved from `run` to
        // `new`, so the opt-out branch needed to remain wired.
        let cfg = RuntimeConfig::new().disable_crash_log();
        let app = RuntimeApp::new(cfg).expect("new should succeed with crash log disabled");
        assert_eq!(app.world.entity_count(), 0);
    }

    #[test]
    fn new_runs_build_scene_with_logger_initialised() {
        // The logger is initialised at the top of `new`. A build_scene
        // callback that touches the log macros must therefore work.
        // Pre-v1.0.18 the macros would have been emit-only via the
        // default fallback Info-level logger initialised lazily — still
        // fine, but this test pins the contract: macros never panic when
        // called from build_scene.
        let cfg = RuntimeConfig::new()
            .disable_crash_log() // skip global panic-hook install in test
            .with_scene_builder(|world, _reg| {
                crate::log_info!("scene builder ran with {} entities", world.entity_count());
            });
        let _app = RuntimeApp::new(cfg).expect("new should succeed");
    }

    // ── v1.0.1: add_input_system regression coverage ─────────────────────

    #[test]
    fn add_input_system_runs_after_regular_systems() {
        // Verifies the ordering invariant `tick_systems_only` documents:
        // input_systems fire *after* user_systems each frame.
        let log = Rc::new(Cell::new(Vec::<&'static str>::new()));
        let l_user = log.clone();
        let l_input = log.clone();
        let cfg = RuntimeConfig::new()
            .disable_physics()
            .disable_scripts()
            .with_system_setup(move |app| {
                let lu = l_user.clone();
                app.add_system(move |_w, _dt| {
                    let mut v = lu.take();
                    v.push("user");
                    lu.set(v);
                });
                let li = l_input.clone();
                app.add_input_system(move |_w, _input, _dt| {
                    let mut v = li.take();
                    v.push("input");
                    li.set(v);
                });
            });
        let mut app = RuntimeApp::new(cfg).unwrap();
        assert_eq!(app.user_system_count(), 1);
        assert_eq!(app.input_system_count(), 1);
        app.tick_systems_only(0.016);
        assert_eq!(log.take(), vec!["user", "input"]);
    }

    #[test]
    fn action_system_reads_input_map_bindings() {
        use voltex_platform::KeyCode;

        let mut map = InputMap::new();
        map.bind_key("jump", KeyCode::Space);
        let cfg = RuntimeConfig::new()
            .disable_physics()
            .disable_scripts()
            .with_input_map(map);
        let mut app = RuntimeApp::new(cfg).unwrap();

        let jumped = Rc::new(Cell::new(false));
        let flag = jumped.clone();
        app.add_action_system(move |_w, input, map, _dt| {
            if map.just_pressed("jump", input) {
                flag.set(true);
            }
        });
        assert_eq!(app.action_system_count(), 1);

        // No key held → action does not fire.
        app.tick_systems_only(0.016);
        assert!(!jumped.get(), "jump fired with no key pressed");

        // Press the bound key → the action system observes it via the map.
        app.input.process_key(KeyCode::Space, true);
        app.tick_systems_only(0.016);
        assert!(jumped.get(), "action system should observe the bound 'jump' key");
    }

    #[test]
    fn time_accumulates_across_ticks() {
        let cfg = RuntimeConfig::new().disable_physics().disable_scripts();
        let mut app = RuntimeApp::new(cfg).unwrap();
        assert_eq!(app.time.frame_count, 0);

        app.tick_systems_only(0.5);
        app.tick_systems_only(0.5);

        assert_eq!(app.time.frame_count, 2);
        assert_eq!(app.time.delta_secs, 0.5);
        assert!((app.time.elapsed_secs - 1.0).abs() < 1e-5, "elapsed={}", app.time.elapsed_secs);
    }

    #[test]
    fn timed_system_receives_advancing_clock() {
        let cfg = RuntimeConfig::new().disable_physics().disable_scripts();
        let mut app = RuntimeApp::new(cfg).unwrap();

        let seen = Rc::new(Cell::new(0.0f32));
        let probe = seen.clone();
        app.add_timed_system(move |_w, time| probe.set(time.elapsed_secs));
        assert_eq!(app.timed_system_count(), 1);

        app.tick_systems_only(0.25);
        assert!((seen.get() - 0.25).abs() < 1e-5, "first frame elapsed = {}", seen.get());
        app.tick_systems_only(0.25);
        assert!((seen.get() - 0.5).abs() < 1e-5, "second frame elapsed = {}", seen.get());
    }

    #[test]
    fn camera_config_defaults_and_builder() {
        let d = RuntimeConfig::default();
        assert_eq!(d.camera_position, Vec3::new(0.0, 10.0, 20.0));
        assert_eq!(d.camera_yaw, 0.0);
        assert!((d.camera_pitch + 0.4).abs() < 1e-6);
        let c = RuntimeConfig::new().with_camera(Vec3::new(1.0, 2.0, 3.0), 0.5, -0.2);
        assert_eq!(c.camera_position, Vec3::new(1.0, 2.0, 3.0));
        assert!((c.camera_yaw - 0.5).abs() < 1e-6);
        assert!((c.camera_pitch + 0.2).abs() < 1e-6);
    }

    #[test]
    fn set_camera_fn_registers_and_runs_on_camera() {
        let cfg = RuntimeConfig::new().disable_physics().disable_scripts().disable_crash_log();
        let mut app = RuntimeApp::new(cfg).unwrap();
        assert!(!app.has_camera_fn());

        // Capture that the updater is actually invoked against a Camera.
        let seen = Rc::new(Cell::new(false));
        let s = seen.clone();
        app.set_camera_fn(move |cam, _world, _dt| {
            cam.position = Vec3::new(3.0, 3.0, 3.0);
            s.set(true);
        });
        assert!(app.has_camera_fn());

        // Drive the updater directly (no window needed) to prove it mutates a Camera.
        let mut fnc = app.camera_fn.take().unwrap();
        let mut cam = voltex_renderer::Camera::new(Vec3::ZERO, 1.0);
        fnc(&mut cam, &app.world, 0.016);
        assert!(seen.get(), "camera_fn should have run");
        assert_eq!(cam.position, Vec3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn save_and_load_scene_roundtrips_entities() {
        let dir = std::env::temp_dir().join("voltex_runtime_scene_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("scene.json");
        let p = path.to_str().unwrap();

        let cfg = RuntimeConfig::new()
            .disable_physics()
            .disable_scripts()
            .disable_crash_log();
        let mut app = RuntimeApp::new(cfg).unwrap();
        let e = app.world.spawn();
        app.world
            .add(e, Transform::from_position(Vec3::new(3.0, 4.0, 5.0)));
        app.save_scene_json(p).unwrap();

        let mut app2 = RuntimeApp::new(
            RuntimeConfig::new()
                .disable_physics()
                .disable_scripts()
                .disable_crash_log(),
        )
        .unwrap();
        app2.load_scene(p).unwrap();

        assert_eq!(app2.world.entity_count(), 1);
        let pos = app2
            .world
            .query::<Transform>()
            .next()
            .map(|(_, t)| t.position)
            .expect("loaded Transform");
        assert!(
            (pos.x - 3.0).abs() < 1e-4 && (pos.y - 4.0).abs() < 1e-4 && (pos.z - 5.0).abs() < 1e-4,
            "position should roundtrip, got {pos:?}"
        );
    }

    #[test]
    fn collision_fn_receives_overlapping_pairs() {
        use voltex_physics::Collider;
        let cfg = RuntimeConfig::new()
            .disable_physics()
            .disable_scripts()
            .disable_crash_log();
        let mut app = RuntimeApp::new(cfg).unwrap();

        // Two overlapping unit spheres → one contact pair.
        let a = app.world.spawn();
        app.world.add(a, Transform::from_position(Vec3::ZERO));
        app.world.add(a, Collider::Sphere { radius: 1.0 });
        let b = app.world.spawn();
        app.world.add(b, Transform::from_position(Vec3::new(0.5, 0.0, 0.0)));
        app.world.add(b, Collider::Sphere { radius: 1.0 });

        let hits = Rc::new(Cell::new(0usize));
        let h = hits.clone();
        app.set_collision_fn(move |contacts, _world, _dt| h.set(contacts.len()));
        assert!(app.has_collision_fn());

        // Trigger detection runs even though physics is disabled.
        app.tick_systems_only(0.016);
        assert!(hits.get() >= 1, "overlapping colliders should report a contact");

        // Separated → no contact.
        if let Some(t) = app.world.get_mut::<Transform>(b) {
            t.position = Vec3::new(10.0, 0.0, 0.0);
        }
        app.tick_systems_only(0.016);
        assert_eq!(hits.get(), 0, "separated colliders should report no contact");
    }

    #[test]
    fn set_light_fn_registers_and_runs_on_light() {
        let cfg = RuntimeConfig::new().disable_physics().disable_scripts().disable_crash_log();
        let mut app = RuntimeApp::new(cfg).unwrap();
        assert!(!app.has_light_fn());
        app.set_light_fn(|light, _world, _dt| {
            light.color = [0.0, 1.0, 0.0];
        });
        assert!(app.has_light_fn());
        let mut fnc = app.light_fn.take().unwrap();
        let mut light = voltex_renderer::LightUniform::new();
        fnc(&mut light, &app.world, 0.016);
        assert_eq!(light.color, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn set_point_lights_fn_registers_and_mutates_set() {
        let cfg = RuntimeConfig::new().disable_physics().disable_scripts().disable_crash_log();
        let mut app = RuntimeApp::new(cfg).unwrap();
        assert!(!app.has_point_lights_fn());
        app.set_point_lights_fn(|lights, _world, _dt| {
            lights.clear();
            lights.push(voltex_renderer::PointLight::new(
                [1.0, 2.0, 3.0],
                [1.0, 0.5, 0.0],
                2.0,
                8.0,
            ));
        });
        assert!(app.has_point_lights_fn());
        let mut fnc = app.point_lights_fn.take().unwrap();
        fnc(&mut app.point_lights, &app.world, 0.016);
        assert_eq!(app.point_lights.len(), 1);
        assert_eq!(app.point_lights[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(app.point_lights[0].range, 2.0);
        assert_eq!(app.point_lights[0].intensity, 8.0);
    }

    #[test]
    fn set_hud_fn_registers_callback() {
        let cfg = RuntimeConfig::new()
            .disable_physics()
            .disable_scripts()
            .disable_crash_log()
            .with_hud(true);
        let mut app = RuntimeApp::new(cfg).unwrap();
        assert!(!app.has_hud_fn(), "no HUD callback before set_hud_fn");
        app.set_hud_fn(|ui, world| {
            ui.text(&format!("entities: {}", world.entity_count()));
        });
        assert!(app.has_hud_fn(), "HUD callback should be registered");
        assert!(app.config.enable_hud);
    }

    // ── Lua input API wiring ─────────────────────────────────────────────

    #[test]
    fn fill_script_input_maps_keys_and_buttons() {
        use winit::event::MouseButton;
        use winit::keyboard::KeyCode;
        let mut input = InputState::new();
        input.process_key(KeyCode::KeyW, true);
        input.process_mouse_button(MouseButton::Left, true);
        let mut si = voltex_script::ScriptInput::new();
        fill_script_input(&mut si, &input);
        assert!(si.key_pressed("W"), "W should map to pressed");
        assert!(si.key_just_pressed("W"), "W press edge should map");
        assert!(!si.key_pressed("A"), "A was not pressed");
        assert!(si.button_pressed("left"), "left button should map");
        assert!(!si.button_pressed("right"));
    }

    #[test]
    fn script_reads_live_input_through_tick() {
        use voltex_ecs::{ScriptComponent, Transform};
        use voltex_math::Vec3;
        let dir = std::env::temp_dir().join("voltex_runtime_input_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("input_mover.lua");
        std::fs::write(
            &path,
            "function on_start(id, gen) end
             function on_update(id, gen, dt)
                 if input.is_key_pressed('W') then
                     set_position(id, gen, 0.0, 0.0, 5.0)
                 end
             end",
        )
        .unwrap();

        // Scripts are enabled by default; physics off to keep the test focused.
        let cfg = RuntimeConfig::new().disable_physics().disable_crash_log();
        let mut app = RuntimeApp::new(cfg).unwrap();
        let e = app.world.spawn();
        app.world.add(e, Transform::from_position(Vec3::ZERO));
        app.world
            .add(e, ScriptComponent::new(path.to_str().unwrap().to_string()));

        // No key held → entity stays at the origin.
        app.tick_systems_only(0.016);
        assert!(app.world.get::<Transform>(e).unwrap().position.z.abs() < 1e-6);

        // Hold W → the script reads it via the `input` table and moves the entity.
        app.input
            .process_key(winit::keyboard::KeyCode::KeyW, true);
        app.tick_systems_only(0.016);
        let z = app.world.get::<Transform>(e).unwrap().position.z;
        assert!(
            (z - 5.0).abs() < 1e-6,
            "script should have observed W and moved entity, got z={z}"
        );
    }

    #[test]
    fn input_system_receives_app_input_reference() {
        // The input system must observe the runtime's `app.input` field
        // (same reference, same state) so user code can read keyboard /
        // mouse state captured by the winit handler.
        let captured_pressed = Rc::new(Cell::new(false));
        let cap = captured_pressed.clone();
        let cfg = RuntimeConfig::new()
            .disable_physics()
            .disable_scripts()
            .with_system_setup(move |app| {
                // Pre-set a key in the app's input so the system sees it.
                app.input.process_key(
                    winit::keyboard::KeyCode::KeyW,
                    true,
                );
                let cc = cap.clone();
                app.add_input_system(move |_w, input, _dt| {
                    cc.set(input.is_key_pressed(winit::keyboard::KeyCode::KeyW));
                });
            });
        let mut app = RuntimeApp::new(cfg).unwrap();
        app.tick_systems_only(0.016);
        assert!(
            captured_pressed.get(),
            "input system did not see the pre-set KeyW press"
        );
    }
}
