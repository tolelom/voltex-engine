//! Declarative configuration for `RuntimeApp`. Callers describe what the
//! runtime should do in builder chain form; `RuntimeApp::new` consumes it and
//! runs the registered callbacks during boot.

use voltex_ecs::{ComponentRegistry, World};
use voltex_platform::WindowConfig;

pub type RegistrySetupFn = Box<dyn FnOnce(&mut ComponentRegistry)>;
pub type SceneBuilderFn = Box<dyn FnOnce(&mut World, &mut ComponentRegistry)>;
pub type SystemSetupFn = Box<dyn FnOnce(&mut crate::app::RuntimeApp)>;

/// All knobs `RuntimeApp` exposes. Builder methods return `Self` for chaining.
pub struct RuntimeConfig {
    pub window: WindowConfig,
    pub initial_scene: Option<String>,
    pub register_components: Option<RegistrySetupFn>,
    pub build_scene: Option<SceneBuilderFn>,
    pub setup_systems: Option<SystemSetupFn>,
    /// When `Some(dt)`, every frame's update step is forced to that delta —
    /// useful for deterministic playback. When `None`, the wallclock-derived
    /// `GameTimer::frame_dt` is used.
    pub fixed_dt: Option<f32>,
    pub enable_physics: bool,
    pub enable_scripts: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            initial_scene: None,
            register_components: None,
            build_scene: None,
            setup_systems: None,
            fixed_dt: None,
            enable_physics: true,
            enable_scripts: true,
        }
    }
}

impl RuntimeConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_window_title(mut self, title: impl Into<String>) -> Self {
        self.window.title = title.into();
        self
    }

    pub fn with_window_size(mut self, width: u32, height: u32) -> Self {
        self.window.width = width;
        self.window.height = height;
        self
    }

    pub fn with_initial_scene(mut self, path: impl Into<String>) -> Self {
        self.initial_scene = Some(path.into());
        self
    }

    pub fn with_registry_setup<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut ComponentRegistry) + 'static,
    {
        self.register_components = Some(Box::new(f));
        self
    }

    pub fn with_scene_builder<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut World, &mut ComponentRegistry) + 'static,
    {
        self.build_scene = Some(Box::new(f));
        self
    }

    pub fn with_system_setup<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut crate::app::RuntimeApp) + 'static,
    {
        self.setup_systems = Some(Box::new(f));
        self
    }

    pub fn with_fixed_dt(mut self, dt: f32) -> Self {
        self.fixed_dt = Some(dt);
        self
    }

    pub fn disable_physics(mut self) -> Self {
        self.enable_physics = false;
        self
    }

    pub fn disable_scripts(mut self) -> Self {
        self.enable_scripts = false;
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
}
