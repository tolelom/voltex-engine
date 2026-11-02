# Phase 5-2: Rigid Body Simulation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 리지드바디 시뮬레이션 — 물체가 중력으로 떨어지고, 충돌 시 임펄스로 튕기는 기본 물리

**Architecture:** `voltex_physics`에 RigidBody 컴포넌트, Semi-implicit Euler 적분, 임펄스 기반 충돌 응답 추가. ECS의 collect-compute-apply 패턴으로 borrow 문제 회피. `physics_step()`이 적분 → 충돌 감지 → 충돌 응답을 순차 실행.

**Tech Stack:** Rust, voltex_math (Vec3), voltex_ecs (World, Entity, Transform, get_mut)

**Spec:** `docs/superpowers/specs/2026-03-25-phase5-2-rigidbody.md`

---

## File Structure

### voltex_physics (수정/추가)
- `crates/voltex_physics/src/rigid_body.rs` — RigidBody 컴포넌트 + PhysicsConfig (Create)
- `crates/voltex_physics/src/integrator.rs` — integrate 함수 (Create)
- `crates/voltex_physics/src/solver.rs` — resolve_collisions + physics_step (Create)
- `crates/voltex_physics/src/lib.rs` — 새 모듈 등록 (Modify)

---

## Task 1: RigidBody 컴포넌트 + PhysicsConfig

**Files:**
- Create: `crates/voltex_physics/src/rigid_body.rs`
- Modify: `crates/voltex_physics/src/lib.rs`

- [ ] **Step 1: rigid_body.rs 작성**

```rust
// crates/voltex_physics/src/rigid_body.rs
use voltex_math::Vec3;

#[derive(Debug, Clone, Copy)]
pub struct RigidBody {
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
    pub restitution: f32,
    pub gravity_scale: f32,
}

impl RigidBody {
    /// Create a dynamic rigid body with given mass.
    pub fn dynamic(mass: f32) -> Self {
        Self {
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass,
            restitution: 0.3,
            gravity_scale: 1.0,
        }
    }

    /// Create a static rigid body (infinite mass, immovable).
    pub fn statik() -> Self {
        Self {
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass: 0.0,
            restitution: 0.3,
            gravity_scale: 0.0,
        }
    }

    pub fn inv_mass(&self) -> f32 {
        if self.mass == 0.0 { 0.0 } else { 1.0 / self.mass }
    }

    pub fn is_static(&self) -> bool {
        self.mass == 0.0
    }
}

pub struct PhysicsConfig {
    pub gravity: Vec3,
    pub fixed_dt: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            fixed_dt: 1.0 / 60.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_body() {
        let rb = RigidBody::dynamic(2.0);
        assert_eq!(rb.mass, 2.0);
        assert!(!rb.is_static());
        assert!((rb.inv_mass() - 0.5).abs() < 1e-6);
        assert_eq!(rb.velocity, Vec3::ZERO);
        assert_eq!(rb.restitution, 0.3);
        assert_eq!(rb.gravity_scale, 1.0);
    }

    #[test]
    fn test_static_body() {
        let rb = RigidBody::statik();
        assert_eq!(rb.mass, 0.0);
        assert!(rb.is_static());
        assert_eq!(rb.inv_mass(), 0.0);
        assert_eq!(rb.gravity_scale, 0.0);
    }

    #[test]
    fn test_physics_config_default() {
        let cfg = PhysicsConfig::default();
        assert!((cfg.gravity.y - (-9.81)).abs() < 1e-6);
        assert!((cfg.fixed_dt - 1.0 / 60.0).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: lib.rs에 모듈 등록**

`crates/voltex_physics/src/lib.rs`에 추가:
```rust
pub mod rigid_body;
pub use rigid_body::{RigidBody, PhysicsConfig};
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 기존 25 + 3 = 28개 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_physics/src/rigid_body.rs crates/voltex_physics/src/lib.rs
git commit -m "feat(physics): add RigidBody component and PhysicsConfig"
```

---

## Task 2: Semi-implicit Euler 적분

**Files:**
- Create: `crates/voltex_physics/src/integrator.rs`
- Modify: `crates/voltex_physics/src/lib.rs`

- [ ] **Step 1: integrator.rs 작성**

NOTE: ECS borrow 제약으로 collect-compute-apply 패턴 사용.
`world.query2()` returns `Vec<(Entity, &A, &B)>` (immutable refs).
변경사항은 별도로 수집 후 `world.get_mut()`으로 적용.

```rust
// crates/voltex_physics/src/integrator.rs
use voltex_ecs::World;
use voltex_ecs::Transform;
use voltex_math::Vec3;
use crate::rigid_body::{RigidBody, PhysicsConfig};

/// Apply gravity and integrate velocity/position using Semi-implicit Euler.
/// Only affects dynamic bodies (mass > 0).
pub fn integrate(world: &mut World, config: &PhysicsConfig) {
    // 1. Collect: read entities with both Transform and RigidBody
    let updates: Vec<(voltex_ecs::Entity, Vec3, Vec3)> = world
        .query2::<Transform, RigidBody>()
        .into_iter()
        .filter(|(_, _, rb)| !rb.is_static())
        .map(|(entity, transform, rb)| {
            // Semi-implicit Euler:
            // v' = v + gravity * gravity_scale * dt
            // x' = x + v' * dt
            let new_velocity = rb.velocity + config.gravity * rb.gravity_scale * config.fixed_dt;
            let new_position = transform.position + new_velocity * config.fixed_dt;
            (entity, new_velocity, new_position)
        })
        .collect();

    // 2. Apply: write back
    for (entity, new_velocity, new_position) in updates {
        if let Some(rb) = world.get_mut::<RigidBody>(entity) {
            rb.velocity = new_velocity;
        }
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = new_position;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::World;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;
    use crate::RigidBody;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn test_gravity_fall() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(0.0, 10.0, 0.0)));
        world.add(e, RigidBody::dynamic(1.0));

        let config = PhysicsConfig::default();
        integrate(&mut world, &config);

        let rb = world.get::<RigidBody>(e).unwrap();
        let t = world.get::<Transform>(e).unwrap();

        // After 1 step: v = 0 + (-9.81) * (1/60) = -0.1635
        let expected_vy = -9.81 * config.fixed_dt;
        assert!(approx(rb.velocity.y, expected_vy));

        // Position: 10 + (-0.1635) * (1/60) = 10 - 0.002725
        let expected_py = 10.0 + expected_vy * config.fixed_dt;
        assert!(approx(t.position.y, expected_py));
    }

    #[test]
    fn test_static_unchanged() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(0.0, 5.0, 0.0)));
        world.add(e, RigidBody::statik());

        let config = PhysicsConfig::default();
        integrate(&mut world, &config);

        let t = world.get::<Transform>(e).unwrap();
        assert!(approx(t.position.y, 5.0));

        let rb = world.get::<RigidBody>(e).unwrap();
        assert!(approx(rb.velocity.y, 0.0));
    }

    #[test]
    fn test_initial_velocity() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut rb = RigidBody::dynamic(1.0);
        rb.velocity = Vec3::new(5.0, 0.0, 0.0);
        rb.gravity_scale = 0.0; // no gravity for this test
        world.add(e, rb);

        let config = PhysicsConfig::default();
        integrate(&mut world, &config);

        let t = world.get::<Transform>(e).unwrap();
        let expected_x = 5.0 * config.fixed_dt;
        assert!(approx(t.position.x, expected_x));
    }
}
```

- [ ] **Step 2: lib.rs에 모듈 등록**

```rust
pub mod integrator;
pub use integrator::integrate;
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 31개 PASS (28 + 3)

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_physics/src/integrator.rs crates/voltex_physics/src/lib.rs
git commit -m "feat(physics): add Semi-implicit Euler integration"
```

---

## Task 3: 임펄스 기반 충돌 응답 + physics_step

**Files:**
- Create: `crates/voltex_physics/src/solver.rs`
- Modify: `crates/voltex_physics/src/lib.rs`

- [ ] **Step 1: solver.rs 작성**

```rust
// crates/voltex_physics/src/solver.rs
use voltex_ecs::{World, Entity};
use voltex_ecs::Transform;
use voltex_math::Vec3;

use crate::contact::ContactPoint;
use crate::rigid_body::{RigidBody, PhysicsConfig};
use crate::collision::detect_collisions;
use crate::integrator::integrate;

const POSITION_SLOP: f32 = 0.01;
const POSITION_PERCENT: f32 = 0.4;

/// Resolve collisions using impulse-based response + positional correction.
pub fn resolve_collisions(world: &mut World, contacts: &[ContactPoint]) {
    // Collect impulse + position corrections
    let mut velocity_changes: Vec<(Entity, Vec3)> = Vec::new();
    let mut position_changes: Vec<(Entity, Vec3)> = Vec::new();

    for contact in contacts {
        let rb_a = world.get::<RigidBody>(contact.entity_a).copied();
        let rb_b = world.get::<RigidBody>(contact.entity_b).copied();

        let (rb_a, rb_b) = match (rb_a, rb_b) {
            (Some(a), Some(b)) => (a, b),
            _ => continue,
        };

        let inv_mass_a = rb_a.inv_mass();
        let inv_mass_b = rb_b.inv_mass();
        let inv_mass_sum = inv_mass_a + inv_mass_b;

        // Both static — skip
        if inv_mass_sum == 0.0 {
            continue;
        }

        // Relative velocity (A relative to B)
        let v_rel = rb_a.velocity - rb_b.velocity;
        let v_rel_n = v_rel.dot(contact.normal);

        // Already separating — skip impulse
        if v_rel_n > 0.0 {
            // Still apply position correction if penetrating
        } else {
            // Impulse
            let e = rb_a.restitution.min(rb_b.restitution);
            let j = -(1.0 + e) * v_rel_n / inv_mass_sum;

            velocity_changes.push((contact.entity_a, contact.normal * (j * inv_mass_a)));
            velocity_changes.push((contact.entity_b, contact.normal * (-j * inv_mass_b)));
        }

        // Positional correction (Baumgarte)
        let correction_mag = (contact.depth - POSITION_SLOP).max(0.0) * POSITION_PERCENT / inv_mass_sum;
        if correction_mag > 0.0 {
            let correction = contact.normal * correction_mag;
            position_changes.push((contact.entity_a, correction * (-inv_mass_a)));
            position_changes.push((contact.entity_b, correction * inv_mass_b));
        }
    }

    // Apply velocity changes
    for (entity, dv) in velocity_changes {
        if let Some(rb) = world.get_mut::<RigidBody>(entity) {
            rb.velocity = rb.velocity + dv;
        }
    }

    // Apply position corrections
    for (entity, dp) in position_changes {
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = t.position + dp;
        }
    }
}

/// Run one physics simulation step: integrate → detect → resolve.
pub fn physics_step(world: &mut World, config: &PhysicsConfig) {
    integrate(world, config);
    let contacts = detect_collisions(world);
    resolve_collisions(world, &contacts);
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::World;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;
    use crate::{Collider, RigidBody};
    use crate::contact::ContactPoint;
    use crate::collision::detect_collisions;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn test_two_dynamic_spheres_head_on() {
        let mut world = World::new();

        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::new(-0.5, 0.0, 0.0)));
        world.add(a, Collider::Sphere { radius: 1.0 });
        let mut rb_a = RigidBody::dynamic(1.0);
        rb_a.velocity = Vec3::new(1.0, 0.0, 0.0);
        rb_a.restitution = 1.0; // perfect elastic
        rb_a.gravity_scale = 0.0;
        world.add(a, rb_a);

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(0.5, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });
        let mut rb_b = RigidBody::dynamic(1.0);
        rb_b.velocity = Vec3::new(-1.0, 0.0, 0.0);
        rb_b.restitution = 1.0;
        rb_b.gravity_scale = 0.0;
        world.add(b, rb_b);

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);

        resolve_collisions(&mut world, &contacts);

        let va = world.get::<RigidBody>(a).unwrap().velocity;
        let vb = world.get::<RigidBody>(b).unwrap().velocity;

        // Equal mass, perfect elastic: velocities swap
        assert!(approx(va.x, -1.0));
        assert!(approx(vb.x, 1.0));
    }

    #[test]
    fn test_dynamic_vs_static_floor() {
        let mut world = World::new();

        // Dynamic sphere above floor
        let ball = world.spawn();
        world.add(ball, Transform::from_position(Vec3::new(0.0, 0.5, 0.0)));
        world.add(ball, Collider::Sphere { radius: 1.0 });
        let mut rb = RigidBody::dynamic(1.0);
        rb.velocity = Vec3::new(0.0, -2.0, 0.0);
        rb.restitution = 1.0;
        rb.gravity_scale = 0.0;
        world.add(ball, rb);

        // Static floor
        let floor = world.spawn();
        world.add(floor, Transform::from_position(Vec3::new(0.0, -1.0, 0.0)));
        world.add(floor, Collider::Box { half_extents: Vec3::new(10.0, 1.0, 10.0) });
        world.add(floor, RigidBody::statik());

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);

        resolve_collisions(&mut world, &contacts);

        let ball_rb = world.get::<RigidBody>(ball).unwrap();
        let floor_rb = world.get::<RigidBody>(floor).unwrap();

        // Ball should bounce up
        assert!(ball_rb.velocity.y > 0.0);
        // Floor should not move
        assert!(approx(floor_rb.velocity.y, 0.0));
    }

    #[test]
    fn test_position_correction() {
        let mut world = World::new();

        // Two overlapping spheres
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });
        let mut rb_a = RigidBody::dynamic(1.0);
        rb_a.gravity_scale = 0.0;
        world.add(a, rb_a);

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });
        let mut rb_b = RigidBody::dynamic(1.0);
        rb_b.gravity_scale = 0.0;
        world.add(b, rb_b);

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].depth > POSITION_SLOP);

        resolve_collisions(&mut world, &contacts);

        let pa = world.get::<Transform>(a).unwrap().position;
        let pb = world.get::<Transform>(b).unwrap().position;

        // Bodies should have moved apart
        let dist = (pb - pa).length();
        assert!(dist > 1.0); // was 1.0, should be slightly more now
    }

    #[test]
    fn test_physics_step_ball_drop() {
        let mut world = World::new();

        // Ball high up
        let ball = world.spawn();
        world.add(ball, Transform::from_position(Vec3::new(0.0, 5.0, 0.0)));
        world.add(ball, Collider::Sphere { radius: 0.5 });
        world.add(ball, RigidBody::dynamic(1.0));

        // Static floor
        let floor = world.spawn();
        world.add(floor, Transform::from_position(Vec3::new(0.0, -1.0, 0.0)));
        world.add(floor, Collider::Box { half_extents: Vec3::new(10.0, 1.0, 10.0) });
        world.add(floor, RigidBody::statik());

        let config = PhysicsConfig::default();

        // Run several steps — ball should fall
        for _ in 0..10 {
            physics_step(&mut world, &config);
        }

        let t = world.get::<Transform>(ball).unwrap();
        // Ball should have moved down from y=5
        assert!(t.position.y < 5.0);
        // But should still be above the floor
        assert!(t.position.y > -1.0);
    }

    #[test]
    fn test_both_static_no_response() {
        let mut world = World::new();

        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });
        world.add(a, RigidBody::statik());

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(0.5, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });
        world.add(b, RigidBody::statik());

        let contacts = detect_collisions(&world);
        resolve_collisions(&mut world, &contacts);

        // Both should remain at their positions
        let pa = world.get::<Transform>(a).unwrap().position;
        let pb = world.get::<Transform>(b).unwrap().position;
        assert!(approx(pa.x, 0.0));
        assert!(approx(pb.x, 0.5));
    }
}
```

- [ ] **Step 2: lib.rs에 모듈 등록**

```rust
pub mod solver;
pub use solver::{resolve_collisions, physics_step};
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 36개 PASS (28 + 3 + 5)

- [ ] **Step 4: 전체 workspace 테스트**

Run: `cargo test --workspace`
Expected: 기존 136 + 11 = 147개 전부 PASS

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_physics/src/solver.rs crates/voltex_physics/src/lib.rs
git commit -m "feat(physics): add impulse collision response and physics_step"
```

---

## Task 4: 문서 업데이트

**Files:**
- Modify: `docs/dev/STATUS.md`
- Modify: `docs/dev/DEFERRED.md`

- [ ] **Step 1: STATUS.md에 Phase 5-2 추가**

Phase 5-1 아래에 추가:
```markdown
### Phase 5-2: Rigid Body Simulation
- voltex_physics: RigidBody (mass, velocity, restitution), PhysicsConfig
- voltex_physics: Semi-implicit Euler integration
- voltex_physics: Impulse-based collision response + positional correction (Baumgarte)
- voltex_physics: physics_step (integrate → detect → resolve)
```

테스트 수 업데이트 (voltex_physics: 36).
다음 항목을 Phase 5-3 (레이캐스팅)으로 변경.

- [ ] **Step 2: DEFERRED.md에 Phase 5-2 미뤄진 항목 추가**

```markdown
## Phase 5-2

- **각속도/회전 물리** — angular_velocity 필드만 존재, 적분 미구현. 관성 텐서 필요.
- **마찰 (Coulomb)** — 미구현. 물체가 미끄러짐 없이 반발만.
- **Sequential Impulse 솔버** — 단일 반복 충돌 응답만. 다중 물체 쌓기 불안정.
- **Sleep/Island 시스템** — 정지 물체 최적화 미구현.
```

- [ ] **Step 3: 커밋**

```bash
git add docs/dev/STATUS.md docs/dev/DEFERRED.md
git commit -m "docs: add Phase 5-2 rigid body simulation status and deferred items"
```
