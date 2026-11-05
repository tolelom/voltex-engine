//! Registration API for per-frame systems and callbacks (`add_*_system`,
//! `set_*_fn`) plus the boxed closure types the app stores them as.

use voltex_ecs::{Entity, World};
use voltex_editor::UiContext;
use voltex_physics::ContactPoint;
use voltex_platform::{InputMap, InputState};
use voltex_renderer::{Camera, LightUniform, PointLight};

use super::RuntimeApp;
use crate::debug_draw::DebugLines;
use crate::time::Time;

pub(super) type UserSystem = Box<dyn FnMut(&mut World, f32) + 'static>;
/// User system that also gets read-only access to the per-frame InputState.
/// Added in D3 so a flagship game can drive entities from keyboard / mouse
/// without requiring a full `InputState as ECS resource` abstraction (v1.1).
pub(super) type InputSystem = Box<dyn FnMut(&mut World, &InputState, f32) + 'static>;
/// User system that reads semantic actions/axes via the [`InputMap`] layer in
/// addition to the raw [`InputState`]. Lets a game rebind controls in one place
/// (`app.input_map`) and drive entities by action name.
pub(super) type ActionSystem = Box<dyn FnMut(&mut World, &InputState, &InputMap, f32) + 'static>;
/// User system that receives the runtime [`Time`] (accumulated elapsed seconds,
/// per-frame delta, frame counter) instead of just a bare `dt`. For timers,
/// cooldowns, and animation phase that need wallclock-since-start.
pub(super) type TimedSystem = Box<dyn FnMut(&mut World, &Time) + 'static>;
/// HUD callback invoked each frame between the 3D pass and present, with the
/// immediate-mode [`UiContext`] (already inside `begin_frame`/`end_frame`) and
/// read-only access to the World. Draw text / bars / panels here.
pub(super) type HudFn = Box<dyn FnMut(&mut UiContext, &World) + 'static>;
/// Per-frame camera updater (see [`RuntimeApp::set_camera_fn`]). Runs after the
/// systems tick, with the live [`Camera`] and read-only World, so games can
/// implement follow / orbit / cinematic cameras. Overrides the default RMB
/// fly-camera for the frame.
pub(super) type CameraFn = Box<dyn FnMut(&mut Camera, &World, f32) + 'static>;
/// Per-frame directional-light updater (see [`RuntimeApp::set_light_fn`]). Runs
/// after the systems tick with the live [`LightUniform`] and read-only World,
/// for day/night cycles, flicker, etc.
pub(super) type LightFn = Box<dyn FnMut(&mut LightUniform, &World, f32) + 'static>;
/// Per-frame point/spot-light updater (see [`RuntimeApp::set_point_lights_fn`]).
/// Runs after the systems tick with the live point-light `Vec` and read-only
/// World, for moving torches, flickering lamps, projectile glows, etc. The
/// resulting set is uploaded to the renderer (capped at `MAX_LIGHTS`) the same
/// way a static [`RuntimeApp::set_point_lights`] call would be.
pub(super) type PointLightsFn = Box<dyn FnMut(&mut Vec<PointLight>, &World, f32) + 'static>;
/// Per-frame collision callback (see [`RuntimeApp::set_collision_fn`]). Receives
/// the frame's contact pairs (between entities carrying a
/// [`Collider`](voltex_physics::Collider) and a Transform) plus the mutable
/// World, for trigger/hit reactions. Runs regardless of `enable_physics`, so it
/// doubles as a trigger-zone detector.
pub(super) type CollisionFn = Box<dyn FnMut(&[ContactPoint], &mut World, f32) + 'static>;
/// Per-frame debug-draw callback (see [`RuntimeApp::set_debug_draw_fn`]). The
/// passed [`DebugLines`] is cleared each frame before the call; whatever the
/// game queues is rendered as `LineList` gizmos over the 3D scene.
pub(super) type DebugFn = Box<dyn FnMut(&mut DebugLines, &World, f32) + 'static>;

impl RuntimeApp {
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
}
