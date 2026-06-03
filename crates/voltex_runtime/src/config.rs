//! Declarative configuration for `RuntimeApp`. Callers describe what the
//! runtime should do in builder chain form; `RuntimeApp::new` consumes it and
//! runs the registered callbacks during boot.

use std::path::PathBuf;

use voltex_ecs::{ComponentRegistry, World};
use voltex_platform::WindowConfig;

/// Boxed closure that augments the [`ComponentRegistry`] during boot.
pub type RegistrySetupFn = Box<dyn FnOnce(&mut ComponentRegistry)>;
/// Boxed closure that populates the initial scene World.
pub type SceneBuilderFn = Box<dyn FnOnce(&mut World, &mut ComponentRegistry)>;
/// Boxed closure that runs after default systems are registered to add custom ones.
pub type SystemSetupFn = Box<dyn FnOnce(&mut crate::app::RuntimeApp)>;
/// Callback that runs once on first `resumed`, after `GpuContext` and
/// `BasicRenderer` exist. Use this to upload meshes to `RenderAssets` and
/// attach `MeshHandle` / `MaterialHandle` components — those require a
/// `wgpu::Device` so they cannot live in `with_scene_builder`.
pub type GpuInitFn =
    Box<dyn FnOnce(&mut crate::app::RuntimeApp, &wgpu::Device, &wgpu::Queue)>;

/// All knobs `RuntimeApp` exposes. Builder methods return `Self` for chaining.
pub struct RuntimeConfig {
    /// Window configuration (title, size, fullscreen, vsync).
    pub window: WindowConfig,
    /// Path of a `.vscn` scene to load on boot, or `None`.
    pub initial_scene: Option<String>,
    /// Optional callback to register custom components on the registry.
    pub register_components: Option<RegistrySetupFn>,
    /// Optional callback to build the initial scene World.
    pub build_scene: Option<SceneBuilderFn>,
    /// Optional callback to register custom systems after defaults.
    pub setup_systems: Option<SystemSetupFn>,
    /// Optional callback that runs once after GPU init (uploads meshes etc.).
    pub gpu_init: Option<GpuInitFn>,
    /// When `Some(dt)`, every frame's update step is forced to that delta —
    /// useful for deterministic playback. When `None`, the wallclock-derived
    /// `GameTimer::frame_dt` is used.
    pub fixed_dt: Option<f32>,
    /// Enable the physics solver step in the default schedule.
    pub enable_physics: bool,
    /// Enable Lua script start/update systems in the default schedule.
    pub enable_scripts: bool,
    /// When `true`, the runtime constructs an immediate-mode UI context +
    /// renderer on GPU init and draws an in-window HUD overlay each frame via
    /// the callback registered with [`crate::RuntimeApp::set_hud_fn`]. Default
    /// `false` (no overlay; the title-bar [`crate::Hud`] is always available).
    pub enable_hud: bool,
    /// When `true`, the runtime owns an [`voltex_audio::AudioSystem`] built from
    /// [`Self::audio_clips`] and runs `audio_sync_system` each frame to drive 3D
    /// playback from `AudioSource` components. Set via [`Self::with_audio`].
    pub enable_audio: bool,
    /// Pre-loaded clips handed to the [`voltex_audio::AudioSystem`] when
    /// `enable_audio` is set. `AudioSource::clip_id` indexes into this list.
    pub audio_clips: Vec<voltex_audio::AudioClip>,
    /// Optional virtual filesystem to consult first when loading scenes /
    /// scripts. When `None`, paths are read directly from disk.
    pub vfs: Option<voltex_asset::Vfs>,
    /// When `true` (default), [`crate::RuntimeApp::run`] installs a
    /// `std::panic::set_hook` that writes a crash log file alongside the
    /// default stderr behaviour. Set to `false` for headless tests or when
    /// the host application installs its own panic hook.
    pub enable_crash_log: bool,
    /// Directory the crash log is written to. When `None`, the current
    /// working directory at hook-fire time is used.
    pub crash_log_dir: Option<PathBuf>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            initial_scene: None,
            register_components: None,
            build_scene: None,
            setup_systems: None,
            gpu_init: None,
            fixed_dt: None,
            enable_physics: true,
            enable_scripts: true,
            enable_hud: false,
            enable_audio: false,
            audio_clips: Vec::new(),
            vfs: None,
            enable_crash_log: true,
            crash_log_dir: None,
        }
    }
}

impl RuntimeConfig {
    /// New config with engine defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the window title.
    pub fn with_window_title(mut self, title: impl Into<String>) -> Self {
        self.window.title = title.into();
        self
    }

    /// Sets the window pixel size.
    pub fn with_window_size(mut self, width: u32, height: u32) -> Self {
        self.window.width = width;
        self.window.height = height;
        self
    }

    /// Loads the named `.vscn` file at boot.
    pub fn with_initial_scene(mut self, path: impl Into<String>) -> Self {
        self.initial_scene = Some(path.into());
        self
    }

    /// Registers a callback to add custom components to the registry on boot.
    pub fn with_registry_setup<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut ComponentRegistry) + 'static,
    {
        self.register_components = Some(Box::new(f));
        self
    }

    /// Registers a callback to populate the initial scene World.
    pub fn with_scene_builder<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut World, &mut ComponentRegistry) + 'static,
    {
        self.build_scene = Some(Box::new(f));
        self
    }

    /// Registers a callback that adds custom systems after defaults.
    pub fn with_system_setup<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut crate::app::RuntimeApp) + 'static,
    {
        self.setup_systems = Some(Box::new(f));
        self
    }

    /// Register a one-shot callback that runs on first `resumed` (after GPU
    /// is up). Use for uploading meshes / materials and attaching MeshHandle
    /// to entities seeded by `with_scene_builder`.
    pub fn with_gpu_init<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut crate::app::RuntimeApp, &wgpu::Device, &wgpu::Queue) + 'static,
    {
        self.gpu_init = Some(Box::new(f));
        self
    }

    /// Forces a fixed timestep `dt` (seconds) — useful for deterministic playback.
    pub fn with_fixed_dt(mut self, dt: f32) -> Self {
        self.fixed_dt = Some(dt);
        self
    }

    /// Removes physics_step from the default schedule.
    pub fn disable_physics(mut self) -> Self {
        self.enable_physics = false;
        self
    }

    /// Removes Lua script systems from the default schedule.
    pub fn disable_scripts(mut self) -> Self {
        self.enable_scripts = false;
        self
    }

    /// Enable the in-window HUD overlay. Combine with
    /// [`crate::RuntimeApp::set_hud_fn`] to draw text / bars over the 3D scene.
    pub fn with_hud(mut self, enabled: bool) -> Self {
        self.enable_hud = enabled;
        self
    }

    /// Enable audio: the runtime builds an `AudioSystem` from `clips` and runs
    /// `audio_sync_system` each frame. `AudioSource::clip_id` indexes `clips`.
    /// Set the listener entity with [`crate::RuntimeApp::set_audio_listener`].
    pub fn with_audio(mut self, clips: Vec<voltex_audio::AudioClip>) -> Self {
        self.enable_audio = true;
        self.audio_clips = clips;
        self
    }

    /// Attach a `Vfs` (filesystem-rooted or embedded `.vpkg`). When set, the
    /// runtime reads scene / script files through the Vfs first; fs path
    /// fallback still applies for paths the Vfs cannot resolve.
    pub fn with_vfs(mut self, vfs: voltex_asset::Vfs) -> Self {
        self.vfs = Some(vfs);
        self
    }

    /// Override the directory the crash log is written to. Pass `None` (or
    /// don't call this method) to use the current working directory.
    pub fn with_crash_log_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.crash_log_dir = Some(dir.into());
        self
    }

    /// Opt out of the crash-log panic hook. Use when the host application
    /// installs its own hook or runs in a context where file writes are
    /// not desired (e.g. fuzzing / headless test harnesses).
    pub fn disable_crash_log(mut self) -> Self {
        self.enable_crash_log = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_enables_physics_and_scripts() {
        let c = RuntimeConfig::default();
        assert!(c.enable_physics);
        assert!(c.enable_scripts);
        assert!(c.initial_scene.is_none());
        assert!(c.fixed_dt.is_none());
    }

    #[test]
    fn builder_with_window_title_sets_title() {
        let c = RuntimeConfig::new().with_window_title("My Game");
        assert_eq!(c.window.title, "My Game");
    }

    #[test]
    fn builder_with_window_size_sets_size() {
        let c = RuntimeConfig::new().with_window_size(1920, 1080);
        assert_eq!(c.window.width, 1920);
        assert_eq!(c.window.height, 1080);
    }

    #[test]
    fn builder_with_fixed_dt_sets_value() {
        let c = RuntimeConfig::new().with_fixed_dt(1.0 / 60.0);
        assert!(c.fixed_dt.is_some());
        assert!((c.fixed_dt.unwrap() - (1.0 / 60.0)).abs() < 1e-6);
    }

    #[test]
    fn builder_disable_toggles_are_independent() {
        let c = RuntimeConfig::new().disable_physics();
        assert!(!c.enable_physics);
        assert!(c.enable_scripts);

        let c = RuntimeConfig::new().disable_scripts();
        assert!(c.enable_physics);
        assert!(!c.enable_scripts);
    }

    #[test]
    fn default_enables_crash_log() {
        let c = RuntimeConfig::default();
        assert!(c.enable_crash_log);
        assert!(c.crash_log_dir.is_none());
    }

    #[test]
    fn builder_disable_crash_log() {
        let c = RuntimeConfig::new().disable_crash_log();
        assert!(!c.enable_crash_log);
    }

    #[test]
    fn hud_is_off_by_default_and_toggleable() {
        assert!(!RuntimeConfig::default().enable_hud);
        assert!(RuntimeConfig::new().with_hud(true).enable_hud);
    }

    #[test]
    fn audio_is_off_by_default_and_with_audio_enables() {
        assert!(!RuntimeConfig::default().enable_audio);
        let c = RuntimeConfig::new().with_audio(Vec::new());
        assert!(c.enable_audio);
        assert!(c.audio_clips.is_empty());
    }

    #[test]
    fn builder_with_crash_log_dir() {
        let p = std::path::PathBuf::from("/tmp/crashes");
        let c = RuntimeConfig::new().with_crash_log_dir(p.clone());
        assert_eq!(c.crash_log_dir.as_deref(), Some(p.as_path()));
    }
}
