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
fn add_child_attaches_via_lua() {
    use voltex_ecs::hierarchy::{Children, Parent};
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);

    lua.exec("
        local p_id, p_gen = spawn_entity()
        local c_id, c_gen = spawn_entity()
        add_child(p_id, p_gen, c_id, c_gen)
        parent_id = p_id
        parent_gen = p_gen
        child_id = c_id
        child_gen = c_gen
    ").unwrap();

    let parent = voltex_ecs::Entity {
        id: lua.get_global_number("parent_id").unwrap() as u32,
        generation: lua.get_global_number("parent_gen").unwrap() as u32,
    };
    let child = voltex_ecs::Entity {
        id: lua.get_global_number("child_id").unwrap() as u32,
        generation: lua.get_global_number("child_gen").unwrap() as u32,
    };
    assert_eq!(world.get::<Parent>(child).map(|p| p.0), Some(parent));
    assert_eq!(world.get::<Children>(parent).map(|c| c.0.clone()), Some(vec![child]));
}

#[test]
fn remove_child_detaches_via_lua() {
    use voltex_ecs::hierarchy::Parent;
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);

    lua.exec("
        local p_id, p_gen = spawn_entity()
        local c_id, c_gen = spawn_entity()
        add_child(p_id, p_gen, c_id, c_gen)
        remove_child(p_id, p_gen, c_id, c_gen)
        child_id = c_id
        child_gen = c_gen
    ").unwrap();

    let child = voltex_ecs::Entity {
        id: lua.get_global_number("child_id").unwrap() as u32,
        generation: lua.get_global_number("child_gen").unwrap() as u32,
    };
    assert!(world.get::<Parent>(child).is_none());
}

#[test]
fn add_child_with_stale_handle_is_noop() {
    use voltex_ecs::hierarchy::Parent;
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);

    // Spawn one entity, capture its handle, then despawn from Rust to make
    // the Lua handle stale.
    let stale = world.spawn();
    world.add(stale, Transform::new());
    world.despawn(stale);

    lua.exec("
        local c_id, c_gen = spawn_entity()
        -- Try to attach the new child under the STALE parent (id=0, gen=0).
        add_child(0, 0, c_id, c_gen)
        child_id = c_id
        child_gen = c_gen
    ").unwrap();

    let child = voltex_ecs::Entity {
        id: lua.get_global_number("child_id").unwrap() as u32,
        generation: lua.get_global_number("child_gen").unwrap() as u32,
    };
    // Stale parent → add_child rejected; child has no Parent.
    assert!(world.get::<Parent>(child).is_none());
}

#[test]
fn find_all_by_tag_returns_all_matches() {
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);

    lua.exec("
        for i = 1, 3 do
            local id, gen = spawn_entity()
            set_tag(id, gen, 'Enemy')
        end
        local id, gen = spawn_entity()
        set_tag(id, gen, 'Hero')

        local enemies = find_all_by_tag('Enemy')
        enemy_count = #enemies
        first_id = enemies[1].id
        first_gen = enemies[1].gen
    ").unwrap();

    assert_eq!(lua.get_global_number("enemy_count"), Some(3.0));
    // The first matching entity is the first one we spawned (id=0, gen=0).
    assert_eq!(lua.get_global_number("first_id"), Some(0.0));
    assert_eq!(lua.get_global_number("first_gen"), Some(0.0));
}

#[test]
fn find_in_radius_returns_entities_within_range() {
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);

    lua.exec(
        "
        local a = {spawn_entity()}; set_position(a[1], a[2], 0.0, 0.0, 0.0)
        local b = {spawn_entity()}; set_position(b[1], b[2], 2.0, 0.0, 0.0)  -- within 3
        local c = {spawn_entity()}; set_position(c[1], c[2], 50.0, 0.0, 0.0) -- far away

        local near = find_in_radius(0.0, 0.0, 0.0, 3.0)
        near_count = #near
        has_handle = (near[1].id ~= nil and near[1].gen ~= nil) and 1 or 0
    ",
    )
    .unwrap();

    assert_eq!(lua.get_global_number("near_count"), Some(2.0), "a + b are within r=3");
    assert_eq!(lua.get_global_number("has_handle"), Some(1.0));
}

#[test]
fn find_in_radius_with_non_numeric_args_returns_empty_table() {
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);

    lua.exec(
        "
        local id, gen = spawn_entity(); set_position(id, gen, 0.0, 0.0, 0.0)
        local t = find_in_radius('nope', 0.0, 0.0, 3.0)
        empty_count = #t
    ",
    )
    .unwrap();

    assert_eq!(lua.get_global_number("empty_count"), Some(0.0));
}

// ── nav / audio bindings ────────────────────────────────────────────────────

#[test]
fn nav_set_target_sets_agent_goal() {
    let lua = LuaState::new();
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, voltex_ai::NavAgent::new(5.0));
    register_engine_api(&lua, &mut world as *mut World);

    lua.exec(&format!(
        "nav_set_target({}, {}, 3.0, 0.0, 4.0)",
        e.id, e.generation
    ))
    .unwrap();

    let t = world
        .get::<voltex_ai::NavAgent>(e)
        .unwrap()
        .target
        .expect("nav target should be set");
    assert!((t.x - 3.0).abs() < 1e-6 && (t.z - 4.0).abs() < 1e-6);
}

#[test]
fn nav_clear_target_and_reached_reflect_state() {
    let lua = LuaState::new();
    let mut world = World::new();
    let e = world.spawn();
    let mut agent = voltex_ai::NavAgent::new(5.0);
    agent.set_target(voltex_math::Vec3::new(1.0, 0.0, 1.0));
    agent.reached = true;
    world.add(e, agent);
    register_engine_api(&lua, &mut world as *mut World);

    lua.exec(&format!(
        "r = nav_reached({}, {}) and 1 or 0\nnav_clear_target({}, {})",
        e.id, e.generation, e.id, e.generation
    ))
    .unwrap();

    assert_eq!(lua.get_global_number("r"), Some(1.0), "reached should be true");
    let agent = world.get::<voltex_ai::NavAgent>(e).unwrap();
    assert!(agent.target.is_none(), "clear_target should drop the goal");
}

#[test]
fn audio_play_and_stop_toggle_source() {
    let lua = LuaState::new();
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, voltex_audio::AudioSource::new(0u32));
    register_engine_api(&lua, &mut world as *mut World);

    lua.exec(&format!("audio_play({}, {})", e.id, e.generation)).unwrap();
    assert!(world.get::<voltex_audio::AudioSource>(e).unwrap().playing);

    lua.exec(&format!("audio_stop({}, {})", e.id, e.generation)).unwrap();
    assert!(!world.get::<voltex_audio::AudioSource>(e).unwrap().playing);
}

#[test]
fn nav_audio_on_entity_without_components_is_noop() {
    let lua = LuaState::new();
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::new()); // no NavAgent / AudioSource
    register_engine_api(&lua, &mut world as *mut World);

    // Must not panic, and nav_reached must be false.
    lua.exec(&format!(
        "nav_set_target({}, {}, 1.0, 2.0, 3.0)\naudio_play({}, {})\nr = nav_reached({}, {}) and 1 or 0",
        e.id, e.generation, e.id, e.generation, e.id, e.generation
    ))
    .unwrap();
    assert_eq!(lua.get_global_number("r"), Some(0.0));
}

#[test]
fn find_all_by_tag_returns_empty_table_when_no_match() {
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);

    lua.exec("
        local none = find_all_by_tag('Missing')
        none_count = #none
        none_is_table = type(none) == 'table'
    ").unwrap();

    assert_eq!(lua.get_global_number("none_count"), Some(0.0));
    // Lua converts boolean to number for get_global_number? No — use exec assertion.
    // Instead, verify by setting a number flag.
    lua.exec("flag = (type(find_all_by_tag('Missing')) == 'table') and 1 or 0").unwrap();
    assert_eq!(lua.get_global_number("flag"), Some(1.0));
}

#[test]
fn iter_entities_returns_all_live() {
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);

    // Spawn three, despawn one, expect iter_entities to report two.
    let e0 = world.spawn();
    let _e1 = world.spawn();
    let _e2 = world.spawn();
    world.despawn(e0);

    lua.exec("
        local all = iter_entities()
        count = #all
        second_id = all[1].id
        second_gen = all[1].gen
        third_id = all[2].id
    ").unwrap();

    assert_eq!(lua.get_global_number("count"), Some(2.0));
    // The first live entity is the second one we spawned (id=1, gen=0).
    assert_eq!(lua.get_global_number("second_id"), Some(1.0));
    assert_eq!(lua.get_global_number("second_gen"), Some(0.0));
    assert_eq!(lua.get_global_number("third_id"), Some(2.0));
}

#[test]
fn iter_by_tag_matches_find_all_by_tag() {
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);

    lua.exec("
        for _ = 1, 4 do
            local id, gen = spawn_entity()
            set_tag(id, gen, 'spinner')
        end
        local found = iter_by_tag('spinner')
        spinner_count = #found
    ").unwrap();

    assert_eq!(lua.get_global_number("spinner_count"), Some(4.0));
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

#[test]
fn entity_count_without_world_global_returns_nothing() {
    // get_world used to dereference a null pointer when the
    // __voltex_world global was missing or non-userdata. Pre-fix
    // this was SIGSEGV; now the function returns 0 and Lua sees nil.
    let lua = LuaState::new();
    // No register_engine_api call: skip world registration. We still
    // need the function bound, so do it manually.
    lua.register_fn("entity_count", lua_entity_count);
    lua.exec("count = entity_count()").unwrap();
    // entity_count not called against a world → Lua sees nil.
    assert_eq!(lua.get_global_number("count"), None);
}

#[test]
fn entity_count_after_global_overwrite_does_not_crash() {
    // A misbehaving script that reassigns the world global must not
    // crash the engine. Pre-fix the next FFI call dereferenced a
    // null/garbage pointer.
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);
    lua.exec("
        __voltex_world = nil
        count = entity_count()
    ").unwrap();
    assert_eq!(lua.get_global_number("count"), None);
}

#[test]
fn set_position_with_non_numeric_args_is_silent_noop() {
    // Pre-fix: lua_tonumberx coerced "hello"/nil to 0 silently,
    // making set_position("a","b",1,2,3) mutate entity (0, 0).
    // Now the FFI rejects non-numeric handle args via isnum check.
    let lua = LuaState::new();
    let mut world = World::new();
    register_engine_api(&lua, &mut world as *mut World);

    // Spawn entity 0 explicitly so a silent (0,0) mutation would be observable.
    lua.exec("
        local id, gen = spawn_entity()
        set_position(\"bogus\", \"id\", 99.0, 99.0, 99.0)
        local x, y, z = get_position(id, gen)
        pos_x = x
    ").unwrap();
    // Position must still be 0 — non-numeric set was rejected.
    assert_eq!(lua.get_global_number("pos_x"), Some(0.0));
}
