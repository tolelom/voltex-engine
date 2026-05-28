#![no_main]
use libfuzzer_sys::fuzz_target;
use voltex_ecs::{deserialize_scene, World};

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let mut world = World::new();
        let _ = deserialize_scene(&mut world, text);
    }
});
