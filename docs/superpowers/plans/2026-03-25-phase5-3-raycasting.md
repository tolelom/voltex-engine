# Phase 5-3: Raycasting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** BVH 가속 레이캐스트 — Ray를 쏘아 가장 가까운 콜라이더 entity를 찾는다

**Architecture:** `voltex_math`에 Ray 타입 추가, `voltex_physics`에 기하 교차 함수 3개(AABB, Sphere, Box)와 ECS 통합 raycast 함수. 기존 BvhTree를 재사용하여 broad phase 가속.

**Tech Stack:** Rust, voltex_math (Vec3, AABB, Ray), voltex_ecs (World, Entity, Transform), voltex_physics (Collider, BvhTree)

**Spec:** `docs/superpowers/specs/2026-03-25-phase5-3-raycasting.md`

---

## File Structure

### voltex_math (수정)
- `crates/voltex_math/src/ray.rs` — Ray 타입 (Create)
- `crates/voltex_math/src/lib.rs` — Ray 모듈 등록 (Modify)

### voltex_physics (추가)
- `crates/voltex_physics/src/ray.rs` — ray_vs_aabb, ray_vs_sphere, ray_vs_box (Create)
- `crates/voltex_physics/src/raycast.rs` — RayHit, raycast (Create)
- `crates/voltex_physics/src/lib.rs` — 새 모듈 등록 (Modify)

---

## Task 1: Ray 타입 (voltex_math)

**Files:**
- Create: `crates/voltex_math/src/ray.rs`
- Modify: `crates/voltex_math/src/lib.rs`

- [ ] **Step 1: ray.rs 작성**

```rust
// crates/voltex_math/src/ray.rs
use crate::Vec3;

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    /// Create a ray. Direction is normalized.
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Point along the ray at parameter t.
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn approx_vec(a: Vec3, b: Vec3) -> bool {
        approx(a.x, b.x) && approx(a.y, b.y) && approx(a.z, b.z)
    }

    #[test]
    fn test_new_normalizes_direction() {
        let r = Ray::new(Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0));
        assert!(approx_vec(r.direction, Vec3::X));
    }

    #[test]
    fn test_at() {
        let r = Ray::new(Vec3::new(1.0, 2.0, 3.0), Vec3::X);
        let p = r.at(5.0);
        assert!(approx_vec(p, Vec3::new(6.0, 2.0, 3.0)));
    }
}
```

- [ ] **Step 2: lib.rs에 ray 모듈 등록**

```rust
pub mod ray;
pub use ray::Ray;
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p voltex_math`
Expected: 37 PASS (기존 35 + 2)

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_math/src/ray.rs crates/voltex_math/src/lib.rs
git commit -m "feat(math): add Ray type with direction normalization"
```

---

## Task 2: 기하 교차 함수 (ray_vs_aabb, ray_vs_sphere, ray_vs_box)

**Files:**
- Create: `crates/voltex_physics/src/ray.rs`
- Modify: `crates/voltex_physics/src/lib.rs`

- [ ] **Step 1: ray.rs 작성**

```rust
// crates/voltex_physics/src/ray.rs
use voltex_math::{Vec3, Ray, AABB};

/// Ray vs AABB (slab method). Returns t of nearest intersection, or None.
/// If ray starts inside AABB, returns Some(0.0).
pub fn ray_vs_aabb(ray: &Ray, aabb: &AABB) -> Option<f32> {
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;

    let o = [ray.origin.x, ray.origin.y, ray.origin.z];
    let d = [ray.direction.x, ray.direction.y, ray.direction.z];
    let bmin = [aabb.min.x, aabb.min.y, aabb.min.z];
    let bmax = [aabb.max.x, aabb.max.y, aabb.max.z];

    for i in 0..3 {
        if d[i].abs() < 1e-8 {
            // Ray parallel to slab
            if o[i] < bmin[i] || o[i] > bmax[i] {
                return None;
            }
        } else {
            let inv_d = 1.0 / d[i];
            let mut t1 = (bmin[i] - o[i]) * inv_d;
            let mut t2 = (bmax[i] - o[i]) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return None;
            }
        }
    }

    if t_max < 0.0 {
        return None; // AABB is behind ray
    }

    Some(t_min.max(0.0))
}

/// Ray vs Sphere. Returns (t, normal) or None.
pub fn ray_vs_sphere(ray: &Ray, center: Vec3, radius: f32) -> Option<(f32, Vec3)> {
    let oc = ray.origin - center;
    let a = ray.direction.dot(ray.direction); // 1.0 if normalized
    let b = 2.0 * oc.dot(ray.direction);
    let c = oc.dot(oc) - radius * radius;
    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        return None;
    }

    let sqrt_d = discriminant.sqrt();
    let mut t = (-b - sqrt_d) / (2.0 * a);

    if t < 0.0 {
        t = (-b + sqrt_d) / (2.0 * a); // try far intersection
        if t < 0.0 {
            return None; // both behind ray
        }
    }

    let point = ray.at(t);
    let normal = (point - center).normalize();
    Some((t, normal))
}

/// Ray vs axis-aligned Box. Returns (t, normal) or None.
pub fn ray_vs_box(ray: &Ray, center: Vec3, half_extents: Vec3) -> Option<(f32, Vec3)> {
    let aabb = AABB::from_center_half_extents(center, half_extents);
    let t = ray_vs_aabb(ray, &aabb)?;

    // Compute normal from the face where the ray enters
    if t == 0.0 {
        // Ray starts inside box — return t=0 with normal opposite to ray direction
        // Find closest face
        let bmin = aabb.min;
        let bmax = aabb.max;
        let p = ray.origin;

        let faces: [(f32, Vec3); 6] = [
            (p.x - bmin.x, Vec3::new(-1.0, 0.0, 0.0)),
            (bmax.x - p.x, Vec3::new(1.0, 0.0, 0.0)),
            (p.y - bmin.y, Vec3::new(0.0, -1.0, 0.0)),
            (bmax.y - p.y, Vec3::new(0.0, 1.0, 0.0)),
            (p.z - bmin.z, Vec3::new(0.0, 0.0, -1.0)),
            (bmax.z - p.z, Vec3::new(0.0, 0.0, 1.0)),
        ];

        let mut min_dist = f32::INFINITY;
        let mut normal = Vec3::Y;
        for (dist, n) in &faces {
            if *dist < min_dist {
                min_dist = *dist;
                normal = *n;
            }
        }
        return Some((0.0, normal));
    }

    // Find which face was hit by checking the hit point
    let hit = ray.at(t);
    let rel = hit - center;
    let hx = half_extents.x;
    let hy = half_extents.y;
    let hz = half_extents.z;

    let normal = if (rel.x - hx).abs() < 1e-4 {
        Vec3::X
    } else if (rel.x + hx).abs() < 1e-4 {
        Vec3::new(-1.0, 0.0, 0.0)
    } else if (rel.y - hy).abs() < 1e-4 {
        Vec3::Y
    } else if (rel.y + hy).abs() < 1e-4 {
        Vec3::new(0.0, -1.0, 0.0)
    } else if (rel.z - hz).abs() < 1e-4 {
        Vec3::Z
    } else {
        Vec3::new(0.0, 0.0, -1.0)
    };

    Some((t, normal))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn approx_vec(a: Vec3, b: Vec3) -> bool {
        approx(a.x, b.x) && approx(a.y, b.y) && approx(a.z, b.z)
    }

    // ray_vs_aabb tests
    #[test]
    fn test_aabb_hit() {
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::X);
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::ONE);
        let t = ray_vs_aabb(&ray, &aabb).unwrap();
        assert!(approx(t, 4.0));
    }

    #[test]
    fn test_aabb_miss() {
        let ray = Ray::new(Vec3::new(-5.0, 5.0, 0.0), Vec3::X);
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::ONE);
        assert!(ray_vs_aabb(&ray, &aabb).is_none());
    }

    #[test]
    fn test_aabb_inside() {
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::ONE);
        let t = ray_vs_aabb(&ray, &aabb).unwrap();
        assert!(approx(t, 0.0));
    }

    // ray_vs_sphere tests
    #[test]
    fn test_sphere_hit() {
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::X);
        let (t, normal) = ray_vs_sphere(&ray, Vec3::ZERO, 1.0).unwrap();
        assert!(approx(t, 4.0));
        assert!(approx_vec(normal, Vec3::new(-1.0, 0.0, 0.0)));
    }

    #[test]
    fn test_sphere_miss() {
        let ray = Ray::new(Vec3::new(-5.0, 5.0, 0.0), Vec3::X);
        assert!(ray_vs_sphere(&ray, Vec3::ZERO, 1.0).is_none());
    }

    #[test]
    fn test_sphere_tangent() {
        let ray = Ray::new(Vec3::new(-5.0, 1.0, 0.0), Vec3::X);
        let result = ray_vs_sphere(&ray, Vec3::ZERO, 1.0);
        // Tangent: discriminant ≈ 0, should still hit
        assert!(result.is_some());
    }

    #[test]
    fn test_sphere_inside() {
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let (t, _normal) = ray_vs_sphere(&ray, Vec3::ZERO, 2.0).unwrap();
        // Should return far intersection
        assert!(approx(t, 2.0));
    }

    // ray_vs_box tests
    #[test]
    fn test_box_hit_face() {
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::X);
        let (t, normal) = ray_vs_box(&ray, Vec3::ZERO, Vec3::ONE).unwrap();
        assert!(approx(t, 4.0));
        assert!(approx_vec(normal, Vec3::new(-1.0, 0.0, 0.0)));
    }

    #[test]
    fn test_box_miss() {
        let ray = Ray::new(Vec3::new(-5.0, 5.0, 0.0), Vec3::X);
        assert!(ray_vs_box(&ray, Vec3::ZERO, Vec3::ONE).is_none());
    }

    #[test]
    fn test_box_inside() {
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let (t, _normal) = ray_vs_box(&ray, Vec3::ZERO, Vec3::ONE).unwrap();
        assert!(approx(t, 0.0));
    }
}
```

- [ ] **Step 2: lib.rs에 ray 모듈 등록**

NOTE: voltex_physics already has modules. The new module name `ray` should not conflict.

```rust
pub mod ray;
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 46 PASS (기존 36 + 10)

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_physics/src/ray.rs crates/voltex_physics/src/lib.rs
git commit -m "feat(physics): add ray intersection tests (AABB, sphere, box)"
```

---

## Task 3: ECS 레이캐스트 통합

**Files:**
- Create: `crates/voltex_physics/src/raycast.rs`
- Modify: `crates/voltex_physics/src/lib.rs`

- [ ] **Step 1: raycast.rs 작성**

```rust
// crates/voltex_physics/src/raycast.rs
use voltex_ecs::{World, Entity};
use voltex_ecs::Transform;
use voltex_math::{Vec3, Ray};

use crate::collider::Collider;
use crate::bvh::BvhTree;
use crate::ray as ray_tests;

#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub entity: Entity,
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

/// Cast a ray into the world. Returns the closest hit within max_dist, or None.
pub fn raycast(world: &World, ray: &Ray, max_dist: f32) -> Option<RayHit> {
    // 1. Gather entities with Transform + Collider
    let entities: Vec<(Entity, Vec3, Collider)> = world
        .query2::<Transform, Collider>()
        .into_iter()
        .map(|(e, t, c)| (e, t.position, *c))
        .collect();

    if entities.is_empty() {
        return None;
    }

    // 2. Build BVH
    let bvh_entries: Vec<(Entity, voltex_math::AABB)> = entities
        .iter()
        .map(|(e, pos, col)| (*e, col.aabb(*pos)))
        .collect();
    let bvh = BvhTree::build(&bvh_entries);

    // 3. Test all leaves (broad + narrow)
    let mut closest: Option<RayHit> = None;

    for (entity, pos, collider) in &entities {
        let aabb = collider.aabb(*pos);

        // Broad phase: ray vs AABB
        let aabb_t = match ray_tests::ray_vs_aabb(ray, &aabb) {
            Some(t) if t <= max_dist => t,
            _ => continue,
        };

        // Early skip if we already have a closer hit
        if let Some(ref hit) = closest {
            if aabb_t >= hit.t {
                continue;
            }
        }

        // Narrow phase: ray vs collider
        let result = match collider {
            Collider::Sphere { radius } => {
                ray_tests::ray_vs_sphere(ray, *pos, *radius)
            }
            Collider::Box { half_extents } => {
                ray_tests::ray_vs_box(ray, *pos, *half_extents)
            }
        };

        if let Some((t, normal)) = result {
            if t <= max_dist {
                if closest.is_none() || t < closest.as_ref().unwrap().t {
                    closest = Some(RayHit {
                        entity: *entity,
                        t,
                        point: ray.at(t),
                        normal,
                    });
                }
            }
        }
    }

    closest
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::World;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;
    use crate::Collider;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn test_empty_world() {
        let world = World::new();
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        assert!(raycast(&world, &ray, 100.0).is_none());
    }

    #[test]
    fn test_hit_sphere() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(5.0, 0.0, 0.0)));
        world.add(e, Collider::Sphere { radius: 1.0 });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hit = raycast(&world, &ray, 100.0).unwrap();

        assert_eq!(hit.entity, e);
        assert!(approx(hit.t, 4.0)); // 5.0 - 1.0 radius
        assert!(approx(hit.point.x, 4.0));
    }

    #[test]
    fn test_closest_of_multiple() {
        let mut world = World::new();

        let far = world.spawn();
        world.add(far, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        world.add(far, Collider::Sphere { radius: 1.0 });

        let near = world.spawn();
        world.add(near, Transform::from_position(Vec3::new(3.0, 0.0, 0.0)));
        world.add(near, Collider::Sphere { radius: 1.0 });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hit = raycast(&world, &ray, 100.0).unwrap();

        assert_eq!(hit.entity, near);
        assert!(approx(hit.t, 2.0)); // 3.0 - 1.0
    }

    #[test]
    fn test_max_dist() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(50.0, 0.0, 0.0)));
        world.add(e, Collider::Sphere { radius: 1.0 });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        assert!(raycast(&world, &ray, 10.0).is_none()); // too far
    }

    #[test]
    fn test_hit_box() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(5.0, 0.0, 0.0)));
        world.add(e, Collider::Box { half_extents: Vec3::ONE });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hit = raycast(&world, &ray, 100.0).unwrap();

        assert_eq!(hit.entity, e);
        assert!(approx(hit.t, 4.0)); // 5.0 - 1.0 half_extent
    }

    #[test]
    fn test_mixed_sphere_box() {
        let mut world = World::new();

        let sphere = world.spawn();
        world.add(sphere, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        world.add(sphere, Collider::Sphere { radius: 1.0 });

        let box_e = world.spawn();
        world.add(box_e, Transform::from_position(Vec3::new(3.0, 0.0, 0.0)));
        world.add(box_e, Collider::Box { half_extents: Vec3::ONE });

        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hit = raycast(&world, &ray, 100.0).unwrap();

        assert_eq!(hit.entity, box_e); // box is closer
        assert!(approx(hit.t, 2.0));  // 3.0 - 1.0
    }
}
```

- [ ] **Step 2: lib.rs에 raycast 모듈 등록**

```rust
pub mod raycast;
pub use raycast::{RayHit, raycast};
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 52 PASS (46 + 6)

- [ ] **Step 4: 전체 workspace 테스트**

Run: `cargo test --workspace`
Expected: all pass

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_physics/src/raycast.rs crates/voltex_physics/src/lib.rs
git commit -m "feat(physics): add BVH-accelerated raycast with ECS integration"
```

---

## Task 4: 문서 업데이트

**Files:**
- Modify: `docs/dev/STATUS.md`
- Modify: `docs/dev/DEFERRED.md`

- [ ] **Step 1: STATUS.md에 Phase 5-3 추가**

Phase 5-2 아래에:
```markdown
### Phase 5-3: Raycasting
- voltex_math: Ray type (origin, direction, at)
- voltex_physics: ray_vs_aabb, ray_vs_sphere, ray_vs_box
- voltex_physics: raycast(world, ray, max_dist) BVH-accelerated ECS integration
```

테스트 수 업데이트. 다음을 Phase 6 (오디오)로 변경.

- [ ] **Step 2: DEFERRED.md에 Phase 5-3 미뤄진 항목 추가**

```markdown
## Phase 5-3

- **Ray vs Plane, Triangle, Mesh** — 콜라이더 기반만 지원. 메시 레벨 레이캐스트 미구현.
- **raycast_all (다중 hit)** — 가장 가까운 hit만 반환.
- **BVH 조기 종료 최적화** — 모든 리프 검사 후 최소 t 선택. front-to-back 순회 미구현.
```

- [ ] **Step 3: 커밋**

```bash
git add docs/dev/STATUS.md docs/dev/DEFERRED.md
git commit -m "docs: add Phase 5-3 raycasting status and deferred items"
```
