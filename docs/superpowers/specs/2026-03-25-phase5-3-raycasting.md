# Phase 5-3: Raycasting — Design Spec

## Overview

`voltex_math`에 Ray 타입을 추가하고, `voltex_physics`에 기하 교차 함수와 BVH 가속 레이캐스트를 구현한다.

## Scope

- Ray 타입 (voltex_math)
- ray_vs_aabb, ray_vs_sphere, ray_vs_box 교차 함수
- BVH 가속 ECS 레이캐스트: `raycast(world, ray, max_dist) -> Option<RayHit>`

## Out of Scope

- Ray vs Plane, Triangle, Mesh
- 연속 레이캐스트 (sweep)
- 다중 hit 반환 (raycast_all)

## Module Structure

### voltex_math (수정)
- `crates/voltex_math/src/ray.rs` — Ray 타입 (Create)
- `crates/voltex_math/src/lib.rs` — Ray 모듈 등록 (Modify)

### voltex_physics (추가)
- `crates/voltex_physics/src/ray.rs` — ray_vs_aabb, ray_vs_sphere, ray_vs_box (Create)
- `crates/voltex_physics/src/raycast.rs` — RayHit, raycast ECS 통합 (Create)
- `crates/voltex_physics/src/lib.rs` — 새 모듈 등록 (Modify)

## Types

### Ray (voltex_math)

```rust
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}
```

- `new(origin, direction) -> Self` — direction을 정규화하여 저장
- `at(t: f32) -> Vec3` — `origin + direction * t`

### RayHit (voltex_physics)

```rust
#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub entity: Entity,
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
}
```

## Functions

### ray_vs_aabb (voltex_physics::ray)

```rust
pub fn ray_vs_aabb(ray: &Ray, aabb: &AABB) -> Option<f32>
```

Slab method. t값만 반환 (BVH 순회용, 법선 불필요).
ray가 AABB 내부에서 시작하면 t=0 반환.

### ray_vs_sphere (voltex_physics::ray)

```rust
pub fn ray_vs_sphere(ray: &Ray, center: Vec3, radius: f32) -> Option<(f32, Vec3)>
```

이차방정식 풀이. (t, normal) 반환.
ray가 구 내부에서 시작하면 far intersection 반환.

### ray_vs_box (voltex_physics::ray)

```rust
pub fn ray_vs_box(ray: &Ray, center: Vec3, half_extents: Vec3) -> Option<(f32, Vec3)>
```

Slab method + 진입 면에서 법선 계산. (t, normal) 반환.
ray가 박스 내부에서 시작하면 t=0, 진행 방향 기준 법선 반환.

### raycast (voltex_physics::raycast)

```rust
pub fn raycast(world: &World, ray: &Ray, max_dist: f32) -> Option<RayHit>
```

1. Transform + Collider 가진 entity 수집, AABB 계산
2. BvhTree::build()로 BVH 구축
3. 모든 리프를 순회하며 ray_vs_aabb로 broad phase
4. AABB hit인 경우 콜라이더 타입별 정밀 교차:
   - Sphere → ray_vs_sphere
   - Box → ray_vs_box
5. t < max_dist인 hit 중 가장 가까운 것 반환

NOTE: BVH 조기 종료 최적화는 추후. 첫 구현은 모든 리프 검사 후 최소 t 선택.

## Conventions

- Ray direction은 항상 단위 벡터
- t >= 0 인 교차만 유효 (ray 뒤쪽 무시)
- normal은 ray가 진입하는 면의 바깥 방향
- max_dist: t의 상한

## Test Plan

### voltex_math (Ray)
- new: direction 정규화 확인
- at: 정확한 점 계산

### voltex_physics (ray.rs)
- ray_vs_aabb: hit, miss, 내부 시작 (t=0)
- ray_vs_sphere: hit (t, normal 정확도), miss, 접선, 내부 시작
- ray_vs_box: 각 면 hit, miss, 내부 시작

### voltex_physics (raycast.rs)
- 빈 world → None
- 단일 entity hit
- 여러 entity 중 가장 가까운 것 반환
- miss (max_dist 초과)
- Sphere + Box 혼합
