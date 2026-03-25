# Phase 5-1: Collision Detection — Design Spec

## Overview

`voltex_physics` crate에 충돌 감지 시스템을 구현한다.
Broad phase (BVH) + Narrow phase (전용 함수) 구조로, Sphere와 Box 콜라이더를 지원한다.

## Dependencies

- `voltex_math` — Vec3, Mat4, AABB(신규)
- `voltex_ecs` — World, Entity, Transform, SparseSet

## Module Structure

```
crates/voltex_physics/
├── Cargo.toml
└── src/
    ├── lib.rs          — public exports
    ├── aabb.rs         — AABB type (voltex_math에 추가)
    ├── collider.rs     — Collider enum
    ├── contact.rs      — ContactPoint
    ├── bvh.rs          — BvhTree (broad phase)
    ├── narrow.rs       — sphere_vs_sphere, sphere_vs_box, box_vs_box
    └── collision.rs    — detect_collisions (ECS 통합)
```

참고: AABB는 `voltex_math`에 추가한다 (스펙에 명시된 기본 수학 타입).

## Types

### AABB (voltex_math에 추가)

```rust
#[derive(Debug, Clone, Copy)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}
```

**Methods:**
- `new(min, max) -> Self`
- `from_center_half_extents(center, half_extents) -> Self`
- `center() -> Vec3`
- `half_extents() -> Vec3`
- `contains_point(point: Vec3) -> bool`
- `intersects(other: &AABB) -> bool`
- `merged(other: &AABB) -> AABB` — 두 AABB를 감싸는 최소 AABB
- `surface_area() -> f32` — BVH SAH 비용 계산용

### Collider (voltex_physics)

```rust
#[derive(Debug, Clone, Copy)]
pub enum Collider {
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
}
```

- ECS 컴포넌트로 사용. 형상(shape)만 저장.
- 위치는 같은 entity의 `Transform.position`에서 가져온다.
- `aabb(&self, position: Vec3) -> AABB` — broad phase용 바운딩 박스 생성

### ContactPoint (voltex_physics)

```rust
#[derive(Debug, Clone, Copy)]
pub struct ContactPoint {
    pub entity_a: Entity,
    pub entity_b: Entity,
    pub normal: Vec3,      // A에서 B 방향 단위 법선
    pub depth: f32,        // 침투 깊이 (양수 = 겹침)
    pub point_on_a: Vec3,  // A 표면의 접촉점
    pub point_on_b: Vec3,  // B 표면의 접촉점
}
```

### BvhTree (voltex_physics)

바이너리 트리. 각 리프는 하나의 Entity + AABB.

```rust
pub struct BvhTree {
    nodes: Vec<BvhNode>,
}

enum BvhNode {
    Leaf { entity: Entity, aabb: AABB },
    Internal { aabb: AABB, left: usize, right: usize },
}
```

**Methods:**
- `build(entries: &[(Entity, AABB)]) -> Self` — 중앙값 분할(median split)로 구축. 가장 긴 축 기준 정렬 후 이분.
- `query_pairs(&self) -> Vec<(Entity, Entity)>` — 트리 순회로 AABB가 겹치는 리프 쌍 반환.

매 프레임 rebuild한다 (동적 씬 대응). 최적화(incremental update)는 성능 문제 발생 시 추후 적용.

### Narrow Phase Functions (voltex_physics::narrow)

모든 함수는 위치(Vec3)와 형상 파라미터를 받아 `Option<ContactPoint>`를 반환한다.
Entity 정보는 호출부에서 채운다.

```rust
pub fn sphere_vs_sphere(
    pos_a: Vec3, radius_a: f32,
    pos_b: Vec3, radius_b: f32,
) -> Option<(Vec3, f32, Vec3, Vec3)>  // (normal, depth, point_a, point_b)

pub fn sphere_vs_box(
    sphere_pos: Vec3, radius: f32,
    box_pos: Vec3, half_extents: Vec3,
) -> Option<(Vec3, f32, Vec3, Vec3)>

pub fn box_vs_box(
    pos_a: Vec3, half_a: Vec3,
    pos_b: Vec3, half_b: Vec3,
) -> Option<(Vec3, f32, Vec3, Vec3)>
```

**box_vs_box**: SAT (Separating Axis Theorem) 기반. 축 정렬(AABB vs AABB)만 지원한다.
회전된 OBB는 Convex Hull 추가 시 GJK/EPA로 대체.

### ECS Integration (voltex_physics::collision)

```rust
pub fn detect_collisions(world: &World) -> Vec<ContactPoint>
```

1. `Transform` + `Collider`를 가진 entity를 `query2`로 수집
2. 각 entity의 AABB 계산 (`collider.aabb(transform.position)`)
3. `BvhTree::build()` → `query_pairs()`로 broad phase 후보 추출
4. 각 후보 쌍에 대해 collider 타입 조합에 맞는 narrow phase 함수 호출
5. 접촉이 있으면 `ContactPoint`에 entity 정보를 채워 결과에 추가

## Conventions

- 법선(normal): entity_a에서 entity_b 방향
- 침투 깊이(depth): 양수 = 겹침, 0 이하 = 접촉 없음
- 좌표계: 기존 voltex_math 규약 (오른손 좌표계)
- WGSL vec3 alignment 규칙은 AABB에 해당 없음 (GPU에 올리지 않음)

## Test Plan

### voltex_math (AABB)
- `new`, `from_center_half_extents` 생성
- `center`, `half_extents` 계산
- `contains_point`: 내부/외부/경계
- `intersects`: 겹침/분리/접선
- `merged`: 두 AABB 합집합
- `surface_area`: 정확도

### voltex_physics
- **narrow::sphere_vs_sphere**: 겹침, 분리, 접선, 완전 포함
- **narrow::sphere_vs_box**: 면/모서리/꼭짓점 접촉, 분리, 내부 포함
- **narrow::box_vs_box**: 각 축 겹침, 분리, 접선
- **collider::aabb**: Sphere/Box의 AABB 생성
- **bvh::build**: 빈 입력, 단일, 다수 엔트리
- **bvh::query_pairs**: 겹치는 쌍 정확성, 분리된 쌍 미포함
- **collision::detect_collisions**: ECS 통합 E2E

## Out of Scope (Phase 5-1)

- Capsule, Convex Hull 콜라이더
- GJK/EPA 알고리즘
- 회전된 박스(OBB) 충돌
- 리지드바디 시뮬레이션 (Phase 5-2)
- 레이캐스팅 (Phase 5-3)
- 연속 충돌 감지 (CCD)
