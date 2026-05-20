//! Engine API exposed to Lua scripts.
//!
//! ## Entity handle contract
//!
//! Entities are passed across the FFI boundary as **two integers**: `(id, gen)`.
//! Both must be supplied together so the engine can reject stale handles after
//! the underlying entity has been despawned and its id reused. Passing only the
//! id (with an implicit `gen=0`) is unsafe and was the source of an ABA bug
//! prior to this rewrite.
//!
//! ```lua
//! local id, gen = spawn_entity()
//! set_position(id, gen, 5.0, 10.0, 15.0)
//! local x, y, z = get_position(id, gen)
//! ```

use crate::ffi;
use crate::state::LuaState;
use voltex_ecs::World;
use voltex_ecs::Transform;
use voltex_math::Vec3;
use std::os::raw::c_int;

/// Register engine API functions into the Lua state.
/// The World pointer is stored in a global "__voltex_world" as a light userdata.
pub fn register_engine_api(lua: &LuaState, world: *mut World) {
    unsafe {
        ffi::lua_pushlightuserdata(lua.raw(), world as *mut std::ffi::c_void);
        let name = std::ffi::CString::new("__voltex_world").unwrap();
        ffi::lua_setglobal(lua.raw(), name.as_ptr());
    }

    lua.register_fn("spawn_entity", lua_spawn_entity);
    lua.register_fn("destroy_entity", lua_destroy_entity);
    lua.register_fn("set_position", lua_set_position);
    lua.register_fn("get_position", lua_get_position);
    lua.register_fn("set_rotation", lua_set_rotation);
    lua.register_fn("get_rotation", lua_get_rotation);
    lua.register_fn("set_scale", lua_set_scale);
    lua.register_fn("get_scale", lua_get_scale);
    lua.register_fn("set_tag", lua_set_tag);
    lua.register_fn("get_tag", lua_get_tag);
    lua.register_fn("find_by_tag", lua_find_by_tag);
    lua.register_fn("entity_count", lua_entity_count);
}

/// Helper to retrieve the World pointer from the Lua registry global.
unsafe fn get_world(L: *mut ffi::lua_State) -> &'static mut World {
    let name = std::ffi::CString::new("__voltex_world").unwrap();
    ffi::lua_getglobal(L, name.as_ptr());
    let ptr = ffi::lua_touserdata(L, -1) as *mut World;
    ffi::lua_pop(L, 1);
    &mut *ptr
}

/// `spawn_entity()` → `id, gen`
unsafe extern "C" fn lua_spawn_entity(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let entity = world.spawn();
    world.add(entity, Transform::new());
    ffi::lua_pushnumber(L, entity.id as f64);
    ffi::lua_pushnumber(L, entity.generation as f64);
    2
}

/// `set_position(id, gen, x, y, z)` — writes only if the handle is still alive.
unsafe extern "C" fn lua_set_position(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let mut isnum = 0;
    let id = ffi::lua_tonumberx(L, 1, &mut isnum) as u32;
    let generation = ffi::lua_tonumberx(L, 2, &mut isnum) as u32;
    let x = ffi::lua_tonumberx(L, 3, &mut isnum) as f32;
    let y = ffi::lua_tonumberx(L, 4, &mut isnum) as f32;
    let z = ffi::lua_tonumberx(L, 5, &mut isnum) as f32;

    let entity = voltex_ecs::Entity { id, generation };
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(t) = world.get_mut::<Transform>(entity) {
        t.position = Vec3::new(x, y, z);
    }
    0
}

/// `get_position(id, gen)` → `x, y, z` (or nothing if handle stale / no Transform).
unsafe extern "C" fn lua_get_position(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let mut isnum = 0;
    let id = ffi::lua_tonumberx(L, 1, &mut isnum) as u32;
    let generation = ffi::lua_tonumberx(L, 2, &mut isnum) as u32;

    let entity = voltex_ecs::Entity { id, generation };
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(t) = world.get::<Transform>(entity) {
        ffi::lua_pushnumber(L, t.position.x as f64);
        ffi::lua_pushnumber(L, t.position.y as f64);
        ffi::lua_pushnumber(L, t.position.z as f64);
        3
    } else {
        0
    }
}

unsafe extern "C" fn lua_entity_count(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    ffi::lua_pushnumber(L, world.entity_count() as f64);
    1
}

/// Helper: extract `(id, gen)` from Lua args at positions 1, 2.
unsafe fn read_handle(L: *mut ffi::lua_State) -> voltex_ecs::Entity {
    let mut isnum = 0;
    let id = ffi::lua_tonumberx(L, 1, &mut isnum) as u32;
    let generation = ffi::lua_tonumberx(L, 2, &mut isnum) as u32;
    voltex_ecs::Entity { id, generation }
}

/// `destroy_entity(id, gen)` — despawns the entity. No-op on stale handles.
unsafe extern "C" fn lua_destroy_entity(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let entity = read_handle(L);
    if world.is_alive(entity) {
        world.despawn(entity);
    }
    0
}

/// `set_rotation(id, gen, x, y, z)`
unsafe extern "C" fn lua_set_rotation(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let entity = read_handle(L);
    let mut isnum = 0;
    let x = ffi::lua_tonumberx(L, 3, &mut isnum) as f32;
    let y = ffi::lua_tonumberx(L, 4, &mut isnum) as f32;
    let z = ffi::lua_tonumberx(L, 5, &mut isnum) as f32;
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(t) = world.get_mut::<Transform>(entity) {
        t.rotation = Vec3::new(x, y, z);
    }
    0
}

/// `get_rotation(id, gen)` → `x, y, z` (or nothing if stale).
unsafe extern "C" fn lua_get_rotation(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let entity = read_handle(L);
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(t) = world.get::<Transform>(entity) {
        ffi::lua_pushnumber(L, t.rotation.x as f64);
        ffi::lua_pushnumber(L, t.rotation.y as f64);
        ffi::lua_pushnumber(L, t.rotation.z as f64);
        3
    } else {
        0
    }
}

/// `set_scale(id, gen, x, y, z)`
unsafe extern "C" fn lua_set_scale(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let entity = read_handle(L);
    let mut isnum = 0;
    let x = ffi::lua_tonumberx(L, 3, &mut isnum) as f32;
    let y = ffi::lua_tonumberx(L, 4, &mut isnum) as f32;
    let z = ffi::lua_tonumberx(L, 5, &mut isnum) as f32;
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(t) = world.get_mut::<Transform>(entity) {
        t.scale = Vec3::new(x, y, z);
    }
    0
}

/// `get_scale(id, gen)` → `x, y, z` (or nothing if stale).
unsafe extern "C" fn lua_get_scale(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let entity = read_handle(L);
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(t) = world.get::<Transform>(entity) {
        ffi::lua_pushnumber(L, t.scale.x as f64);
        ffi::lua_pushnumber(L, t.scale.y as f64);
        ffi::lua_pushnumber(L, t.scale.z as f64);
        3
    } else {
        0
    }
}

/// `set_tag(id, gen, tag_string)` — replaces any existing Tag.
unsafe extern "C" fn lua_set_tag(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let entity = read_handle(L);
    if !world.is_alive(entity) {
        return 0;
    }
    let cstr = ffi::lua_tostring(L, 3);
    if cstr.is_null() {
        return 0;
    }
    let tag = std::ffi::CStr::from_ptr(cstr).to_string_lossy().into_owned();
    world.remove::<voltex_ecs::scene::Tag>(entity);
    world.add(entity, voltex_ecs::scene::Tag(tag));
    0
}

/// `get_tag(id, gen)` → tag string (or nothing if stale / no Tag).
unsafe extern "C" fn lua_get_tag(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let entity = read_handle(L);
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(tag) = world.get::<voltex_ecs::scene::Tag>(entity) {
        let cstr = std::ffi::CString::new(tag.0.as_str()).unwrap_or_default();
        ffi::lua_pushstring(L, cstr.as_ptr());
        1
    } else {
        0
    }
}

/// `find_by_tag(tag_string)` → `id, gen` of the first entity with that Tag,
/// or nothing if no match. The returned handle is fresh (valid generation).
unsafe extern "C" fn lua_find_by_tag(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let cstr = ffi::lua_tostring(L, 1);
    if cstr.is_null() {
        return 0;
    }
    let needle = std::ffi::CStr::from_ptr(cstr).to_string_lossy().into_owned();
    for (entity, tag) in world.query::<voltex_ecs::scene::Tag>() {
        if tag.0 == needle {
            ffi::lua_pushnumber(L, entity.id as f64);
            ffi::lua_pushnumber(L, entity.generation as f64);
            return 2;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::LuaState;

    #[test]
    fn spawn_returns_id_and_generation_pair() {
        let lua = LuaState::new();
        let mut world = World::new();
        register_engine_api(&lua, &mut world as *mut World);

        lua.exec("
            local id, gen = spawn_entity()
            spawned_id = id
            spawned_gen = gen
        ").unwrap();

        assert_eq!(lua.get_global_number("spawned_id"), Some(0.0));
        assert_eq!(lua.get_global_number("spawned_gen"), Some(0.0));
    }

    #[test]
    fn set_and_get_position_roundtrip_with_handle() {
        let lua = LuaState::new();
        let mut world = World::new();
        register_engine_api(&lua, &mut world as *mut World);

        lua.exec("
            local id, gen = spawn_entity()
            set_position(id, gen, 5.0, 10.0, 15.0)
            local x, y, z = get_position(id, gen)
            result_x = x
            result_y = y
            result_z = z
        ").unwrap();

        assert_eq!(lua.get_global_number("result_x"), Some(5.0));
        assert_eq!(lua.get_global_number("result_y"), Some(10.0));
        assert_eq!(lua.get_global_number("result_z"), Some(15.0));
    }

    #[test]
    fn entity_count_works() {
        let lua = LuaState::new();
        let mut world = World::new();
        register_engine_api(&lua, &mut world as *mut World);

        lua.exec("
            spawn_entity()
            spawn_entity()
            count = entity_count()
        ").unwrap();

        assert_eq!(lua.get_global_number("count"), Some(2.0));
    }

    #[test]
    fn stale_handle_after_despawn_is_rejected() {
        // Reproduces the ABA bug: spawn id=0, despawn it (gen bumps to 1),
        // re-spawn → new entity at id=0,gen=1. The OLD handle (id=0,gen=0)
        // must NOT mutate the new entity.
        let lua = LuaState::new();
        let mut world = World::new();
        register_engine_api(&lua, &mut world as *mut World);

        // Step 1: spawn and despawn from Rust side so we can capture the stale handle.
        let first = world.spawn();
        world.add(first, Transform::new());
        assert_eq!(first.id, 0);
        assert_eq!(first.generation, 0);
        world.despawn(first);

        // Step 2: Lua sees the new entity at (id=0, gen=1). Try to set position
        // with the STALE (id=0, gen=0) handle — must be a no-op.
        lua.exec("
            local id, gen = spawn_entity()   -- id=0, gen=1
            new_id = id
            new_gen = gen
            set_position(0, 0, 99.0, 99.0, 99.0)   -- stale write
            local x, y, z = get_position(id, gen)   -- correct read
            x_after_stale = x
        ").unwrap();

        assert_eq!(lua.get_global_number("new_id"), Some(0.0));
        assert_eq!(lua.get_global_number("new_gen"), Some(1.0));
        // The stale set_position must NOT have written 99; the new entity
        // still has its default (0, 0, 0) position.
        assert_eq!(lua.get_global_number("x_after_stale"), Some(0.0));
    }

    #[test]
    fn destroy_entity_makes_handle_stale() {
        let lua = LuaState::new();
        let mut world = World::new();
        register_engine_api(&lua, &mut world as *mut World);

        lua.exec("
            local id, gen = spawn_entity()
            destroy_entity(id, gen)
            -- After destroy, the handle is stale; get_position returns nothing.
            local x = get_position(id, gen)
            after_destroy = x
            count = entity_count()
        ").unwrap();

        assert_eq!(lua.get_global_number("after_destroy"), None);
        assert_eq!(lua.get_global_number("count"), Some(0.0));
    }

    #[test]
    fn rotation_and_scale_roundtrip() {
        let lua = LuaState::new();
        let mut world = World::new();
        register_engine_api(&lua, &mut world as *mut World);

        lua.exec("
            local id, gen = spawn_entity()
            set_rotation(id, gen, 0.5, 1.0, 1.5)
            set_scale(id, gen, 2.0, 3.0, 4.0)
            local rx, ry, rz = get_rotation(id, gen)
            local sx, sy, sz = get_scale(id, gen)
            r_x, r_y, r_z = rx, ry, rz
            s_x, s_y, s_z = sx, sy, sz
        ").unwrap();

        assert_eq!(lua.get_global_number("r_x"), Some(0.5));
        assert_eq!(lua.get_global_number("r_z"), Some(1.5));
        assert_eq!(lua.get_global_number("s_x"), Some(2.0));
        assert_eq!(lua.get_global_number("s_z"), Some(4.0));
    }

    #[test]
    fn tag_set_get_and_find_by_tag() {
        let lua = LuaState::new();
        let mut world = World::new();
        register_engine_api(&lua, &mut world as *mut World);

        lua.exec("
            local a_id, a_gen = spawn_entity()
            local b_id, b_gen = spawn_entity()
            set_tag(a_id, a_gen, 'Hero')
            set_tag(b_id, b_gen, 'Enemy')

            found_a_tag = get_tag(a_id, a_gen)

            local id, gen = find_by_tag('Enemy')
            found_b_id = id
            found_b_gen = gen
        ").unwrap();

        assert_eq!(lua.get_global_string("found_a_tag").as_deref(), Some("Hero"));
        assert_eq!(lua.get_global_number("found_b_id"), Some(1.0));
        assert_eq!(lua.get_global_number("found_b_gen"), Some(0.0));
    }

    #[test]
    fn find_by_tag_returns_nothing_when_no_match() {
        let lua = LuaState::new();
        let mut world = World::new();
        register_engine_api(&lua, &mut world as *mut World);

        lua.exec("
            local id, gen = find_by_tag('Missing')
            missing_id = id
        ").unwrap();

        assert_eq!(lua.get_global_number("missing_id"), None);
    }

    #[test]
    fn stale_handle_get_position_returns_nothing() {
        let lua = LuaState::new();
        let mut world = World::new();
        register_engine_api(&lua, &mut world as *mut World);

        let first = world.spawn();
        world.add(first, Transform::new());
        world.despawn(first);

        // Lua reads from stale handle → multiple-return yields nil.
        lua.exec("
            local x = get_position(0, 0)
            stale_read = x   -- should be nil (no return values), Lua sets to nil
        ").unwrap();

        // get_global_number returns None for nil globals.
        assert_eq!(lua.get_global_number("stale_read"), None);
    }
}
