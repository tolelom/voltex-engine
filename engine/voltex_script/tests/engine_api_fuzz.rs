//! Lua-side fuzz of every engine API binding. Game scripts are user code:
//! calling a binding with nil, wrong types, NaN/inf, huge or negative ids,
//! stale handles or missing arguments must raise a Lua error or return
//! something harmless. A Rust panic inside an `extern "C"` binding aborts
//! the whole process, so this test passing at all is the main assertion.

use voltex_ecs::{Transform, World};
use voltex_script::{register_engine_api, LuaState};

const FUZZ: &str = r#"
local names = {
  "spawn_entity", "destroy_entity", "set_position", "get_position",
  "set_rotation", "get_rotation", "set_scale", "get_scale", "set_tag",
  "get_tag", "find_by_tag", "find_all_by_tag", "add_child", "remove_child",
  "entity_count", "find_in_radius", "iter_entities", "iter_by_tag",
  "nav_set_target", "nav_clear_target", "nav_reached", "audio_play", "audio_stop",
}
local nan, inf = 0/0, math.huge
local pool = {
  nil, true, false, 0, 1, 2, -1, 3.5, nan, inf, -inf, 2^31, 2^32, 2^53, -2^63,
  math.maxinteger, math.mininteger, "", "player", "é가", string.rep("x", 5000),
  {}, {1, 2, 3}, {id = 1, generation = 0}, print, coroutine.create(function() end),
}
local seed = 12345
local function rand(n)
  seed = (seed * 1103515245 + 12345) % 2147483648
  return seed % n + 1
end
ok_calls = 0
for i = 1, ITERATIONS do
  -- keep a few live entities around so valid ids show up too
  if rand(5) == 1 then pcall(spawn_entity) end
  local f = _G[names[rand(#names)]]
  local args = {}
  local argc = rand(7) - 1
  for a = 1, argc do args[a] = pool[rand(#pool)] end
  local ok, a, b, c = pcall(f, table.unpack(args, 1, argc))
  if ok then ok_calls = ok_calls + 1 end
  -- iterator-returning bindings: drive the iterator a few steps
  if ok and type(a) == "function" then
    for _ = 1, 3 do pcall(a, b, c) end
  end
end
calls_done = ITERATIONS
"#;

#[test]
fn engine_api_survives_garbage_arguments_from_lua() {
    let lua = LuaState::new();
    let mut world = World::new();
    for _ in 0..5 {
        let e = world.spawn();
        world.add(e, Transform::new());
    }
    register_engine_api(&lua, &mut world as *mut World);
    lua.exec(&FUZZ.replace("ITERATIONS", "20000")).expect("fuzz driver itself must not error");
    assert_eq!(lua.get_global_number("calls_done"), Some(20000.0));
    // Not just argument-check rejections: plenty of calls must reach the
    // binding bodies (valid ids, lenient numeric coercions).
    let ok_calls = lua.get_global_number("ok_calls").unwrap();
    assert!(ok_calls > 2000.0, "only {ok_calls} calls succeeded");
}
