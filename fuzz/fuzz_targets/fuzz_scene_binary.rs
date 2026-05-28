#![no_main]
use libfuzzer_sys::fuzz_target;
use voltex_ecs::{deserialize_scene_binary, ComponentRegistry, World};

fuzz_target!(|data: &[u8]| {
    let mut world = World::new();
    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    let _ = deserialize_scene_binary(&mut world, data, &registry);
});
