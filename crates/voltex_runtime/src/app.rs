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
use voltex_ecs::{propagate_transforms, ComponentRegistry, World};
use voltex_physics::PhysicsConfig;
use voltex_platform::{GameTimer, InputState};
use voltex_renderer_ecs::RenderAssets;
use voltex_script::ScriptInstancePool;

use crate::config::RuntimeConfig;
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
}

type UserSystem = Box<dyn FnMut(&mut World, f32) + 'static>;

pub struct RuntimeApp {
    pub(crate) runtime_state: Option<RuntimeState>,
    user_systems: Vec<UserSystem>,
    pub file_watcher: FileWatcher,
    pub timer: GameTimer,
    pub input: InputState,
    pub physics_config: PhysicsConfig,
    pub script_pool: ScriptInstancePool,
    pub assets: RenderAssets,
    pub registry: ComponentRegistry,
    pub world: World,
    pub config: RuntimeConfig,
}

impl RuntimeApp {
    /// Pre-window setup. Runs `register_components`, optionally loads the
    /// initial scene, then runs `build_scene` and `setup_systems`. Window /
    /// GPU init is deferred to the winit `resumed` callback inside `run()`.
    pub fn new(mut config: RuntimeConfig) -> RuntimeResult<Self> {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        if let Some(setup) = config.register_components.take() {
            setup(&mut registry);
        }

        let mut world = World::new();
        if let Some(scene_path) = config.initial_scene.as_ref() {
            let _ = load_scene_via_vfs_or_disk(
                &mut world,
                &registry,
                scene_path,
                config.vfs.as_ref(),
            );
        }
        let scene_builder = config.build_scene.take();
        if let Some(build) = scene_builder {
            build(&mut world, &mut registry);
        }

        let mut app = Self {
            runtime_state: None,
            user_systems: Vec::new(),
            file_watcher: FileWatcher::new(Duration::from_millis(500)),
            timer: GameTimer::new(60),
            input: InputState::new(),
            physics_config: PhysicsConfig::default(),
            script_pool: ScriptInstancePool::new(),
            assets: RenderAssets::new(),
            registry,
            world,
            config,
        };
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

    pub fn user_system_count(&self) -> usize {
        self.user_systems.len()
    }

    /// Run one frame's worth of non-rendering systems. Used internally by the
    /// winit handler and directly by unit tests / headless harnesses that
    /// don't need a window.
    pub fn tick_systems_only(&mut self, dt: f32) {
        propagate_transforms(&mut self.world);
        if self.config.enable_physics {
            voltex_physics::physics_step(&mut self.world, &self.physics_config);
        }
        if self.config.enable_scripts {
            voltex_script::script_start_all(&mut self.world, &mut self.script_pool);
            voltex_script::script_update_all(&mut self.world, &mut self.script_pool, dt);
            let _ = self.script_pool.poll_reload(&mut self.file_watcher);
        }
        for sys in self.user_systems.iter_mut() {
            sys(&mut self.world, dt);
        }
    }

    /// Take over the calling thread with a winit event loop until the window
    /// is closed. GPU resources are constructed lazily on the first `resumed`
    /// callback, so `tick_systems_only` may be called before this.
    pub fn run(self) -> RuntimeResult<()> {
        let event_loop = winit::event_loop::EventLoop::new()
            .map_err(|e| RuntimeError::WindowInit(e.to_string()))?;
        let mut handler = crate::handler::RuntimeHandler { app: Some(self) };
        event_loop
            .run_app(&mut handler)
            .map_err(|e| RuntimeError::WindowInit(e.to_string()))?;
        Ok(())
    }
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
}
