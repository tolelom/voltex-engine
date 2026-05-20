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
use crate::pool::ScriptInstancePool;
use crate::state::LuaState;

/// Resolve every ScriptComponent to a live `ScriptInstance` (creating one if
/// necessary), then fire `on_start(id, gen)` for any instance that has not
/// yet been started.
pub fn script_start_all(world: &mut World, pool: &mut ScriptInstancePool) {
    let world_ptr = world as *mut World;
    let entries: Vec<(Entity, String, u32)> = world
        .query::<ScriptComponent>()
        .map(|(e, s)| (e, s.script_path.clone(), s.instance_id))
        .collect();

    for (entity, path, cached_id) in entries {
        let mut iid = cached_id;
        if iid == ScriptComponent::UNBOUND || pool.get(iid).is_none() {
            iid = match pool.get_or_create(&path, world_ptr) {
                Ok(id) => id,
                Err(_) => continue,
            };
            if let Some(s) = world.get_mut::<ScriptComponent>(entity) {
                s.instance_id = iid;
            }
        }
        let Some(inst) = pool.get_mut(iid) else { continue };
        if inst.started {
            continue;
        }
        invoke_callback(&inst.lua, "on_start", entity, None);
        inst.started = true;
    }
}

/// Fire `on_update(id, gen, dt)` for every ScriptComponent whose backing
/// instance is `started`. Unstarted instances are skipped — call
/// `script_start_all` first.
pub fn script_update_all(world: &mut World, pool: &mut ScriptInstancePool, dt: f32) {
    let entries: Vec<(Entity, u32)> = world
        .query::<ScriptComponent>()
        .map(|(e, s)| (e, s.instance_id))
        .collect();

    for (entity, iid) in entries {
        let Some(inst) = pool.get_mut(iid) else { continue };
        if !inst.started {
            continue;
        }
        invoke_callback(&inst.lua, "on_update", entity, Some(dt));
    }
}

fn invoke_callback(lua: &LuaState, fn_name: &str, entity: Entity, dt: Option<f32>) {
    unsafe {
        let l = lua.raw();
        let c = match CString::new(fn_name) {
            Ok(c) => c,
            Err(_) => return,
        };
        let ty = ffi::lua_getglobal(l, c.as_ptr());
        if ty != ffi::LUA_TFUNCTION {
            ffi::lua_pop(l, 1);
            return;
        }
        ffi::lua_pushnumber(l, entity.id as f64);
        ffi::lua_pushnumber(l, entity.generation as f64);
        let nargs = if let Some(dt) = dt {
            ffi::lua_pushnumber(l, dt as f64);
            3
        } else {
            2
        };
        let rc = ffi::lua_pcall(l, nargs, 0, 0);
        if rc != ffi::LUA_OK {
            // best-effort: pop the error message left on the stack
            ffi::lua_pop(l, 1);
        }
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
            assert!(
                (t.rotation.y - 0.3).abs() < 1e-5,
                "expected rotation.y ≈ 0.3, got {}",
                t.rotation.y
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
}
