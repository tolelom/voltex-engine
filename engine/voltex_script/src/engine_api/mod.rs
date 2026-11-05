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
//!
//! ## Safety model of the `lua_*` callbacks
//!
//! Every `unsafe extern "C" fn lua_*` below is only called by the Lua VM with
//! the live `L` of the state it was registered on, so the raw `ffi::lua_*`
//! calls on `L` are valid. World access goes through [`get_world`], which
//! trusts the `__voltex_world` pointer stored by [`register_engine_api`]
//! (see the invariant there).

use crate::ffi;
use crate::state::LuaState;
use voltex_ecs::SpatialGrid;
use voltex_ecs::Transform;
use voltex_ecs::World;
use voltex_math::{Quat, Vec3};
use std::os::raw::c_int;

/// Register engine API functions into the Lua state.
///
/// The World pointer is stored in the plain Lua global `__voltex_world` as a
/// light userdata (not the registry, so a script could overwrite it — the
/// callbacks then become no-ops). **Pointer invariant** (not checked, this fn
/// only stores it): `world` must be valid and not otherwise borrowed whenever
/// a script on `lua` calls one of these functions.
pub fn register_engine_api(lua: &LuaState, world: *mut World) {
    // SAFETY: `lua.raw()` is the live state owned by `lua`; the pushed value
    // is consumed by lua_setglobal.
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
    lua.register_fn("find_in_radius", lua_find_in_radius);
    lua.register_fn("iter_entities", lua_iter_entities);
    lua.register_fn("iter_by_tag", lua_iter_by_tag);
    // Gameplay-system bindings (nav / audio).
    lua.register_fn("nav_set_target", lua_nav_set_target);
    lua.register_fn("nav_clear_target", lua_nav_clear_target);
    lua.register_fn("nav_reached", lua_nav_reached);
    lua.register_fn("audio_play", lua_audio_play);
    lua.register_fn("audio_stop", lua_audio_stop);
}

/// Fetch the World pointer from the plain Lua global `__voltex_world`.
///
/// Returns `None` if the global is missing or not a light userdata. Pre-fix
/// this dereferenced a null pointer (UB → SIGSEGV) any time a script ran
/// before `register_engine_api` or accidentally reassigned the global.
///
/// # Safety
/// `L` must be a live state. The returned lifetime `'a` is unbounded (picked
/// by the caller): the reference is only sound for the duration of the
/// current Lua callback, while the invariant on [`register_engine_api`]
/// holds. Never store it.
unsafe fn get_world<'a>(L: *mut ffi::lua_State) -> Option<&'a mut World> {
    let name = std::ffi::CString::new("__voltex_world").unwrap();
    ffi::lua_getglobal(L, name.as_ptr());
    let ptr = ffi::lua_touserdata(L, -1) as *mut World;
    ffi::lua_pop(L, 1);
    if ptr.is_null() {
        None
    } else {
        Some(&mut *ptr)
    }
}

/// Helper: read a strict-typed `(id, gen)` pair from Lua args at positions
/// 1 and 2. Returns `None` if either argument is not a number — so callers
/// who pass strings/nils/tables get a silent no-op instead of accidentally
/// targeting entity (0, 0).
unsafe fn read_handle_strict(L: *mut ffi::lua_State) -> Option<voltex_ecs::Entity> {
    read_handle_strict_at(L, 1, 2)
}

/// Push an entity across the FFI boundary as two numbers `(id, gen)` (the
/// engine-wide handle contract). Returns the Lua return count (2).
unsafe fn push_entity(L: *mut ffi::lua_State, entity: voltex_ecs::Entity) -> c_int {
    ffi::lua_pushnumber(L, entity.id as f64);
    ffi::lua_pushnumber(L, entity.generation as f64);
    2
}

/// Push an entity as a `{ id =, gen = }` table, leaving it on top of the stack
/// (at -1) so the caller can `lua_rawseti` it into an array.
unsafe fn push_entity_table(L: *mut ffi::lua_State, entity: voltex_ecs::Entity) {
    let id_key = std::ffi::CString::new("id").unwrap();
    let gen_key = std::ffi::CString::new("gen").unwrap();
    ffi::lua_createtable(L, 0, 2);
    ffi::lua_pushnumber(L, entity.id as f64);
    ffi::lua_setfield(L, -2, id_key.as_ptr());
    ffi::lua_pushnumber(L, entity.generation as f64);
    ffi::lua_setfield(L, -2, gen_key.as_ptr());
}

/// `spawn_entity()` → `id, gen`
unsafe extern "C" fn lua_spawn_entity(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let entity = world.spawn();
    world.add(entity, Transform::new());
    push_entity(L, entity)
}

/// `set_position(id, gen, x, y, z)` — writes only if the handle is still alive.
unsafe extern "C" fn lua_set_position(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
    let mut isnum_x: c_int = 0;
    let mut isnum_y: c_int = 0;
    let mut isnum_z: c_int = 0;
    let x = ffi::lua_tonumberx(L, 3, &mut isnum_x) as f32;
    let y = ffi::lua_tonumberx(L, 4, &mut isnum_y) as f32;
    let z = ffi::lua_tonumberx(L, 5, &mut isnum_z) as f32;
    if isnum_x == 0 || isnum_y == 0 || isnum_z == 0 {
        return 0;
    }

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
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };

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
    let Some(world) = get_world(L) else { return 0 };
    ffi::lua_pushnumber(L, world.entity_count() as f64);
    1
}

/// `find_in_radius(x, y, z, r)` → array table of `{ id =, gen = }` for every
/// entity with a `Transform` whose position is within `r` of `(x, y, z)`.
/// Backed by a [`SpatialGrid`] built from the world's transforms (so Lua AI can
/// find nearby targets — enemies near the player, blast radii, pickups). Builds
/// the grid per call (O(n)); fine for the modest entity counts Lua games drive.
/// Non-numeric args (or a missing world) yield an empty table so `ipairs` is
/// always safe.
unsafe extern "C" fn lua_find_in_radius(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else {
        ffi::lua_createtable(L, 0, 0);
        return 1;
    };
    let mut isnum_x: c_int = 0;
    let mut isnum_y: c_int = 0;
    let mut isnum_z: c_int = 0;
    let mut isnum_r: c_int = 0;
    let x = ffi::lua_tonumberx(L, 1, &mut isnum_x) as f32;
    let y = ffi::lua_tonumberx(L, 2, &mut isnum_y) as f32;
    let z = ffi::lua_tonumberx(L, 3, &mut isnum_z) as f32;
    let r = ffi::lua_tonumberx(L, 4, &mut isnum_r) as f32;
    if isnum_x == 0 || isnum_y == 0 || isnum_z == 0 || isnum_r == 0 {
        ffi::lua_createtable(L, 0, 0);
        return 1;
    }

    // Cell size = radius keeps the query to a small cell neighbourhood.
    let grid = SpatialGrid::from_transforms(world, r);
    let hits = grid.query_radius(Vec3::new(x, y, z), r);

    ffi::lua_createtable(L, hits.len() as c_int, 0);
    for (index, entity) in (1 as ffi::lua_Integer..).zip(hits) {
        push_entity_table(L, entity);
        ffi::lua_rawseti(L, -2, index);
    }
    1
}

/// `destroy_entity(id, gen)` — despawns the entity. No-op on stale handles
/// or non-numeric args.
unsafe extern "C" fn lua_destroy_entity(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
    if world.is_alive(entity) {
        world.despawn(entity);
    }
    0
}

/// `set_rotation(id, gen, x, y, z)`
unsafe extern "C" fn lua_set_rotation(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
    let mut isnum_x: c_int = 0;
    let mut isnum_y: c_int = 0;
    let mut isnum_z: c_int = 0;
    let x = ffi::lua_tonumberx(L, 3, &mut isnum_x) as f32;
    let y = ffi::lua_tonumberx(L, 4, &mut isnum_y) as f32;
    let z = ffi::lua_tonumberx(L, 5, &mut isnum_z) as f32;
    if isnum_x == 0 || isnum_y == 0 || isnum_z == 0 {
        return 0;
    }
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(t) = world.get_mut::<Transform>(entity) {
        // Lua continues to expose Euler (YXZ radians) — convert at the FFI
        // boundary so existing scripts stay portable across the E3b change.
        t.rotation = Quat::from_euler_yxz(Vec3::new(x, y, z));
    }
    0
}

/// `get_rotation(id, gen)` → `x, y, z` (or nothing if stale).
unsafe extern "C" fn lua_get_rotation(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(t) = world.get::<Transform>(entity) {
        // Decompose the unit quat into the YXZ Euler triple. Gimbal-lock
        // ambiguity collapses (y, z) → (0, z') — matches the Inspector.
        let e = t.rotation.to_euler_yxz();
        ffi::lua_pushnumber(L, e.x as f64);
        ffi::lua_pushnumber(L, e.y as f64);
        ffi::lua_pushnumber(L, e.z as f64);
        3
    } else {
        0
    }
}

/// `set_scale(id, gen, x, y, z)`
unsafe extern "C" fn lua_set_scale(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
    let mut isnum_x: c_int = 0;
    let mut isnum_y: c_int = 0;
    let mut isnum_z: c_int = 0;
    let x = ffi::lua_tonumberx(L, 3, &mut isnum_x) as f32;
    let y = ffi::lua_tonumberx(L, 4, &mut isnum_y) as f32;
    let z = ffi::lua_tonumberx(L, 5, &mut isnum_z) as f32;
    if isnum_x == 0 || isnum_y == 0 || isnum_z == 0 {
        return 0;
    }
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
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
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
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
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
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
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
    let Some(world) = get_world(L) else {
        // Still push an empty table so callers can `ipairs` without error.
        ffi::lua_createtable(L, 0, 0);
        return 1;
    };
    let cstr = ffi::lua_tostring(L, 1);
    ffi::lua_createtable(L, 0, 0);
    if cstr.is_null() {
        return 1;
    }
    let needle = std::ffi::CStr::from_ptr(cstr).to_string_lossy().into_owned();

    let mut index: ffi::lua_Integer = 1;
    for (entity, tag) in world.query::<voltex_ecs::scene::Tag>() {
        if tag.0 == needle {
            // push_entity_table leaves the inner `{id=,gen=}` at -1, outer at -2.
            push_entity_table(L, entity);
            ffi::lua_rawseti(L, -2, index);
            index += 1;
        }
    }
    1
}

/// Helper: read a strict-typed `(id, gen)` pair from arbitrary slot positions.
unsafe fn read_handle_strict_at(L: *mut ffi::lua_State, id_pos: c_int, gen_pos: c_int) -> Option<voltex_ecs::Entity> {
    let mut isnum_id: c_int = 0;
    let mut isnum_gen: c_int = 0;
    let id = ffi::lua_tonumberx(L, id_pos, &mut isnum_id) as u32;
    let generation = ffi::lua_tonumberx(L, gen_pos, &mut isnum_gen) as u32;
    if isnum_id == 0 || isnum_gen == 0 {
        return None;
    }
    Some(voltex_ecs::Entity::from_raw_parts(id, generation))
}

/// `add_child(parent_id, parent_gen, child_id, child_gen)` — attaches `child`
/// under `parent`. No-op if either handle is stale or non-numeric.
unsafe extern "C" fn lua_add_child(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(parent) = read_handle_strict_at(L, 1, 2) else { return 0 };
    let Some(child) = read_handle_strict_at(L, 3, 4) else { return 0 };
    if !world.is_alive(parent) || !world.is_alive(child) {
        return 0;
    }
    voltex_ecs::hierarchy::add_child(world, parent, child);
    0
}

/// `remove_child(parent_id, parent_gen, child_id, child_gen)` — detaches
/// `child` from `parent`. No-op if either handle is stale, non-numeric, or
/// the child is not currently attached.
unsafe extern "C" fn lua_remove_child(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(parent) = read_handle_strict_at(L, 1, 2) else { return 0 };
    let Some(child) = read_handle_strict_at(L, 3, 4) else { return 0 };
    if !world.is_alive(parent) || !world.is_alive(child) {
        return 0;
    }
    voltex_ecs::hierarchy::remove_child(world, parent, child);
    0
}

/// `iter_entities()` → array table of `{id=, gen=}` for every currently live
/// entity (in id order). Empty table if the world is empty.
unsafe extern "C" fn lua_iter_entities(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else {
        ffi::lua_createtable(L, 0, 0);
        return 1;
    };
    ffi::lua_createtable(L, 0, 0);
    for (index, entity) in (1 as ffi::lua_Integer..).zip(world.live_entities()) {
        push_entity_table(L, entity);
        ffi::lua_rawseti(L, -2, index);
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
    let Some(world) = get_world(L) else { return 0 };
    let cstr = ffi::lua_tostring(L, 1);
    if cstr.is_null() {
        return 0;
    }
    let needle = std::ffi::CStr::from_ptr(cstr).to_string_lossy().into_owned();
    for (entity, tag) in world.query::<voltex_ecs::scene::Tag>() {
        if tag.0 == needle {
            return push_entity(L, entity);
        }
    }
    0
}


// ── Gameplay-system bindings ────────────────────────────────────────────────
// `engine_api` is the Lua surface for engine components, so it knows about
// `NavAgent` (voltex_ai) and `AudioSource` (voltex_audio) the same way it knows
// `Transform` / `Tag`. All are silent no-ops on stale handles / missing
// components / non-numeric args.

/// `nav_set_target(id, gen, x, y, z)` — set the entity's NavAgent goal.
unsafe extern "C" fn lua_nav_set_target(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
    let mut ix: c_int = 0;
    let mut iy: c_int = 0;
    let mut iz: c_int = 0;
    let x = ffi::lua_tonumberx(L, 3, &mut ix) as f32;
    let y = ffi::lua_tonumberx(L, 4, &mut iy) as f32;
    let z = ffi::lua_tonumberx(L, 5, &mut iz) as f32;
    if ix == 0 || iy == 0 || iz == 0 {
        return 0;
    }
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(agent) = world.get_mut::<voltex_ai::NavAgent>(entity) {
        agent.set_target(Vec3::new(x, y, z));
    }
    0
}

/// `nav_clear_target(id, gen)` — stop the entity's NavAgent (go idle).
unsafe extern "C" fn lua_nav_clear_target(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(agent) = world.get_mut::<voltex_ai::NavAgent>(entity) {
        agent.clear_target();
    }
    0
}

/// `nav_reached(id, gen)` → bool — `true` if the NavAgent reached its goal.
/// Returns `false` for stale handles / entities without a NavAgent.
unsafe extern "C" fn lua_nav_reached(L: *mut ffi::lua_State) -> c_int {
    let reached = (|| {
        let world = get_world(L)?;
        let entity = read_handle_strict(L)?;
        if !world.is_alive(entity) {
            return None;
        }
        world.get::<voltex_ai::NavAgent>(entity).map(|a| a.reached)
    })()
    .unwrap_or(false);
    ffi::lua_pushboolean(L, reached as c_int);
    1
}

/// `audio_play(id, gen)` — start the entity's AudioSource (one-shot or loop).
unsafe extern "C" fn lua_audio_play(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(src) = world.get_mut::<voltex_audio::AudioSource>(entity) {
        src.play();
    }
    0
}

/// `audio_stop(id, gen)` — stop the entity's AudioSource.
unsafe extern "C" fn lua_audio_stop(L: *mut ffi::lua_State) -> c_int {
    let Some(world) = get_world(L) else { return 0 };
    let Some(entity) = read_handle_strict(L) else { return 0 };
    if !world.is_alive(entity) {
        return 0;
    }
    if let Some(src) = world.get_mut::<voltex_audio::AudioSource>(entity) {
        src.stop();
    }
    0
}

#[cfg(test)]
mod tests;
