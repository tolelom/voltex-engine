//! Model-based property test for the ECS core: random spawn / despawn / add /
//! remove / clear sequences on a `World`, mirrored in a plain HashMap model,
//! with every observable (liveness, get, query, query2, entity_count)
//! compared after each step. Targets sparse-set index bookkeeping
//! (swap-remove, slot reuse) and clear()'s generation contract.

use std::collections::{BTreeMap, BTreeSet};
use voltex_ecs::{Entity, World};

#[derive(Debug, Clone, Copy, PartialEq)]
struct A(u32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct B(u32);

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
}

#[derive(Default)]
struct Model {
    alive: BTreeSet<(u32, u32)>,
    a: BTreeMap<(u32, u32), u32>,
    b: BTreeMap<(u32, u32), u32>,
    /// Every handle ever issued — dead ones must stay dead.
    ever: Vec<Entity>,
}

fn key(e: Entity) -> (u32, u32) {
    (e.id, e.generation)
}

fn check(world: &World, m: &Model, ctx: &str) {
    assert_eq!(world.entity_count(), m.alive.len(), "{ctx}: entity_count");
    for &e in &m.ever {
        let k = key(e);
        assert_eq!(world.is_alive(e), m.alive.contains(&k), "{ctx}: is_alive {k:?}");
        assert_eq!(world.get::<A>(e).map(|v| v.0), m.a.get(&k).copied(), "{ctx}: get A {k:?}");
        assert_eq!(world.get::<B>(e).map(|v| v.0), m.b.get(&k).copied(), "{ctx}: get B {k:?}");
    }
    let qa: BTreeMap<_, _> = world.query::<A>().map(|(e, v)| (key(e), v.0)).collect();
    assert_eq!(qa, m.a, "{ctx}: query A");
    let q2: BTreeSet<_> = world.query2::<A, B>().map(|(e, a, b)| (key(e), a.0, b.0)).collect();
    let want: BTreeSet<_> = m
        .a
        .iter()
        .filter_map(|(k, a)| m.b.get(k).map(|b| (*k, *a, *b)))
        .collect();
    assert_eq!(q2, want, "{ctx}: query2 A,B");
}

#[test]
fn world_matches_a_simple_model_under_random_operations() {
    for seed in [0xC0FFEEu32, 42, 0x9E37_79B9, 31337] {
        let mut rng = XorShift(seed);
        let mut world = World::new();
        let mut m = Model::default();
        for step in 0..1500 {
            let live: Vec<Entity> = m.ever.iter().copied().filter(|e| m.alive.contains(&key(*e))).collect();
            let any = |rng: &mut XorShift| m.ever.get(rng.below(m.ever.len())).copied();
            let pick_live = |rng: &mut XorShift| live.get(rng.below(live.len())).copied();
            let op = rng.below(100);
            let ctx = format!("seed {seed:x} step {step} op {op}");
            match op {
                0..=24 => {
                    let e = world.spawn();
                    assert!(!m.alive.contains(&key(e)), "{ctx}: spawn reissued a live handle");
                    assert!(!m.ever.contains(&e), "{ctx}: spawn reissued an old handle {e:?}");
                    m.alive.insert(key(e));
                    m.ever.push(e);
                }
                25..=39 => {
                    // despawn — sometimes a stale handle, which must be a no-op
                    if let Some(e) = any(&mut rng) {
                        world.despawn(e);
                        if m.alive.remove(&key(e)) {
                            m.a.remove(&key(e));
                            m.b.remove(&key(e));
                        }
                    }
                }
                40..=59 => {
                    if let Some(e) = pick_live(&mut rng) {
                        let v = rng.next();
                        world.add(e, A(v));
                        m.a.insert(key(e), v);
                    }
                }
                60..=74 => {
                    if let Some(e) = pick_live(&mut rng) {
                        let v = rng.next();
                        world.add(e, B(v));
                        m.b.insert(key(e), v);
                    }
                }
                75..=87 => {
                    if let Some(e) = any(&mut rng) {
                        let got = world.remove::<A>(e).map(|v| v.0);
                        assert_eq!(got, m.a.remove(&key(e)), "{ctx}: remove A return");
                    }
                }
                88..=97 => {
                    if let Some(e) = any(&mut rng) {
                        let got = world.remove::<B>(e).map(|v| v.0);
                        assert_eq!(got, m.b.remove(&key(e)), "{ctx}: remove B return");
                    }
                }
                _ => {
                    world.clear();
                    m.alive.clear();
                    m.a.clear();
                    m.b.clear();
                }
            }
            check(&world, &m, &ctx);
        }
    }
}
