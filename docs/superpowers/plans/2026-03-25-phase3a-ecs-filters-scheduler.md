# ECS Query Filters + System Scheduler Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `With<T>` / `Without<T>` query filters and a simple ordered system scheduler to voltex_ecs.

**Architecture:** Query filters use existing `SparseSet::contains()` for per-entity filtering. Scheduler stores `Box<dyn System>` and runs them in registration order. `fn(&mut World)` auto-implements `System`.

**Tech Stack:** Pure Rust, no external dependencies. Extends existing `World` in `world.rs`, new `scheduler.rs`.

---

### Task 1: `has_component<T>` Helper on World

**Files:**
- Modify: `crates/voltex_ecs/src/world.rs`

- [ ] **Step 1: Write test**

```rust
#[test]
fn test_has_component() {
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Position { x: 1.0, y: 2.0 });
    assert!(world.has_component::<Position>(e));
    assert!(!world.has_component::<Velocity>(e));
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test --package voltex_ecs -- world::tests::test_has_component -v`
Expected: FAIL — method `has_component` not found

- [ ] **Step 3: Implement**

Add to `impl World` in `crates/voltex_ecs/src/world.rs`:

```rust
pub fn has_component<T: 'static>(&self, entity: Entity) -> bool {
    self.storage::<T>().map_or(false, |s| s.contains(entity))
}
```

- [ ] **Step 4: Run test**

Run: `cargo test --package voltex_ecs -- world::tests::test_has_component -v`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_ecs/src/world.rs
git commit -m "feat(ecs): add has_component helper to World"
```

---

### Task 2: `query_with` and `query_without` (Single Component)

**Files:**
- Modify: `crates/voltex_ecs/src/world.rs`

- [ ] **Step 1: Write tests**

```rust
#[test]
fn test_query_with() {
    let mut world = World::new();
    let e0 = world.spawn();
    let e1 = world.spawn();
    let e2 = world.spawn();
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e1, Position { x: 2.0, y: 0.0 });
    // e1 has Position but no Velocity
    world.add(e2, Position { x: 3.0, y: 0.0 });
    world.add(e2, Velocity { dx: 3.0, dy: 0.0 });

    let results = world.query_with::<Position, Velocity>();
    assert_eq!(results.len(), 2);
    let entities: Vec<Entity> = results.iter().map(|(e, _)| *e).collect();
    assert!(entities.contains(&e0));
    assert!(entities.contains(&e2));
    assert!(!entities.contains(&e1));
}

#[test]
fn test_query_without() {
    let mut world = World::new();
    let e0 = world.spawn();
    let e1 = world.spawn();
    let e2 = world.spawn();
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e1, Position { x: 2.0, y: 0.0 });
    // e1 has Position but no Velocity — should be included
    world.add(e2, Position { x: 3.0, y: 0.0 });
    world.add(e2, Velocity { dx: 3.0, dy: 0.0 });

    let results = world.query_without::<Position, Velocity>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e1);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --package voltex_ecs -- world::tests::test_query_with -v`
Expected: FAIL

- [ ] **Step 3: Implement query_with and query_without**

Add to `impl World`:

```rust
/// Query entities that have component T AND also have component W.
pub fn query_with<T: 'static, W: 'static>(&self) -> Vec<(Entity, &T)> {
    let t_storage = match self.storage::<T>() {
        Some(s) => s,
        None => return Vec::new(),
    };
    let mut result = Vec::new();
    for (entity, data) in t_storage.iter() {
        if self.has_component::<W>(entity) {
            result.push((entity, data));
        }
    }
    result
}

/// Query entities that have component T but NOT component W.
pub fn query_without<T: 'static, W: 'static>(&self) -> Vec<(Entity, &T)> {
    let t_storage = match self.storage::<T>() {
        Some(s) => s,
        None => return Vec::new(),
    };
    let mut result = Vec::new();
    for (entity, data) in t_storage.iter() {
        if !self.has_component::<W>(entity) {
            result.push((entity, data));
        }
    }
    result
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package voltex_ecs -- world::tests -v`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_ecs/src/world.rs
git commit -m "feat(ecs): add query_with and query_without filters"
```

---

### Task 3: `query2_with` and `query2_without`

**Files:**
- Modify: `crates/voltex_ecs/src/world.rs`

- [ ] **Step 1: Write tests**

```rust
#[test]
fn test_query2_with() {
    #[derive(Debug, PartialEq)]
    struct Health(i32);

    let mut world = World::new();
    let e0 = world.spawn();
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e0, Health(100));

    let e1 = world.spawn();
    world.add(e1, Position { x: 2.0, y: 0.0 });
    world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
    // e1 has no Health

    let results = world.query2_with::<Position, Velocity, Health>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e0);
}

#[test]
fn test_query2_without() {
    #[derive(Debug, PartialEq)]
    struct Health(i32);

    let mut world = World::new();
    let e0 = world.spawn();
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e0, Health(100));

    let e1 = world.spawn();
    world.add(e1, Position { x: 2.0, y: 0.0 });
    world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
    // e1 has no Health

    let results = world.query2_without::<Position, Velocity, Health>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e1);
}
```

- [ ] **Step 2: Run tests to verify failure**

- [ ] **Step 3: Implement**

```rust
/// Query entities with components A and B, that also have component W.
pub fn query2_with<A: 'static, B: 'static, W: 'static>(&self) -> Vec<(Entity, &A, &B)> {
    self.query2::<A, B>().into_iter()
        .filter(|(e, _, _)| self.has_component::<W>(*e))
        .collect()
}

/// Query entities with components A and B, that do NOT have component W.
pub fn query2_without<A: 'static, B: 'static, W: 'static>(&self) -> Vec<(Entity, &A, &B)> {
    self.query2::<A, B>().into_iter()
        .filter(|(e, _, _)| !self.has_component::<W>(*e))
        .collect()
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package voltex_ecs -- world::tests -v`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_ecs/src/world.rs
git commit -m "feat(ecs): add query2_with and query2_without filters"
```

---

### Task 4: System Trait + Scheduler

**Files:**
- Create: `crates/voltex_ecs/src/scheduler.rs`
- Modify: `crates/voltex_ecs/src/lib.rs`

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::World;

    #[derive(Debug, PartialEq)]
    struct Counter(u32);

    #[test]
    fn test_scheduler_runs_in_order() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Counter(0));

        let mut scheduler = Scheduler::new();
        scheduler.add(|world: &mut World| {
            let c = world.get_mut::<Counter>(world.query::<Counter>().next().unwrap().0).unwrap();
            c.0 += 1; // 0 → 1
        });
        scheduler.add(|world: &mut World| {
            let c = world.get_mut::<Counter>(world.query::<Counter>().next().unwrap().0).unwrap();
            c.0 *= 10; // 1 → 10
        });

        scheduler.run_all(&mut world);

        let c = world.get::<Counter>(e).unwrap();
        assert_eq!(c.0, 10); // proves order: add first, then multiply
    }

    #[test]
    fn test_scheduler_empty() {
        let mut world = World::new();
        let mut scheduler = Scheduler::new();
        scheduler.run_all(&mut world); // should not panic
    }

    #[test]
    fn test_scheduler_multiple_runs() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Counter(0));

        let mut scheduler = Scheduler::new();
        scheduler.add(|world: &mut World| {
            let c = world.get_mut::<Counter>(world.query::<Counter>().next().unwrap().0).unwrap();
            c.0 += 1;
        });

        scheduler.run_all(&mut world);
        scheduler.run_all(&mut world);
        scheduler.run_all(&mut world);

        assert_eq!(world.get::<Counter>(e).unwrap().0, 3);
    }

    #[test]
    fn test_scheduler_add_chaining() {
        let mut scheduler = Scheduler::new();
        scheduler
            .add(|_: &mut World| {})
            .add(|_: &mut World| {});
        assert_eq!(scheduler.len(), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --package voltex_ecs -- scheduler::tests -v`
Expected: FAIL — module not found

- [ ] **Step 3: Implement Scheduler**

```rust
// crates/voltex_ecs/src/scheduler.rs

use crate::World;

/// A system that can be run on the world.
pub trait System {
    fn run(&mut self, world: &mut World);
}

/// Blanket impl: any FnMut(&mut World) is a System.
impl<F: FnMut(&mut World)> System for F {
    fn run(&mut self, world: &mut World) {
        (self)(world);
    }
}

/// Runs registered systems in order.
pub struct Scheduler {
    systems: Vec<Box<dyn System>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self { systems: Vec::new() }
    }

    /// Add a system. Systems run in the order they are added.
    pub fn add<S: System + 'static>(&mut self, system: S) -> &mut Self {
        self.systems.push(Box::new(system));
        self
    }

    /// Run all systems in registration order.
    pub fn run_all(&mut self, world: &mut World) {
        for system in &mut self.systems {
            system.run(world);
        }
    }

    /// Number of registered systems.
    pub fn len(&self) -> usize {
        self.systems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.systems.is_empty()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
```

Register in `crates/voltex_ecs/src/lib.rs`:
```rust
pub mod scheduler;
pub use scheduler::{Scheduler, System};
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package voltex_ecs -- scheduler::tests -v`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_ecs/src/scheduler.rs crates/voltex_ecs/src/lib.rs
git commit -m "feat(ecs): add System trait and ordered Scheduler"
```

---

### Task 5: Export and Full Build Verification

**Files:**
- Modify: `crates/voltex_ecs/src/lib.rs`

- [ ] **Step 1: Verify lib.rs exports are complete**

Ensure `lib.rs` exports:
```rust
pub use world::World;  // existing — now includes query_with, query_without, etc.
pub use scheduler::{Scheduler, System};
```

- [ ] **Step 2: Run all ECS tests**

Run: `cargo test --package voltex_ecs -v`
Expected: All tests PASS (existing + new)

- [ ] **Step 3: Run full workspace build**

Run: `cargo build --workspace`
Expected: BUILD SUCCESS

- [ ] **Step 4: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_ecs/src/lib.rs
git commit -m "feat(ecs): complete query filters and scheduler with exports"
```
