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
    lua.register_fn("find_all_by_tag", lua_find_all_by_tag);
    lua.register_fn("add_child", lua_add_child);
    lua.register_fn("remove_child", lua_remove_child);
    lua.register_fn("entity_count", lua_entity_count);
    lua.register_fn("iter_entities", lua_iter_entities);
    lua.register_fn("iter_by_tag", lua_iter_by_tag);
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

/// `find_all_by_tag(tag_string)` → table (array) of `{id=, gen=}` for every
/// entity with that Tag. Returns an empty table if nothing matches (callers can
/// use `#result` to count). Always pushes exactly one Lua value.
unsafe extern "C" fn lua_find_all_by_tag(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let cstr = ffi::lua_tostring(L, 1);
    ffi::lua_createtable(L, 0, 0);
    if cstr.is_null() {
        return 1;
    }
    let needle = std::ffi::CStr::from_ptr(cstr).to_string_lossy().into_owned();

    let id_key = std::ffi::CString::new("id").unwrap();
    let gen_key = std::ffi::CString::new("gen").unwrap();
    let mut index: ffi::lua_Integer = 1;
    for (entity, tag) in world.query::<voltex_ecs::scene::Tag>() {
        if tag.0 == needle {
            ffi::lua_createtable(L, 0, 2);
            ffi::lua_pushnumber(L, entity.id as f64);
            ffi::lua_setfield(L, -2, id_key.as_ptr());
            ffi::lua_pushnumber(L, entity.generation as f64);
            ffi::lua_setfield(L, -2, gen_key.as_ptr());
            // outer table is now at -2, inner at -1 → assign and pop inner.
            ffi::lua_rawseti(L, -2, index);
            index += 1;
        }
    }
    1
}

/// `add_child(parent_id, parent_gen, child_id, child_gen)` — attaches `child`
/// under `parent`. No-op if either handle is stale.
unsafe extern "C" fn lua_add_child(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let mut isnum = 0;
    let parent = voltex_ecs::Entity {
        id: ffi::lua_tonumberx(L, 1, &mut isnum) as u32,
        generation: ffi::lua_tonumberx(L, 2, &mut isnum) as u32,
    };
    let child = voltex_ecs::Entity {
        id: ffi::lua_tonumberx(L, 3, &mut isnum) as u32,
        generation: ffi::lua_tonumberx(L, 4, &mut isnum) as u32,
    };
    if !world.is_alive(parent) || !world.is_alive(child) {
        return 0;
    }
    voltex_ecs::hierarchy::add_child(world, parent, child);
    0
}

/// `remove_child(parent_id, parent_gen, child_id, child_gen)` — detaches
/// `child` from `parent`. No-op if either handle is stale or not currently
/// attached.
unsafe extern "C" fn lua_remove_child(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let mut isnum = 0;
    let parent = voltex_ecs::Entity {
        id: ffi::lua_tonumberx(L, 1, &mut isnum) as u32,
        generation: ffi::lua_tonumberx(L, 2, &mut isnum) as u32,
    };
    let child = voltex_ecs::Entity {
        id: ffi::lua_tonumberx(L, 3, &mut isnum) as u32,
        generation: ffi::lua_tonumberx(L, 4, &mut isnum) as u32,
    };
    if !world.is_alive(parent) || !world.is_alive(child) {
        return 0;
    }
    voltex_ecs::hierarchy::remove_child(world, parent, child);
    0
}

/// `iter_entities()` → array table of `{id=, gen=}` for every currently live
/// entity (in id order). Empty table if the world is empty.
unsafe extern "C" fn lua_iter_entities(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    ffi::lua_createtable(L, 0, 0);
    let id_key = std::ffi::CString::new("id").unwrap();
    let gen_key = std::ffi::CString::new("gen").unwrap();
    let mut index: ffi::lua_Integer = 1;
    for entity in world.live_entities() {
        ffi::lua_createtable(L, 0, 2);
        ffi::lua_pushnumber(L, entity.id as f64);
        ffi::lua_setfield(L, -2, id_key.as_ptr());
        ffi::lua_pushnumber(L, entity.generation as f64);
        ffi::lua_setfield(L, -2, gen_key.as_ptr());
        ffi::lua_rawseti(L, -2, index);
        index += 1;
    }
    1
}

/// `iter_by_tag(tag)` → array table of `{id=, gen=}` for every entity carrying
/// that tag. Alias of `find_all_by_tag` — named for `for _, e in ipairs(...)`
/// readability.
unsafe extern "C" fn lua_iter_by_tag(L: *mut ffi::lua_State) -> c_int {
    lua_find_all_by_tag(L)
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
mod tests;
