//! Randomized properties for snapshot replication: `apply_diff(old,
//! diff_snapshots(old, new))` reproduces `new` bit-for-bit (adds, removals,
//! partial field changes), decoders never panic on arbitrary bytes, and the
//! interpolation buffer's result doesn't depend on the order UDP delivered
//! the snapshots in.

use std::collections::BTreeMap;
use voltex_net::{
    apply_diff, deserialize_snapshot, diff_snapshots, serialize_snapshot, EntityState,
    InterpolationBuffer, Snapshot,
};

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn f(&mut self) -> f32 {
        (self.next() % 20_001) as f32 / 100.0 - 100.0
    }
    fn chance(&mut self, percent: u32) -> bool {
        self.next() % 100 < percent
    }
}

fn random_entity(rng: &mut XorShift, id: u32) -> EntityState {
    EntityState {
        id,
        position: [rng.f(), rng.f(), rng.f()],
        rotation: [rng.f(), rng.f(), rng.f(), rng.f()],
        velocity: [rng.f(), rng.f(), rng.f()],
    }
}

/// id -> bit patterns of every field, so -0.0 vs 0.0 and NaNs compare exactly.
fn bits(s: &Snapshot) -> BTreeMap<u32, Vec<u32>> {
    s.entities
        .iter()
        .map(|e| {
            let fields = e.position.iter().chain(&e.rotation).chain(&e.velocity).map(|v| v.to_bits()).collect();
            (e.id, fields)
        })
        .collect()
}

fn evolve(rng: &mut XorShift, old: &Snapshot) -> Snapshot {
    let mut entities = Vec::new();
    for e in &old.entities {
        if rng.chance(15) {
            continue; // removed
        }
        let mut e = e.clone();
        if rng.chance(50) {
            e.position[rng.next() as usize % 3] = rng.f();
        }
        if rng.chance(30) {
            e.rotation = [rng.f(), rng.f(), rng.f(), rng.f()];
        }
        if rng.chance(30) {
            e.velocity[0] = -e.velocity[0]; // includes 0.0 -> -0.0
        }
        entities.push(e);
    }
    for _ in 0..rng.next() % 5 {
        let id = 1000 + rng.next() % 1000;
        entities.push(random_entity(rng, id));
    }
    entities.sort_by_key(|e| e.id);
    entities.dedup_by_key(|e| e.id);
    Snapshot { tick: old.tick + 1, entities }
}

#[test]
fn diff_then_apply_reproduces_the_new_snapshot_exactly() {
    let mut rng = XorShift(0x5EAD_0001);
    for chain in 0..50 {
        let mut snap = Snapshot { tick: 0, entities: (0..30).map(|i| random_entity(&mut rng, i)).collect() };
        for step in 0..40 {
            let next = evolve(&mut rng, &snap);
            let diff = diff_snapshots(&snap, &next);
            let applied = apply_diff(&snap, &diff).unwrap();
            assert_eq!(applied.tick, next.tick, "chain {chain} step {step}: tick");
            assert_eq!(bits(&applied), bits(&next), "chain {chain} step {step}: entity state");
            let wire = deserialize_snapshot(&serialize_snapshot(&next)).unwrap();
            assert_eq!(bits(&wire), bits(&next), "chain {chain} step {step}: full serialize");
            snap = applied;
        }
    }
}

#[test]
fn decoders_never_panic_on_arbitrary_bytes() {
    let mut rng = XorShift(0xF022_0003);
    let base = Snapshot { tick: 1, entities: (0..5).map(|i| random_entity(&mut rng, i)).collect() };
    let valid = diff_snapshots(&base, &evolve(&mut rng, &base));
    for _ in 0..20_000 {
        let len = rng.next() as usize % 200;
        let mut bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        let _ = deserialize_snapshot(&bytes);
        let _ = apply_diff(&base, &bytes);
        // Also mutate a valid diff (bit flips + truncation).
        bytes = valid.clone();
        for _ in 0..1 + rng.next() % 4 {
            let i = rng.next() as usize % bytes.len();
            bytes[i] ^= 1 << (rng.next() % 8);
        }
        bytes.truncate(rng.next() as usize % (bytes.len() + 1));
        let _ = apply_diff(&base, &bytes);
    }
}

#[test]
fn interpolation_ignores_arrival_order() {
    let mut rng = XorShift(0x1E2F_0005);
    for case in 0..300 {
        let n = 3 + rng.next() as usize % 10;
        let frames: Vec<(f64, Snapshot)> = (0..n)
            .map(|i| {
                let t = i as f64 * 0.05;
                (t, Snapshot { tick: i as u32, entities: vec![random_entity(&mut rng, 7)] })
            })
            .collect();
        let mut shuffled = frames.clone();
        for i in (1..shuffled.len()).rev() {
            shuffled.swap(i, rng.next() as usize % (i + 1));
        }
        let mut in_order = InterpolationBuffer::new(0.1);
        let mut out_of_order = InterpolationBuffer::new(0.1);
        for (t, s) in frames {
            in_order.push(t, s);
        }
        for (t, s) in shuffled {
            out_of_order.push(t, s);
        }
        let render = (rng.next() % 1000) as f64 / 1000.0 * (n as f64 * 0.05);
        let (a, b) = (in_order.interpolate(render), out_of_order.interpolate(render));
        assert_eq!(a.as_ref().map(bits), b.as_ref().map(bits), "case {case}: render {render}");
    }
}
