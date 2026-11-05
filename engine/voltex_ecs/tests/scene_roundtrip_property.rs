//! Randomized round-trip property for every scene format: random worlds with
//! transforms, tags, hierarchy (including entities despawned along the way,
//! so ids have holes) must come back structurally identical from JSON,
//! binary VSCN and the legacy text format.

use voltex_ecs::{
    add_child, deserialize_scene_text, deserialize_scene_binary, deserialize_scene_json,
    serialize_scene_text, serialize_scene_binary, serialize_scene_json, ComponentRegistry, Parent, Tag,
    Transform, World,
};
use voltex_math::{Quat, Vec3};

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        self.next() as usize % n.max(1)
    }
    fn f(&mut self) -> f32 {
        (self.next() % 20001) as f32 / 1000.0 - 10.0
    }
}

fn random_world(seed: u32) -> World {
    let mut rng = XorShift(seed);
    let mut world = World::new();
    let mut live = Vec::new();
    for _ in 0..60 {
        match rng.below(10) {
            0 if !live.is_empty() => {
                let e = live.swap_remove(rng.below(live.len()));
                world.despawn(e);
            }
            1 | 2 if live.len() >= 2 => {
                let p = live[rng.below(live.len())];
                let c = live[rng.below(live.len())];
                add_child(&mut world, p, c); // self/cycle attempts are rejected
            }
            _ => {
                let e = world.spawn();
                let mut t = Transform::from_position(Vec3::new(rng.f(), rng.f(), rng.f()));
                t.rotation = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), rng.f());
                t.scale = Vec3::new(1.0 + rng.f().abs(), 1.0, 1.0);
                world.add(e, t);
                if rng.below(2) == 0 {
                    world.add(e, Tag(format!("tag-{} é", rng.below(7))));
                }
                live.push(e);
            }
        }
    }
    world
}

/// Identity-free structure: per entity its transform, tag, and its parent's
/// (transform, tag) — a dead parent counts as none.
type Look = ([u32; 10], Option<String>);
fn structure(world: &World) -> Vec<(Look, Option<Look>)> {
    let look = |e| {
        let t: &Transform = world.get(e)?;
        let r = t.rotation;
        let bits = [
            t.position.x, t.position.y, t.position.z, r.x, r.y, r.z, r.w, t.scale.x, t.scale.y,
            t.scale.z,
        ]
        .map(|v: f32| (v * 1000.0).round() as i32 as u32);
        Some((bits, world.get::<Tag>(e).map(|t| t.0.clone())))
    };
    let mut v: Vec<_> = world
        .query::<Transform>()
        .map(|(e, _)| {
            let parent = world
                .get::<Parent>(e)
                .filter(|p| world.is_alive(p.0))
                .and_then(|p| look(p.0));
            (look(e).unwrap(), parent)
        })
        .collect();
    v.sort();
    v
}

#[test]
fn random_worlds_actually_exercise_hierarchy_tags_and_holes() {
    // Guard against a vacuous property: the generator must produce parents,
    // tags and despawned slots for the round-trip checks to mean anything.
    for seed in [1u32, 0xBEEF, 0x1234_5678, 99, 7] {
        let world = random_world(seed);
        let s = structure(&world);
        assert!(s.iter().any(|(_, p)| p.is_some()), "seed {seed:x}: no hierarchy");
        assert!(s.iter().any(|((_, tag), _)| tag.is_some()), "seed {seed:x}: no tags");
        // Despawned slots are reused by later spawns, so "despawn happened"
        // shows as either a hole or a bumped generation.
        let max_id = world.live_entities().map(|e| e.id).max().unwrap_or(0) as usize;
        let reused = world.live_entities().any(|e| e.generation > 0);
        assert!(reused || world.entity_count() <= max_id, "seed {seed:x}: no despawns exercised");
    }
}

fn registry() -> ComponentRegistry {
    let mut r = ComponentRegistry::new();
    r.register_defaults();
    r
}

#[test]
fn json_roundtrip_preserves_random_worlds() {
    for seed in [1u32, 0xBEEF, 0x1234_5678, 99, 7] {
        let world = random_world(seed);
        let json = serialize_scene_json(&world, &registry());
        let mut back = World::new();
        deserialize_scene_json(&mut back, &json, &registry()).unwrap();
        assert_eq!(structure(&back), structure(&world), "seed {seed:x}");
    }
}

#[test]
fn binary_roundtrip_preserves_random_worlds() {
    for seed in [1u32, 0xBEEF, 0x1234_5678, 99, 7] {
        let world = random_world(seed);
        let bin = serialize_scene_binary(&world, &registry());
        let mut back = World::new();
        deserialize_scene_binary(&mut back, &bin, &registry()).unwrap();
        assert_eq!(structure(&back), structure(&world), "seed {seed:x}");
    }
}

#[test]
fn text_roundtrip_preserves_random_worlds() {
    for seed in [1u32, 0xBEEF, 0x1234_5678, 99, 7] {
        let world = random_world(seed);
        let text = serialize_scene_text(&world);
        let mut back = World::new();
        deserialize_scene_text(&mut back, &text);
        assert_eq!(structure(&back), structure(&world), "seed {seed:x}");
    }
}
