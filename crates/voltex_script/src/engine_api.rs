use crate::ffi;
use crate::state::LuaState;
use voltex_ecs::World;
use voltex_ecs::Transform;
use voltex_math::Vec3;
use std::os::raw::c_int;

/// Register engine API functions into the Lua state.
/// The World pointer is stored in a global "__voltex_world" as a light userdata.
pub fn register_engine_api(lua: &LuaState, world: *mut World) {
    // Store world pointer as light userdata in global
    unsafe {
        ffi::lua_pushlightuserdata(lua.raw(), world as *mut std::ffi::c_void);
        let name = std::ffi::CString::new("__voltex_world").unwrap();
        ffi::lua_setglobal(lua.raw(), name.as_ptr());
    }

    lua.register_fn("spawn_entity", lua_spawn_entity);
    lua.register_fn("set_position", lua_set_position);
    lua.register_fn("get_position", lua_get_position);
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

unsafe extern "C" fn lua_spawn_entity(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let entity = world.spawn();
    world.add(entity, Transform::new());
    ffi::lua_pushnumber(L, entity.id as f64);
    1
}

unsafe extern "C" fn lua_set_position(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let mut isnum = 0;
    let id = ffi::lua_tonumberx(L, 1, &mut isnum) as u32;
    let x = ffi::lua_tonumberx(L, 2, &mut isnum) as f32;
    let y = ffi::lua_tonumberx(L, 3, &mut isnum) as f32;
    let z = ffi::lua_tonumberx(L, 4, &mut isnum) as f32;

    let entity = voltex_ecs::Entity { id, generation: 0 };
    if let Some(t) = world.get_mut::<Transform>(entity) {
        t.position = Vec3::new(x, y, z);
    }
    0
}

unsafe extern "C" fn lua_get_position(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    let mut isnum = 0;
    let id = ffi::lua_tonumberx(L, 1, &mut isnum) as u32;

    let entity = voltex_ecs::Entity { id, generation: 0 };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::LuaState;

    #[test]
    fn test_spawn_and_get_position() {
        let lua = LuaState::new();
        let mut world = World::new();
        register_engine_api(&lua, &mut world as *mut World);

        lua.exec("
            local id = spawn_entity()
            set_position(id, 5.0, 10.0, 15.0)
            local x, y, z = get_position(id)
            result_x = x
            result_y = y
            result_z = z
        ").unwrap();

        assert_eq!(lua.get_global_number("result_x"), Some(5.0));
        assert_eq!(lua.get_global_number("result_y"), Some(10.0));
        assert_eq!(lua.get_global_number("result_z"), Some(15.0));
    }

    #[test]
    fn test_entity_count() {
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
}
