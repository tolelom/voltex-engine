//! `RuntimeApp` — owns every subsystem (World, registry, renderer, script
//! pool, audio bridge, etc.) and exposes a single `run()` entry point that
//! drives the winit event loop until the window closes.
//!
//! ## Drop order
//!
//! Field declaration order below is the actual drop order Rust uses
//! (top-down). It is deliberately:
//!
//! 1. `gpu_window` (GPU + window + renderer) — must release wgpu resources
//!    before the device they belong to is dropped.
//! 2. `user_systems` and the other callbacks — closures that may capture
//!    handles into the World.
//! 3. `file_watcher`, `frame_timer`, `input`, `physics_config` — pure data.
//! 4. `script_pool` — LuaState holds a raw `*mut World`; pool must drop
//!    before the World it points at.
//! 5. `assets`, `registry`, `world` — destroyed last.
//! 6. `hud`, `config` — last, no resource cleanup.

use std::time::Duration;

use voltex_asset::FileWatcher;
use voltex_audio::AudioSystem;
use voltex_ecs::{propagate_transforms, ComponentRegistry, Entity, World};
use voltex_physics::{detect_collisions, PhysicsConfig};
use crate::time::Time;
use voltex_platform::{GameTimer, InputMap, InputState};
use voltex_editor::UiContext;
use voltex_renderer::PointLight;
use voltex_renderer_ecs::RenderAssets;
use voltex_script::ScriptInstancePool;

use crate::config::RuntimeConfig;
use crate::debug_draw::DebugLines;
use crate::error::{RuntimeError, RuntimeResult};

mod boot;
mod hooks;
mod script_input;
#[cfg(test)]
mod tests;

use boot::load_scene_via_vfs_or_disk;
pub(crate) use hooks::HudFn;
use hooks::{
    ActionSystem, CameraFn, CollisionFn, DebugDrawFn, InputSystem, LightFn, PointLightsFn,
    TimedSystem, UserSystem,
};
use script_input::fill_script_input;

/// Poll interval of [`RuntimeApp::file_watcher`] (script hot reload).
const SCRIPT_WATCH_INTERVAL: Duration = Duration::from_millis(500);
/// Poll interval of the audio-clip hot-reload watcher.
const AUDIO_WATCH_INTERVAL: Duration = Duration::from_millis(300);
/// Rate handed to [`GameTimer::new`]. The runtime only reads the timer's
/// wallclock `frame_dt()`, never its fixed-update accumulator, so this value
/// does not affect any engine behaviour.
const FRAME_TIMER_FIXED_HZ: u32 = 60;

/// GPU + OS-window state created in winit's `resumed` callback. Held in an
/// `Option<GpuWindowState>` so the rest of `RuntimeApp` can exist (and be unit
/// tested) before any window is open.
pub(crate) struct GpuWindowState {
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


/// The runtime's top-level application object. Owns the ECS world, asset
/// stores, schedulers, and the main loop integration with `winit`/`wgpu`.
pub struct RuntimeApp {
    /// `None` until the first winit `resumed` (see [`GpuWindowState`]).
    pub(crate) gpu_window: Option<GpuWindowState>,
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
    pub(crate) debug_draw_fn: Option<DebugDrawFn>,
    /// Reused debug-line buffer (cleared + refilled by `debug_draw_fn` each frame).
    pub(crate) debug_lines: DebugLines,
    /// Additive point lights uploaded to the renderer each frame.
    pub(crate) point_lights: Vec<PointLight>,
    /// Optional per-frame point/spot-light updater (see [`Self::set_point_lights_fn`]).
    pub(crate) point_lights_fn: Option<PointLightsFn>,
    /// Watcher the script pool drains every frame (`poll_reload`) for Lua hot
    /// reload. The runtime registers every script file it loads from disk;
    /// scripts seeded from the Vfs (`.vpkg`) are not files and aren't watched.
    pub file_watcher: FileWatcher,
    /// Wallclock frame timer. The runtime calls `tick()` once per redraw and
    /// feeds its `frame_dt()` (clamped to 250 ms) to the systems, unless
    /// [`RuntimeConfig::fixed_frame_dt`] overrides it. Its fixed-update
    /// accumulator (`should_fixed_update`) is not used by the runtime.
    pub frame_timer: GameTimer,
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
        // Not in this crate's unit tests: the hook is process-wide and
        // first-install-wins, so a test RuntimeApp would point it at the cwd
        // (the crate directory) and any other failing test would drop a crash
        // file there. The hook itself is covered by crash.rs tests.
        if config.enable_crash_log && !cfg!(test) {
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

        let mut registry = boot::build_registry(config.register_components.take());
        let world = boot::load_initial_world(&mut config, &mut registry);
        // Resolve the configured asset root once (exe-dir anchoring involves a
        // syscall); every relative asset read below and per-frame goes through it.
        let asset_root = config.resolved_asset_root();
        let configured_audio = boot::load_configured_audio(&mut config, asset_root.as_deref());

        let hud = crate::Hud::new(config.window.title.clone());
        let mut app = Self {
            gpu_window: None,
            user_systems: Vec::new(),
            input_systems: Vec::new(),
            hud_fn: None,
            camera_fn: None,
            light_fn: None,
            collision_fn: None,
            debug_draw_fn: None,
            debug_lines: DebugLines::new(),
            point_lights: Vec::new(),
            point_lights_fn: None,
            file_watcher: FileWatcher::new(SCRIPT_WATCH_INTERVAL),
            frame_timer: GameTimer::new(FRAME_TIMER_FIXED_HZ),
            input: InputState::new(),
            input_map: config.input_map.take().unwrap_or_default(),
            action_systems: Vec::new(),
            time: Time::new(),
            timed_systems: Vec::new(),
            physics_config: PhysicsConfig::default(),
            script_pool: ScriptInstancePool::new(),
            script_input: voltex_script::ScriptInput::new(),
            audio: configured_audio.system,
            audio_voices: crate::audio_sync::AudioVoiceTracker::default(),
            asset_root,
            asset_path_cache: crate::asset_paths::AssetPathCache::new(),
            audio_clip_cache: crate::audio_paths::AudioClipCache::new(),
            audio_watcher: FileWatcher::new(AUDIO_WATCH_INTERVAL),
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
        for (path, clip_index) in configured_audio.hot_reload_watches {
            app.watch_audio_clip(path, clip_index);
        }
        // Seed the AudioPath resolver with the with_audio_files mapping —
        // a scene AudioPath naming the same file binds to the existing clip
        // (no duplicate decode, shared hot-reload). Keyed by the path as
        // configured (the form scenes are expected to carry).
        for (path, clip_index) in configured_audio.path_cache_seeds {
            app.audio_clip_cache.seed(path, clip_index as u32);
        }
        app.seed_scripts_from_vfs();
        if let Some(setup) = app.config.setup_systems.take() {
            setup(&mut app);
        }
        Ok(app)
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
        // Before loading, so a panic inside the load names the scene.
        crate::crash::set_crash_scene(path);
        // clear() 뒤 이전 씬 핸들은 전부 stale이 된다(v1.0.120 generation bump).
        // 이전 씬의 소리는 핸들과 별개로 씬 교체 시점에 끄는 게 맞으므로 전부 stop.
        if let Some(audio) = self.audio.as_ref() {
            self.audio_voices.stop_all(audio);
        }
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
        if let Some(mut collision_fn) = self.collision_fn.take() {
            let contacts = detect_collisions(&self.world);
            collision_fn(&contacts, &mut self.world, dt);
            self.collision_fn = Some(collision_fn);
        }
        if self.config.enable_scripts {
            self.run_scripts(dt);
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
        self.tick_audio();
        // Despawn expired-lifetime entities after user systems have observed
        // them on their final frame (no-op if no Lifetime entities).
        crate::lifetime::update_lifetimes(&mut self.world, dt);
    }

    /// Lua step of [`Self::tick_systems_only`]: publish input, run
    /// `on_start` / `on_update`, then poll script hot reload.
    fn run_scripts(&mut self, dt: f32) {
        // Publish the current frame's input so the `input` Lua table reads
        // live state. Re-derive the pointer every frame (the app may have
        // moved) and clear it after, mirroring the `__voltex_world` rule.
        fill_script_input(&mut self.script_input, &self.input);
        voltex_script::set_current_input(&self.script_input as *const _);
        let mut errors = voltex_script::script_start_all(&mut self.world, &mut self.script_pool);
        // Hot reload: watch every script file the pool just read from disk.
        for path in self.script_pool.take_paths_to_watch() {
            self.file_watcher.watch(path.into());
        }
        errors.extend(voltex_script::script_update_all(&mut self.world, &mut self.script_pool, dt));
        voltex_script::set_current_input(std::ptr::null());
        for error in errors {
            crate::log_error!(
                "voltex_runtime: Lua {} failed in `{}` (entity {}): {}",
                error.callback,
                error.script_path,
                error.entity.id,
                error.message
            );
        }
        let _ = self.script_pool.poll_reload(&mut self.file_watcher);
    }

    /// Audio step of [`Self::tick_systems_only`] (no-op without a mixer):
    /// bind `AudioPath`s, sync `AudioSource` edges to the mixer, then poll
    /// clip hot reload.
    fn tick_audio(&mut self) {
        let Some(audio) = self.audio.as_mut() else { return };
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

    /// Take over the calling thread with a winit event loop until the window
    /// is closed. GPU resources are constructed lazily on the first `resumed`
    /// callback, so `tick_systems_only` may be called before this.
    ///
    /// The crash-log panic hook and env logger are installed by [`Self::new`]
    /// (see there), not here.
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
