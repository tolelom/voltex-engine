//! Mutation fuzz for the three scene loaders. Serialized random worlds (with
//! hierarchy and tags) are corrupted thousands of times; scene files come
//! from disk and the editor, so loading must return `Err` / skip bad data and
//! leave a world that transform propagation can walk — never panic.

use std::panic::{catch_unwind, AssertUnwindSafe};
use voltex_ecs::{
    add_child, deserialize_scene, deserialize_scene_binary, deserialize_scene_json,
    propagate_transforms, serialize_scene, serialize_scene_binary, serialize_scene_json,
    ComponentRegistry, Tag, Transform, World,
};
use voltex_math::Vec3;

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

fn sample_world() -> World {
    let mut world = World::new();
    let mut prev = None;
    for i in 0..8 {
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(i as f32, 0.5, -1.0)));
        world.add(e, Tag(format!("node {i} é")));
        if let Some(p) = prev {
            if i % 3 != 0 {
                add_child(&mut world, p, e);
            }
        }
        prev = Some(e);
    }
    world
}

fn registry() -> ComponentRegistry {
    let mut r = ComponentRegistry::new();
    r.register_defaults();
    r
}

fn mutate(rng: &mut XorShift, src: &[u8], text: bool) -> Vec<u8> {
    let mut b = src.to_vec();
    for _ in 0..1 + rng.next() % 4 {
        let i = rng.next() as usize % b.len();
        b[i] = match rng.next() % 3 {
            0 if b[i].is_ascii_digit() => b"0123456789"[rng.next() as usize % 10],
            1 if text => b"0123456789-.e,:[]{}\" \n"[rng.next() as usize % 22],
            _ => rng.next() as u8,
        };
    }
    if rng.next().is_multiple_of(5) {
        b.truncate(rng.next() as usize % b.len());
    }
    b
}

/// Whatever a load produced, transform propagation over it must not panic.
fn check_loaded(world: &mut World) {
    propagate_transforms(world);
}

#[test]
fn scene_loaders_never_panic_on_corrupted_files() {
    let world = sample_world();
    let reg = registry();
    let binary = serialize_scene_binary(&world, &reg);
    let json = serialize_scene_json(&world, &reg);
    let text = serialize_scene(&world);
    let mut rng = XorShift(0x5CE7_E001);
    for i in 0..9000 {
        let kind = i % 3;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut w = World::new();
            match kind {
                0 => {
                    let input = mutate(&mut rng, &binary, false);
                    let _ = deserialize_scene_binary(&mut w, &input, &reg);
                }
                1 => {
                    let input = mutate(&mut rng, json.as_bytes(), true);
                    let _ = deserialize_scene_json(&mut w, &String::from_utf8_lossy(&input), &reg);
                }
                _ => {
                    let input = mutate(&mut rng, text.as_bytes(), true);
                    deserialize_scene(&mut w, &String::from_utf8_lossy(&input));
                }
            }
            check_loaded(&mut w);
        }));
        assert!(outcome.is_ok(), "loader {kind} mutation {i} panicked");
    }
}
