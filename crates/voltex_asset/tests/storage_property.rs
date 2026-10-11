//! Generation-handle safety for `AssetStorage`: a released handle must never
//! resolve again, however many times its slot is reused. Deterministic ABA
//! regression plus a randomized model check (insert / add_ref / release /
//! replace) against a plain map.

use std::collections::BTreeMap;
use voltex_asset::{AssetStorage, Handle};

#[test]
fn handle_from_a_reused_slot_does_not_resolve_after_the_next_reuse() {
    // release() cleared the slot to None, so the next insert() computed the
    // generation as `None -> 0, + 1` and every reuse got generation 1: the
    // handle from the 2nd occupant then resolved to the 3rd occupant.
    let mut s = AssetStorage::new();
    let a = s.insert("a");
    assert!(s.release(a));
    let b = s.insert("b");
    assert!(s.release(b));
    let c = s.insert("c");
    assert_eq!(s.get(c), Some(&"c"));
    assert_eq!(s.get(b), None, "stale handle {b:?} resolved to the new occupant {c:?}");
    assert_eq!(s.get(a), None);
}

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

#[test]
fn storage_matches_a_model_and_stale_handles_stay_dead() {
    for seed in [3u32, 0xA55E7, 0x1234_9876] {
        let mut rng = XorShift(seed);
        let mut s: AssetStorage<u32> = AssetStorage::new();
        // live handle -> (value, ref_count)
        let mut live: BTreeMap<(u32, u32), (u32, u32)> = BTreeMap::new();
        let mut ever: Vec<Handle<u32>> = Vec::new();
        for step in 0..5000 {
            let pick = |rng: &mut XorShift| ever.get(rng.next() as usize % ever.len().max(1)).copied();
            match rng.next() % 4 {
                0 => {
                    let v = rng.next();
                    let h = s.insert(v);
                    let key = (h.id(), h.generation());
                    assert!(!ever.iter().any(|o| (o.id(), o.generation()) == key), "seed {seed:x} step {step}: handle {key:?} reissued");
                    live.insert(key, (v, 1));
                    ever.push(h);
                }
                1 => {
                    if let Some(h) = pick(&mut rng) {
                        s.add_ref(h);
                        if let Some(e) = live.get_mut(&(h.id(), h.generation())) {
                            e.1 += 1;
                        }
                    }
                }
                2 => {
                    if let Some(h) = pick(&mut rng) {
                        let removed = s.release(h);
                        let key = (h.id(), h.generation());
                        let expect = match live.get_mut(&key) {
                            Some(e) => {
                                e.1 -= 1;
                                e.1 == 0
                            }
                            None => false,
                        };
                        if expect {
                            live.remove(&key);
                        }
                        assert_eq!(removed, expect, "seed {seed:x} step {step}: release {key:?}");
                    }
                }
                _ => {
                    if let Some(h) = pick(&mut rng) {
                        let v = rng.next();
                        let key = (h.id(), h.generation());
                        assert_eq!(s.replace_in_place(h, v), live.contains_key(&key), "seed {seed:x} step {step}: replace {key:?}");
                        if let Some(e) = live.get_mut(&key) {
                            e.0 = v;
                        }
                    }
                }
            }
            assert_eq!(s.len(), live.len(), "seed {seed:x} step {step}: len");
            for h in &ever {
                let key = (h.id(), h.generation());
                assert_eq!(s.get(*h).copied(), live.get(&key).map(|e| e.0), "seed {seed:x} step {step}: get {key:?}");
                assert_eq!(s.ref_count(*h), live.get(&key).map_or(0, |e| e.1), "seed {seed:x} step {step}: ref_count {key:?}");
            }
        }
    }
}
