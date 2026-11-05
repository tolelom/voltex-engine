#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = voltex_ai::deserialize_navmesh_headerless(data);
});
