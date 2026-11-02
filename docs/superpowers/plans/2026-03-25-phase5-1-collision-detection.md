# Phase 5-1: Collision Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sphere + Box 콜라이더 충돌 감지 시스템 구현 (BVH broad phase + 전용 narrow phase)

**Architecture:** `voltex_math`에 AABB 추가, `voltex_physics` crate 신규 생성. Broad phase는 BVH 트리(중앙값 분할), narrow phase는 Sphere/Box 조합별 전용 함수. ECS 통합으로 `detect_collisions(world)` API 제공.

**Tech Stack:** Rust, voltex_math (Vec3), voltex_ecs (World, Entity, Transform, SparseSet)

**Spec:** `docs/superpowers/specs/2026-03-25-phase5-1-collision-detection.md`

---

## File Structure

### voltex_math (수정)
- `crates/voltex_math/src/aabb.rs` — AABB 타입 (Create)
- `crates/voltex_math/src/lib.rs` — AABB 모듈 등록 (Modify)

### voltex_physics (신규)
- `crates/voltex_physics/Cargo.toml` — crate 설정 (Create)
- `crates/voltex_physics/src/lib.rs` — public exports (Create)
- `crates/voltex_physics/src/collider.rs` — Collider enum + aabb() (Create)
- `crates/voltex_physics/src/contact.rs` — ContactPoint (Create)
- `crates/voltex_physics/src/narrow.rs` — 충돌 테스트 함수 3개 (Create)
- `crates/voltex_physics/src/bvh.rs` — BvhTree (Create)
- `crates/voltex_physics/src/collision.rs` — detect_collisions ECS 통합 (Create)

### Workspace (수정)
- `Cargo.toml` — workspace members에 voltex_physics 추가 (Modify)

---

## Task 1: AABB 타입 (voltex_math)

**Files:**
- Create: `crates/voltex_math/src/aabb.rs`
- Modify: `crates/voltex_math/src/lib.rs`

- [ ] **Step 1: aabb.rs 생성 — 테스트부터 작성**

```rust
// crates/voltex_math/src/aabb.rs
use crate::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_center_half_extents(center: Vec3, half: Vec3) -> Self {
        Self {
            min: center - half,
            max: center + half,
        }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn merged(&self, other: &AABB) -> AABB {
        AABB {
            min: Vec3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Vec3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    pub fn surface_area(&self) -> f32 {
        let d = self.max - self.min;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_accessors() {
        let a = AABB::new(Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0));
        let c = a.center();
        assert!((c.x).abs() < 1e-6);
        assert!((c.y).abs() < 1e-6);
        assert!((c.z).abs() < 1e-6);
        let h = a.half_extents();
        assert!((h.x - 1.0).abs() < 1e-6);
        assert!((h.y - 2.0).abs() < 1e-6);
        assert!((h.z - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_from_center_half_extents() {
        let a = AABB::from_center_half_extents(Vec3::new(5.0, 5.0, 5.0), Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(a.min, Vec3::new(4.0, 4.0, 4.0));
        assert_eq!(a.max, Vec3::new(6.0, 6.0, 6.0));
    }

    #[test]
    fn test_contains_point() {
        let a = AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        assert!(a.contains_point(Vec3::new(1.0, 1.0, 1.0)));  // inside
        assert!(a.contains_point(Vec3::ZERO));                  // boundary
        assert!(!a.contains_point(Vec3::new(3.0, 1.0, 1.0)));  // outside
    }

    #[test]
    fn test_intersects() {
        let a = AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let b = AABB::new(Vec3::new(1.0, 1.0, 1.0), Vec3::new(3.0, 3.0, 3.0));
        assert!(a.intersects(&b));  // overlap

        let c = AABB::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0));
        assert!(!a.intersects(&c)); // separated

        let d = AABB::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 2.0, 2.0));
        assert!(a.intersects(&d));  // touching edge
    }

    #[test]
    fn test_merged() {
        let a = AABB::new(Vec3::ZERO, Vec3::ONE);
        let b = AABB::new(Vec3::new(2.0, 2.0, 2.0), Vec3::new(3.0, 3.0, 3.0));
        let m = a.merged(&b);
        assert_eq!(m.min, Vec3::ZERO);
        assert_eq!(m.max, Vec3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn test_surface_area() {
        // 2x2x2 box: area = 2*(4+4+4) = 24
        let a = AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        assert!((a.surface_area() - 24.0).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: lib.rs에 aabb 모듈 등록**

`crates/voltex_math/src/lib.rs`에 추가:
```rust
pub mod aabb;
pub use aabb::AABB;
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p voltex_math`
Expected: 기존 29개 + AABB 6개 = 35개 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_math/src/aabb.rs crates/voltex_math/src/lib.rs
git commit -m "feat(math): add AABB type with intersection, merge, and containment"
```

---

## Task 2: voltex_physics crate 설정 + Collider 타입

**Files:**
- Create: `crates/voltex_physics/Cargo.toml`
- Create: `crates/voltex_physics/src/lib.rs`
- Create: `crates/voltex_physics/src/contact.rs`
- Create: `crates/voltex_physics/src/collider.rs`
- Modify: `Cargo.toml` (workspace)

- [ ] **Step 1: Cargo.toml 생성**

```toml
# crates/voltex_physics/Cargo.toml
[package]
name = "voltex_physics"
version = "0.1.0"
edition = "2021"

[dependencies]
voltex_math.workspace = true
voltex_ecs.workspace = true
```

- [ ] **Step 2: workspace에 추가**

`Cargo.toml` workspace members에 추가:
```toml
"crates/voltex_physics",
```

workspace.dependencies에 추가:
```toml
voltex_physics = { path = "crates/voltex_physics" }
```

- [ ] **Step 3: contact.rs 작성**

```rust
// crates/voltex_physics/src/contact.rs
use voltex_ecs::Entity;
use voltex_math::Vec3;

#[derive(Debug, Clone, Copy)]
pub struct ContactPoint {
    pub entity_a: Entity,
    pub entity_b: Entity,
    pub normal: Vec3,
    pub depth: f32,
    pub point_on_a: Vec3,
    pub point_on_b: Vec3,
}
```

- [ ] **Step 4: collider.rs 작성 (테스트 포함)**

```rust
// crates/voltex_physics/src/collider.rs
use voltex_math::{Vec3, AABB};

#[derive(Debug, Clone, Copy)]
pub enum Collider {
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
}

impl Collider {
    pub fn aabb(&self, position: Vec3) -> AABB {
        match self {
            Collider::Sphere { radius } => {
                let r = Vec3::new(*radius, *radius, *radius);
                AABB::new(position - r, position + r)
            }
            Collider::Box { half_extents } => {
                AABB::new(position - *half_extents, position + *half_extents)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_aabb() {
        let c = Collider::Sphere { radius: 2.0 };
        let aabb = c.aabb(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(aabb.min, Vec3::new(-1.0, -2.0, -2.0));
        assert_eq!(aabb.max, Vec3::new(3.0, 2.0, 2.0));
    }

    #[test]
    fn test_box_aabb() {
        let c = Collider::Box { half_extents: Vec3::new(1.0, 2.0, 3.0) };
        let aabb = c.aabb(Vec3::ZERO);
        assert_eq!(aabb.min, Vec3::new(-1.0, -2.0, -3.0));
        assert_eq!(aabb.max, Vec3::new(1.0, 2.0, 3.0));
    }
}
```

- [ ] **Step 5: lib.rs 작성**

```rust
// crates/voltex_physics/src/lib.rs
pub mod collider;
pub mod contact;

pub use collider::Collider;
pub use contact::ContactPoint;
```

- [ ] **Step 6: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 2개 PASS (sphere_aabb, box_aabb)

- [ ] **Step 7: 커밋**

```bash
git add crates/voltex_physics/ Cargo.toml
git commit -m "feat(physics): add voltex_physics crate with Collider and ContactPoint"
```

---

## Task 3: Narrow Phase — sphere_vs_sphere

**Files:**
- Create: `crates/voltex_physics/src/narrow.rs`
- Modify: `crates/voltex_physics/src/lib.rs`

- [ ] **Step 1: narrow.rs 작성 — 테스트부터**

```rust
// crates/voltex_physics/src/narrow.rs
use voltex_math::Vec3;

/// Returns (normal A→B, depth, point_on_a, point_on_b) or None if no collision.
pub fn sphere_vs_sphere(
    pos_a: Vec3, radius_a: f32,
    pos_b: Vec3, radius_b: f32,
) -> Option<(Vec3, f32, Vec3, Vec3)> {
    let diff = pos_b - pos_a;
    let dist_sq = diff.length_squared();
    let sum_r = radius_a + radius_b;

    if dist_sq > sum_r * sum_r {
        return None;
    }

    let dist = dist_sq.sqrt();
    let normal = if dist > 1e-8 {
        diff * (1.0 / dist)
    } else {
        Vec3::Y // arbitrary fallback for coincident centers
    };

    let depth = sum_r - dist;
    let point_on_a = pos_a + normal * radius_a;
    let point_on_b = pos_b - normal * radius_b;

    Some((normal, depth, point_on_a, point_on_b))
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
    fn test_sphere_sphere_separated() {
        let r = sphere_vs_sphere(
            Vec3::ZERO, 1.0,
            Vec3::new(5.0, 0.0, 0.0), 1.0,
        );
        assert!(r.is_none());
    }

    #[test]
    fn test_sphere_sphere_overlapping() {
        let r = sphere_vs_sphere(
            Vec3::ZERO, 1.0,
            Vec3::new(1.5, 0.0, 0.0), 1.0,
        );
        let (normal, depth, pa, pb) = r.unwrap();
        assert!(approx_vec(normal, Vec3::X));
        assert!(approx(depth, 0.5));
        assert!(approx_vec(pa, Vec3::new(1.0, 0.0, 0.0)));
        assert!(approx_vec(pb, Vec3::new(0.5, 0.0, 0.0)));
    }

    #[test]
    fn test_sphere_sphere_touching() {
        let r = sphere_vs_sphere(
            Vec3::ZERO, 1.0,
            Vec3::new(2.0, 0.0, 0.0), 1.0,
        );
        let (normal, depth, _pa, _pb) = r.unwrap();
        assert!(approx_vec(normal, Vec3::X));
        assert!(approx(depth, 0.0));
    }

    #[test]
    fn test_sphere_sphere_coincident() {
        let r = sphere_vs_sphere(
            Vec3::ZERO, 1.0,
            Vec3::ZERO, 1.0,
        );
        let (_normal, depth, _pa, _pb) = r.unwrap();
        assert!(approx(depth, 2.0));
    }
}
```

- [ ] **Step 2: lib.rs에 narrow 모듈 등록**

```rust
pub mod narrow;
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 6개 PASS (collider 2 + narrow 4)

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_physics/src/narrow.rs crates/voltex_physics/src/lib.rs
git commit -m "feat(physics): add sphere_vs_sphere narrow phase"
```

---

## Task 4: Narrow Phase — sphere_vs_box

**Files:**
- Modify: `crates/voltex_physics/src/narrow.rs`

- [ ] **Step 1: sphere_vs_box 함수 + 테스트 추가**

`narrow.rs`에 추가:

```rust
pub fn sphere_vs_box(
    sphere_pos: Vec3, radius: f32,
    box_pos: Vec3, half_extents: Vec3,
) -> Option<(Vec3, f32, Vec3, Vec3)> {
    // Closest point on box to sphere center
    let bmin = box_pos - half_extents;
    let bmax = box_pos + half_extents;

    let closest = Vec3::new(
        sphere_pos.x.clamp(bmin.x, bmax.x),
        sphere_pos.y.clamp(bmin.y, bmax.y),
        sphere_pos.z.clamp(bmin.z, bmax.z),
    );

    let diff = sphere_pos - closest;
    let dist_sq = diff.length_squared();

    // Sphere center outside box
    if dist_sq > 1e-8 {
        let dist = dist_sq.sqrt();
        if dist > radius {
            return None;
        }
        let normal = diff * (-1.0 / dist); // points from sphere toward box
        // Convention: normal A→B means sphere→box
        let depth = radius - dist;
        let point_on_a = sphere_pos - normal * radius; // sphere surface (note: normal is sphere→box, so subtract)

        // Actually: normal should be A(sphere)→B(box)
        // normal = (box - sphere) direction = -diff.normalize()
        let point_on_b = closest;
        return Some((normal, depth, point_on_a, point_on_b));
    }

    // Sphere center inside box — find nearest face
    let dx_min = sphere_pos.x - bmin.x;
    let dx_max = bmax.x - sphere_pos.x;
    let dy_min = sphere_pos.y - bmin.y;
    let dy_max = bmax.y - sphere_pos.y;
    let dz_min = sphere_pos.z - bmin.z;
    let dz_max = bmax.z - sphere_pos.z;

    let mut min_dist = dx_min;
    let mut normal = Vec3::new(-1.0, 0.0, 0.0);
    let mut closest_face = Vec3::new(bmin.x, sphere_pos.y, sphere_pos.z);

    if dx_max < min_dist {
        min_dist = dx_max;
        normal = Vec3::new(1.0, 0.0, 0.0);
        closest_face = Vec3::new(bmax.x, sphere_pos.y, sphere_pos.z);
    }
    if dy_min < min_dist {
        min_dist = dy_min;
        normal = Vec3::new(0.0, -1.0, 0.0);
        closest_face = Vec3::new(sphere_pos.x, bmin.y, sphere_pos.z);
    }
    if dy_max < min_dist {
        min_dist = dy_max;
        normal = Vec3::new(0.0, 1.0, 0.0);
        closest_face = Vec3::new(sphere_pos.x, bmax.y, sphere_pos.z);
    }
    if dz_min < min_dist {
        min_dist = dz_min;
        normal = Vec3::new(0.0, 0.0, -1.0);
        closest_face = Vec3::new(sphere_pos.x, sphere_pos.y, bmin.z);
    }
    if dz_max < min_dist {
        min_dist = dz_max;
        normal = Vec3::new(0.0, 0.0, 1.0);
        closest_face = Vec3::new(sphere_pos.x, sphere_pos.y, bmax.z);
    }

    let depth = min_dist + radius;
    let point_on_a = sphere_pos + normal * radius;
    let point_on_b = closest_face;

    Some((normal, depth, point_on_a, point_on_b))
}
```

테스트 추가 (tests 모듈 안):
```rust
    #[test]
    fn test_sphere_box_separated() {
        let r = sphere_vs_box(
            Vec3::new(5.0, 0.0, 0.0), 1.0,
            Vec3::ZERO, Vec3::ONE,
        );
        assert!(r.is_none());
    }

    #[test]
    fn test_sphere_box_face_overlap() {
        // sphere at x=1.5, radius=1, box at origin half=1 → face contact on +x
        let r = sphere_vs_box(
            Vec3::new(1.5, 0.0, 0.0), 1.0,
            Vec3::ZERO, Vec3::ONE,
        );
        let (normal, depth, _pa, pb) = r.unwrap();
        assert!(approx(normal.x, -1.0)); // sphere→box: -X direction
        assert!(approx(depth, 0.5));
        assert!(approx(pb.x, 1.0));
    }

    #[test]
    fn test_sphere_box_center_inside() {
        // sphere center at box center
        let r = sphere_vs_box(
            Vec3::ZERO, 0.5,
            Vec3::ZERO, Vec3::ONE,
        );
        assert!(r.is_some());
        let (_normal, depth, _pa, _pb) = r.unwrap();
        assert!(depth > 0.0);
    }
```

- [ ] **Step 2: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 9개 PASS

- [ ] **Step 3: 커밋**

```bash
git add crates/voltex_physics/src/narrow.rs
git commit -m "feat(physics): add sphere_vs_box narrow phase"
```

---

## Task 5: Narrow Phase — box_vs_box (SAT)

**Files:**
- Modify: `crates/voltex_physics/src/narrow.rs`

- [ ] **Step 1: box_vs_box 함수 + 테스트 추가**

`narrow.rs`에 추가:

```rust
pub fn box_vs_box(
    pos_a: Vec3, half_a: Vec3,
    pos_b: Vec3, half_b: Vec3,
) -> Option<(Vec3, f32, Vec3, Vec3)> {
    // AABB vs AABB using SAT on 3 axes
    let diff = pos_b - pos_a;

    let overlap_x = (half_a.x + half_b.x) - diff.x.abs();
    if overlap_x < 0.0 { return None; }

    let overlap_y = (half_a.y + half_b.y) - diff.y.abs();
    if overlap_y < 0.0 { return None; }

    let overlap_z = (half_a.z + half_b.z) - diff.z.abs();
    if overlap_z < 0.0 { return None; }

    // Find minimum overlap axis
    let (normal, depth) = if overlap_x <= overlap_y && overlap_x <= overlap_z {
        let sign = if diff.x >= 0.0 { 1.0 } else { -1.0 };
        (Vec3::new(sign, 0.0, 0.0), overlap_x)
    } else if overlap_y <= overlap_z {
        let sign = if diff.y >= 0.0 { 1.0 } else { -1.0 };
        (Vec3::new(0.0, sign, 0.0), overlap_y)
    } else {
        let sign = if diff.z >= 0.0 { 1.0 } else { -1.0 };
        (Vec3::new(0.0, 0.0, sign), overlap_z)
    };

    let point_on_a = pos_a + Vec3::new(
        normal.x * half_a.x,
        normal.y * half_a.y,
        normal.z * half_a.z,
    );
    let point_on_b = pos_b - Vec3::new(
        normal.x * half_b.x,
        normal.y * half_b.y,
        normal.z * half_b.z,
    );

    Some((normal, depth, point_on_a, point_on_b))
}
```

테스트 추가:
```rust
    #[test]
    fn test_box_box_separated() {
        let r = box_vs_box(
            Vec3::ZERO, Vec3::ONE,
            Vec3::new(5.0, 0.0, 0.0), Vec3::ONE,
        );
        assert!(r.is_none());
    }

    #[test]
    fn test_box_box_overlapping() {
        let r = box_vs_box(
            Vec3::ZERO, Vec3::ONE,
            Vec3::new(1.5, 0.0, 0.0), Vec3::ONE,
        );
        let (normal, depth, _pa, _pb) = r.unwrap();
        assert!(approx_vec(normal, Vec3::X));
        assert!(approx(depth, 0.5));
    }

    #[test]
    fn test_box_box_touching() {
        let r = box_vs_box(
            Vec3::ZERO, Vec3::ONE,
            Vec3::new(2.0, 0.0, 0.0), Vec3::ONE,
        );
        let (_normal, depth, _pa, _pb) = r.unwrap();
        assert!(approx(depth, 0.0));
    }

    #[test]
    fn test_box_box_y_axis() {
        let r = box_vs_box(
            Vec3::ZERO, Vec3::ONE,
            Vec3::new(0.0, 1.5, 0.0), Vec3::ONE,
        );
        let (normal, depth, _pa, _pb) = r.unwrap();
        assert!(approx_vec(normal, Vec3::Y));
        assert!(approx(depth, 0.5));
    }
```

- [ ] **Step 2: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 13개 PASS

- [ ] **Step 3: 커밋**

```bash
git add crates/voltex_physics/src/narrow.rs
git commit -m "feat(physics): add box_vs_box narrow phase (SAT)"
```

---

## Task 6: BVH Tree (Broad Phase)

**Files:**
- Create: `crates/voltex_physics/src/bvh.rs`
- Modify: `crates/voltex_physics/src/lib.rs`

- [ ] **Step 1: bvh.rs 작성**

```rust
// crates/voltex_physics/src/bvh.rs
use voltex_ecs::Entity;
use voltex_math::AABB;

#[derive(Debug)]
enum BvhNode {
    Leaf { entity: Entity, aabb: AABB },
    Internal { aabb: AABB, left: usize, right: usize },
}

#[derive(Debug)]
pub struct BvhTree {
    nodes: Vec<BvhNode>,
}

impl BvhTree {
    /// Build BVH from entity-AABB pairs using median split on longest axis.
    pub fn build(entries: &[(Entity, AABB)]) -> Self {
        let mut tree = BvhTree { nodes: Vec::new() };
        if !entries.is_empty() {
            let mut sorted: Vec<(Entity, AABB)> = entries.to_vec();
            tree.build_recursive(&mut sorted);
        }
        tree
    }

    fn build_recursive(&mut self, entries: &mut [(Entity, AABB)]) -> usize {
        if entries.len() == 1 {
            let idx = self.nodes.len();
            self.nodes.push(BvhNode::Leaf {
                entity: entries[0].0,
                aabb: entries[0].1,
            });
            return idx;
        }

        // Compute bounding AABB
        let mut combined = entries[0].1;
        for e in entries.iter().skip(1) {
            combined = combined.merged(&e.1);
        }

        // Find longest axis
        let extent = combined.max - combined.min;
        let axis = if extent.x >= extent.y && extent.x >= extent.z {
            0 // x
        } else if extent.y >= extent.z {
            1 // y
        } else {
            2 // z
        };

        // Sort by axis center
        entries.sort_by(|a, b| {
            let ca = a.1.center();
            let cb = b.1.center();
            let va = match axis { 0 => ca.x, 1 => ca.y, _ => ca.z };
            let vb = match axis { 0 => cb.x, 1 => cb.y, _ => cb.z };
            va.partial_cmp(&vb).unwrap()
        });

        let mid = entries.len() / 2;
        let (left_entries, right_entries) = entries.split_at_mut(mid);

        let left = self.build_recursive(left_entries);
        let right = self.build_recursive(right_entries);

        let idx = self.nodes.len();
        self.nodes.push(BvhNode::Internal {
            aabb: combined,
            left,
            right,
        });
        idx
    }

    /// Return all pairs of entities whose AABBs overlap.
    pub fn query_pairs(&self) -> Vec<(Entity, Entity)> {
        let mut pairs = Vec::new();
        if self.nodes.is_empty() {
            return pairs;
        }
        let root = self.nodes.len() - 1;
        self.collect_leaves_and_test(root, &mut pairs);
        pairs
    }

    fn collect_leaves_and_test(&self, node_idx: usize, pairs: &mut Vec<(Entity, Entity)>) {
        // Collect all leaves, then test all pairs.
        // For small counts this is fine; optimize later if needed.
        let mut leaves = Vec::new();
        self.collect_leaves(node_idx, &mut leaves);
        for i in 0..leaves.len() {
            for j in (i + 1)..leaves.len() {
                let (ea, aabb_a) = leaves[i];
                let (eb, aabb_b) = leaves[j];
                if aabb_a.intersects(&aabb_b) {
                    pairs.push((ea, eb));
                }
            }
        }
    }

    fn collect_leaves(&self, node_idx: usize, out: &mut Vec<(Entity, AABB)>) {
        match &self.nodes[node_idx] {
            BvhNode::Leaf { entity, aabb } => {
                out.push((*entity, *aabb));
            }
            BvhNode::Internal { left, right, .. } => {
                self.collect_leaves(*left, out);
                self.collect_leaves(*right, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_math::Vec3;

    fn make_entity(id: u32) -> Entity {
        Entity { id, generation: 0 }
    }

    #[test]
    fn test_build_empty() {
        let tree = BvhTree::build(&[]);
        assert!(tree.query_pairs().is_empty());
    }

    #[test]
    fn test_build_single() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::ONE)),
        ];
        let tree = BvhTree::build(&entries);
        assert!(tree.query_pairs().is_empty());
    }

    #[test]
    fn test_overlapping_pair() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0))),
            (make_entity(1), AABB::new(Vec3::ONE, Vec3::new(3.0, 3.0, 3.0))),
        ];
        let tree = BvhTree::build(&entries);
        let pairs = tree.query_pairs();
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn test_separated_pair() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::ONE)),
            (make_entity(1), AABB::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0))),
        ];
        let tree = BvhTree::build(&entries);
        assert!(tree.query_pairs().is_empty());
    }

    #[test]
    fn test_multiple_entities() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0))),
            (make_entity(1), AABB::new(Vec3::ONE, Vec3::new(3.0, 3.0, 3.0))),
            (make_entity(2), AABB::new(Vec3::new(10.0, 10.0, 10.0), Vec3::new(11.0, 11.0, 11.0))),
        ];
        let tree = BvhTree::build(&entries);
        let pairs = tree.query_pairs();
        // Only 0-1 overlap, 2 is far away
        assert_eq!(pairs.len(), 1);
        let (a, b) = pairs[0];
        assert!((a.id == 0 && b.id == 1) || (a.id == 1 && b.id == 0));
    }
}
```

- [ ] **Step 2: lib.rs에 bvh 모듈 등록**

```rust
pub mod bvh;
pub use bvh::BvhTree;
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 18개 PASS (collider 2 + narrow 11 + bvh 5)

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_physics/src/bvh.rs crates/voltex_physics/src/lib.rs
git commit -m "feat(physics): add BVH tree for broad phase collision detection"
```

---

## Task 7: ECS Integration — detect_collisions

**Files:**
- Create: `crates/voltex_physics/src/collision.rs`
- Modify: `crates/voltex_physics/src/lib.rs`

- [ ] **Step 1: collision.rs 작성**

```rust
// crates/voltex_physics/src/collision.rs
use voltex_ecs::{World, Entity};
use voltex_ecs::Transform;
use voltex_math::Vec3;

use crate::collider::Collider;
use crate::contact::ContactPoint;
use crate::bvh::BvhTree;
use crate::narrow;

/// Detect all collisions among entities that have Transform + Collider.
pub fn detect_collisions(world: &World) -> Vec<ContactPoint> {
    // 1. Gather entities with Transform + Collider
    let pairs_data: Vec<(Entity, Vec3, Collider)> = world
        .query2::<Transform, Collider>()
        .iter()
        .map(|(e, t, c)| (*e, t.position, *c))
        .collect();

    if pairs_data.len() < 2 {
        return Vec::new();
    }

    // 2. Build AABBs
    let entries: Vec<(Entity, voltex_math::AABB)> = pairs_data
        .iter()
        .map(|(e, pos, col)| (*e, col.aabb(*pos)))
        .collect();

    // 3. Broad phase
    let bvh = BvhTree::build(&entries);
    let broad_pairs = bvh.query_pairs();

    // 4. Narrow phase
    let mut contacts = Vec::new();

    // Build a lookup: entity_id -> (position, collider)
    // Using a simple linear scan since entity count is expected to be moderate
    let lookup = |entity: Entity| -> Option<(Vec3, Collider)> {
        pairs_data.iter().find(|(e, _, _)| *e == entity).map(|(_, p, c)| (*p, *c))
    };

    for (ea, eb) in broad_pairs {
        let (pos_a, col_a) = match lookup(ea) { Some(v) => v, None => continue };
        let (pos_b, col_b) = match lookup(eb) { Some(v) => v, None => continue };

        let result = match (&col_a, &col_b) {
            (Collider::Sphere { radius: ra }, Collider::Sphere { radius: rb }) => {
                narrow::sphere_vs_sphere(pos_a, *ra, pos_b, *rb)
            }
            (Collider::Sphere { radius }, Collider::Box { half_extents }) => {
                narrow::sphere_vs_box(pos_a, *radius, pos_b, *half_extents)
            }
            (Collider::Box { half_extents }, Collider::Sphere { radius }) => {
                // Swap order: sphere is A
                narrow::sphere_vs_box(pos_b, *radius, pos_a, *half_extents)
                    .map(|(n, d, pa, pb)| (-n, d, pb, pa)) // flip normal and points
            }
            (Collider::Box { half_extents: ha }, Collider::Box { half_extents: hb }) => {
                narrow::box_vs_box(pos_a, *ha, pos_b, *hb)
            }
        };

        if let Some((normal, depth, point_on_a, point_on_b)) = result {
            contacts.push(ContactPoint {
                entity_a: ea,
                entity_b: eb,
                normal,
                depth,
                point_on_a,
                point_on_b,
            });
        }
    }

    contacts
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::World;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;
    use crate::Collider;

    #[test]
    fn test_no_colliders() {
        let world = World::new();
        let contacts = detect_collisions(&world);
        assert!(contacts.is_empty());
    }

    #[test]
    fn test_single_entity() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        world.add(e, Collider::Sphere { radius: 1.0 });
        let contacts = detect_collisions(&world);
        assert!(contacts.is_empty());
    }

    #[test]
    fn test_two_spheres_colliding() {
        let mut world = World::new();
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.5, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);
        assert!((contacts[0].depth - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_two_spheres_separated() {
        let mut world = World::new();
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });

        let contacts = detect_collisions(&world);
        assert!(contacts.is_empty());
    }

    #[test]
    fn test_sphere_vs_box_collision() {
        let mut world = World::new();
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.5, 0.0, 0.0)));
        world.add(b, Collider::Box { half_extents: Vec3::ONE });

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].depth > 0.0);
    }

    #[test]
    fn test_box_vs_box_collision() {
        let mut world = World::new();
        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Box { half_extents: Vec3::ONE });

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.5, 0.0, 0.0)));
        world.add(b, Collider::Box { half_extents: Vec3::ONE });

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1);
        assert!((contacts[0].depth - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_three_entities_mixed() {
        let mut world = World::new();

        let a = world.spawn();
        world.add(a, Transform::from_position(Vec3::ZERO));
        world.add(a, Collider::Sphere { radius: 1.0 });

        let b = world.spawn();
        world.add(b, Transform::from_position(Vec3::new(1.5, 0.0, 0.0)));
        world.add(b, Collider::Sphere { radius: 1.0 });

        let c = world.spawn();
        world.add(c, Transform::from_position(Vec3::new(100.0, 0.0, 0.0)));
        world.add(c, Collider::Box { half_extents: Vec3::ONE });

        let contacts = detect_collisions(&world);
        assert_eq!(contacts.len(), 1); // only a-b collide
    }
}
```

- [ ] **Step 2: lib.rs에 collision 모듈 등록**

```rust
pub mod collision;
pub use collision::detect_collisions;
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p voltex_physics`
Expected: 25개 PASS (collider 2 + narrow 11 + bvh 5 + collision 7)

- [ ] **Step 4: 전체 workspace 테스트**

Run: `cargo test --workspace`
Expected: 기존 105 + AABB 6 + physics 25 = 136개 전부 PASS

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_physics/src/collision.rs crates/voltex_physics/src/lib.rs
git commit -m "feat(physics): add detect_collisions ECS integration"
```

---

## Task 8: 문서 업데이트 + 최종 커밋

**Files:**
- Modify: `docs/dev/STATUS.md`
- Modify: `docs/dev/DEFERRED.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: STATUS.md에 Phase 5-1 추가**

Phase 4c 아래에 추가:
```markdown
### Phase 5-1: Collision Detection
- voltex_math: AABB type
- voltex_physics: Collider (Sphere, Box), ContactPoint
- voltex_physics: BVH broad phase (median split)
- voltex_physics: Narrow phase (sphere-sphere, sphere-box, box-box SAT)
- voltex_physics: detect_collisions ECS integration
```

crate 구조에 `voltex_physics` 추가.
테스트 수 업데이트.
다음 항목을 Phase 5-2 (리지드바디)로 변경.

- [ ] **Step 2: DEFERRED.md에 Phase 5-1 미뤄진 항목 추가**

```markdown
## Phase 5-1

- **Capsule, Convex Hull 콜라이더** — Sphere + Box만 구현. 추후 GJK/EPA와 함께 추가.
- **OBB (회전된 박스) 충돌** — 축 정렬 AABB만 지원. OBB는 GJK/EPA로 대체 예정.
- **Incremental BVH 업데이트** — 매 프레임 전체 rebuild. 성능 이슈 시 incremental update 적용.
- **연속 충돌 감지 (CCD)** — 이산 충돌만. 빠른 물체의 터널링 미처리.
```

- [ ] **Step 3: 커밋**

```bash
git add docs/dev/STATUS.md docs/dev/DEFERRED.md CLAUDE.md
git commit -m "docs: add Phase 5-1 collision detection status and deferred items"
```
