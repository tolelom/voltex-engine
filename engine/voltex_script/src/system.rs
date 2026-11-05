//! Per-frame integration between ECS `ScriptComponent` and the Lua pool.
//!
//! Two entry points the runtime / editor calls:
//!
//! - `script_start_all(world, pool)` — invoked once per PlayStart (or whenever
//!   a new ScriptComponent first becomes visible). For each component whose
//!   `instance_id == UNBOUND` or whose id no longer maps, the path is resolved
//!   into a fresh instance and `on_start(id, gen)` is fired.
//! - `script_update_all(world, pool, dt)` — every frame. For each component
//!   whose instance has already been started, fire `on_update(id, gen, dt)`.

use std::ffi::CString;
use voltex_ecs::{Entity, ScriptComponent, World};

use crate::ffi;
use crate::instance::ScriptInstance;
use crate::pool::ScriptInstancePool;
use crate::state::LuaState;

/// A Lua error raised inside a script callback.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptCallbackError {
    /// Entity whose callback failed.
    pub entity: Entity,
    /// `"on_start"` or `"on_update"`.
    pub callback: &'static str,
    /// Path of the script that raised the error.
    pub script_path: String,
    /// Lua error message.
    pub message: String,
}

/// Runs one callback and returns its error only if it differs from the last
/// error this instance reported (so a script failing every frame is reported once).
fn run_callback(
    inst: &mut ScriptInstance,
    callback: &'static str,
    entity: Entity,
    dt: Option<f32>,
) -> Option<ScriptCallbackError> {
    let message = invoke_callback(&inst.lua, callback, entity, dt).err()?;
    if inst.last_error.as_deref() == Some(message.as_str()) {
        return None;
    }
    inst.last_error = Some(message.clone());
    Some(ScriptCallbackError { entity, callback, script_path: inst.path.clone(), message })
}

/// Resolve every ScriptComponent to a live `ScriptInstance` (creating one if
/// necessary), then fire `on_start(id, gen)` for any instance that has not
/// yet been started. Returns the Lua errors raised this call (each distinct
/// error once; see [`ScriptCallbackError`]).
pub fn script_start_all(world: &mut World, pool: &mut ScriptInstancePool) -> Vec<ScriptCallbackError> {
    let mut errors = Vec::new();
    let world_ptr = world as *mut World;
    let entries: Vec<(Entity, String, u32)> = world
        .query::<ScriptComponent>()
        .map(|(e, s)| (e, s.script_path.clone(), s.instance_id))
        .collect();

    for (entity, path, cached_id) in entries {
        let mut instance_id = cached_id;
        if instance_id == ScriptComponent::UNBOUND || pool.get(instance_id).is_none() {
            instance_id = match pool.get_or_create(&path, world_ptr) {
                Ok(id) => id,
                Err(_) => continue,
            };
            if let Some(s) = world.get_mut::<ScriptComponent>(entity) {
                s.instance_id = instance_id;
            }
        }
        let Some(inst) = pool.get_mut(instance_id) else { continue };
        if inst.started {
            continue;
        }
        errors.extend(run_callback(inst, "on_start", entity, None));
        inst.started = true;
    }
    errors
}

/// Fire `on_update(id, gen, dt)` for every ScriptComponent whose backing
/// instance is `started`. Unstarted instances are skipped — call
/// `script_start_all` first. Returns the Lua errors raised this call (each
/// distinct error once; see [`ScriptCallbackError`]).
pub fn script_update_all(
    world: &mut World,
    pool: &mut ScriptInstancePool,
    dt: f32,
) -> Vec<ScriptCallbackError> {
    let mut errors = Vec::new();
    let entries: Vec<(Entity, u32)> = world
        .query::<ScriptComponent>()
        .map(|(e, s)| (e, s.instance_id))
        .collect();

    for (entity, instance_id) in entries {
        let Some(inst) = pool.get_mut(instance_id) else { continue };
        if !inst.started {
            continue;
        }
        errors.extend(run_callback(inst, "on_update", entity, Some(dt)));
    }
    errors
}

/// Call the global Lua function `fn_name(id, gen[, dt])` if the script defines
/// one. A missing function is a no-op; a Lua error is returned as its message.
fn invoke_callback(
    lua: &LuaState,
    fn_name: &str,
    entity: Entity,
    dt: Option<f32>,
) -> Result<(), String> {
    // SAFETY: `lua.raw()` is the live state owned by `lua`; `fn_name_c`
    // outlives the getglobal call; every push is balanced by the pcall (which
    // pops function + args) or by the explicit pops below. The error string
    // returned by `lua_tostring` is copied before its stack slot is popped.
    unsafe {
        let L = lua.raw();
        let fn_name_c = match CString::new(fn_name) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let ty = ffi::lua_getglobal(L, fn_name_c.as_ptr());
        if ty != ffi::LUA_TFUNCTION {
            ffi::lua_pop(L, 1);
            return Ok(());
        }
        ffi::lua_pushnumber(L, entity.id as f64);
        ffi::lua_pushnumber(L, entity.generation as f64);
        let nargs = if let Some(dt) = dt {
            ffi::lua_pushnumber(L, dt as f64);
            3
        } else {
            2
        };
        let rc = ffi::lua_pcall(L, nargs, 0, 0);
        if rc == ffi::LUA_OK {
            return Ok(());
        }
        // pcall left the error value on the stack: copy it out, then pop it.
        let message_ptr = ffi::lua_tostring(L, -1);
        let message = if message_ptr.is_null() {
            "(non-string Lua error)".to_string()
        } else {
            std::ffi::CStr::from_ptr(message_ptr).to_string_lossy().into_owned()
        };
        ffi::lua_pop(L, 1);
        Err(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::{Transform, World};
    use voltex_math::Vec3;

    fn write_tmp_lua(name: &str, body: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("voltex_script_system_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn start_all_invokes_on_start_and_sets_flag() {
        let path = write_tmp_lua(
            "marker.lua",
            "
            started_count = 0
            function on_start(id, gen)
                started_count = started_count + 1
            end
            ",
        );

        let mut world = World::new();
        let e = world.spawn();
        world.add(
            e,
            ScriptComponent::new(path.to_str().unwrap().to_string()),
        );

        let mut pool = ScriptInstancePool::new();
        script_start_all(&mut world, &mut pool);

        let s = world.get::<ScriptComponent>(e).unwrap();
        assert_ne!(s.instance_id, ScriptComponent::UNBOUND);
        let inst = pool.get(s.instance_id).unwrap();
        assert!(inst.started);
        assert_eq!(inst.lua.get_global_number("started_count"), Some(1.0));
    }

    #[test]
    fn update_all_calls_on_update_for_started_only() {
        let path = write_tmp_lua(
            "counter.lua",
            "
            tick_total = 0
            function on_start(id, gen) tick_total = 0 end
            function on_update(id, gen, dt) tick_total = tick_total + dt end
            ",
        );

        let mut world = World::new();
        let e = world.spawn();
        world.add(
            e,
            ScriptComponent::new(path.to_str().unwrap().to_string()),
        );

        let mut pool = ScriptInstancePool::new();

        // Without start, update is a no-op — instance_id still UNBOUND, no
        // instance, nothing to call.
        script_update_all(&mut world, &mut pool, 1.0);
        assert!(pool.is_empty());

        // After start, three updates of dt=0.5 → tick_total = 1.5.
        script_start_all(&mut world, &mut pool);
        script_update_all(&mut world, &mut pool, 0.5);
        script_update_all(&mut world, &mut pool, 0.5);
        script_update_all(&mut world, &mut pool, 0.5);
        let s = world.get::<ScriptComponent>(e).unwrap();
        let inst = pool.get(s.instance_id).unwrap();
        let total = inst.lua.get_global_number("tick_total").unwrap();
        assert!((total - 1.5).abs() < 1e-6, "tick_total = {}", total);
    }

    #[test]
    fn five_cubes_rotate_via_iter_by_tag() {
        use voltex_ecs::scene::Tag;
        let path = write_tmp_lua(
            "spinner_5.lua",
            "
            function on_start(id, gen)
                set_rotation(id, gen, 0.0, 0.0, 0.0)
            end
            function on_update(id, gen, dt)
                local rx, ry, rz = get_rotation(id, gen)
                if ry ~= nil then
                    set_rotation(id, gen, rx, ry + dt, rz)
                end
            end
            ",
        );

        let mut world = World::new();
        let mut cubes = Vec::new();
        for _ in 0..5 {
            let e = world.spawn();
            world.add(e, Transform::from_position(Vec3::ZERO));
            world.add(e, Tag("spinner".to_string()));
            world.add(
                e,
                ScriptComponent::new(path.to_str().unwrap().to_string()),
            );
            cubes.push(e);
        }

        let mut pool = ScriptInstancePool::new();
        script_start_all(&mut world, &mut pool);

        for _ in 0..3 {
            script_update_all(&mut world, &mut pool, 0.1);
        }

        for e in cubes {
            let t = world.get::<Transform>(e).unwrap();
            // After 3 ticks of dt=0.1 the script accumulated yaw=0.3 rad via
            // get_rotation/set_rotation. Since E3b the Lua API still talks
            // YXZ Euler, but the stored value is a quaternion — compare in
            // quat space to avoid relying on the Vec3-style field meaning.
            let expected =
                voltex_math::Quat::from_euler_yxz(Vec3::new(0.0, 0.3, 0.0));
            assert!(
                (t.rotation.x - expected.x).abs() < 1e-5
                    && (t.rotation.y - expected.y).abs() < 1e-5
                    && (t.rotation.z - expected.z).abs() < 1e-5
                    && (t.rotation.w - expected.w).abs() < 1e-5,
                "expected rotation ≈ from_euler(0, 0.3, 0), got {:?}",
                t.rotation
            );
        }
    }

    #[test]
    fn missing_on_start_is_no_op() {
        let path = write_tmp_lua("no_callbacks.lua", "globals_only = 42");

        let mut world = World::new();
        let e = world.spawn();
        world.add(
            e,
            ScriptComponent::new(path.to_str().unwrap().to_string()),
        );

        let mut pool = ScriptInstancePool::new();
        // Must not panic even though `on_start` / `on_update` are absent.
        script_start_all(&mut world, &mut pool);
        script_update_all(&mut world, &mut pool, 0.016);
        let s = world.get::<ScriptComponent>(e).unwrap();
        let inst = pool.get(s.instance_id).unwrap();
        assert!(inst.started);
        assert_eq!(inst.lua.get_global_number("globals_only"), Some(42.0));
    }

    #[test]
    fn lua_error_in_on_update_is_returned_once() {
        let path = write_tmp_lua(
            "update_error.lua",
            "
            function on_update(id, gen, dt)
                error('boom in update')
            end
            ",
        );
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, ScriptComponent::new(path.to_str().unwrap().to_string()));
        let mut pool = ScriptInstancePool::new();

        assert!(script_start_all(&mut world, &mut pool).is_empty());
        let first = script_update_all(&mut world, &mut pool, 0.016);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].entity, e);
        assert_eq!(first[0].callback, "on_update");
        assert!(first[0].message.contains("boom in update"), "{}", first[0].message);

        // Same error next frame is not reported again.
        assert!(script_update_all(&mut world, &mut pool, 0.016).is_empty());
    }

    #[test]
    fn lua_error_in_on_start_is_returned() {
        let path = write_tmp_lua(
            "start_error.lua",
            "
            function on_start(id, gen)
                error('boom in start')
            end
            ",
        );
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, ScriptComponent::new(path.to_str().unwrap().to_string()));
        let mut pool = ScriptInstancePool::new();

        let errors = script_start_all(&mut world, &mut pool);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].callback, "on_start");
        assert!(errors[0].message.contains("boom in start"));
    }
}
