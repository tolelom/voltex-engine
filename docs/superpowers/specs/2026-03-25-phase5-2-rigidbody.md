# Phase 5-2: Rigid Body Simulation — Design Spec

## Overview

`voltex_physics`에 리지드바디 시뮬레이션을 추가한다.
Semi-implicit Euler 적분 + 임펄스 기반 충돌 응답으로, 물체가 떨어지고 충돌하여 튕기는 기본 물리를 구현한다.

## Scope

- RigidBody 컴포넌트 (질량, 속도, 반발 계수)
- 중력 + Semi-implicit Euler 적분
- 임펄스 기반 충돌 응답 (선형만)
- 위치 보정 (penetration resolution)
- physics_step 통합 함수

## Out of Scope

- 각속도/회전 물리 (관성 텐서 필요, 추후 추가)
- 마찰 (Coulomb friction)
- Sequential Impulse 솔버
- 연속 충돌 감지 (CCD)
- Sleep/Island 시스템

## Module Structure

기존 `voltex_physics`에 추가:

```
crates/voltex_physics/src/
├── (기존) collider.rs, contact.rs, narrow.rs, bvh.rs, collision.rs, lib.rs
├── rigid_body.rs    — RigidBody 컴포넌트, PhysicsConfig
├── integrator.rs    — Semi-implicit Euler 적분
└── solver.rs        — 임펄스 기반 충돌 응답 + 위치 보정 + physics_step
```

## Types

### RigidBody (ECS 컴포넌트)

```rust
#[derive(Debug, Clone, Copy)]
pub struct RigidBody {
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
    pub restitution: f32,
    pub gravity_scale: f32,
}
```

- `mass == 0.0` → 정적 물체 (무한 질량, 중력 영향 없음, 움직이지 않음)
- `mass > 0.0` → 동적 물체
- `restitution` — 반발 계수 (0.0 = 완전 비탄성, 1.0 = 완전 탄성)
- `gravity_scale` — 중력 배율 (기본 1.0)
- `angular_velocity` — 필드만 존재, 이번 Phase에서는 적분하지 않음

**Methods:**
- `dynamic(mass: f32) -> Self` — 동적 물체 생성 (velocity=0, restitution=0.3, gravity_scale=1.0)
- `statik() -> Self` — 정적 물체 생성 (mass=0, velocity=0, restitution=0.3)
- `inv_mass(&self) -> f32` — mass가 0이면 0.0, 아니면 1.0/mass
- `is_static(&self) -> bool` — mass == 0.0

### PhysicsConfig

```rust
pub struct PhysicsConfig {
    pub gravity: Vec3,
    pub fixed_dt: f32,
}
```

- `default()` → gravity = (0, -9.81, 0), fixed_dt = 1.0/60.0

## Functions

### integrate (integrator.rs)

```rust
pub fn integrate(world: &mut World, config: &PhysicsConfig)
```

Transform + RigidBody를 가진 동적 entity 순회:
1. `velocity += gravity * gravity_scale * dt`
2. `position += velocity * dt`

정적 물체(mass == 0.0)는 건너뜀.

### resolve_collisions (solver.rs)

```rust
pub fn resolve_collisions(world: &mut World, contacts: &[ContactPoint])
```

각 ContactPoint에 대해:
1. 두 entity의 RigidBody 조회 (없으면 스킵)
2. 상대 속도 계산: `v_rel = velocity_a - velocity_b`
3. 법선 방향 성분: `v_rel_n = v_rel · normal`
4. 분리 중이면 스킵: `v_rel_n > 0`
5. 반발 계수: `e = min(restitution_a, restitution_b)`
6. 임펄스 크기: `j = -(1 + e) * v_rel_n / (inv_mass_a + inv_mass_b)`
7. 속도 업데이트: `v_a += j * normal * inv_mass_a`, `v_b -= j * normal * inv_mass_b`

**위치 보정 (Positional Correction):**
- 침투 깊이(depth)에 비례하여 물체를 법선 방향으로 분리
- Baumgarte stabilization: `correction = max(depth - slop, 0) * percent / (inv_mass_a + inv_mass_b)`
- slop = 0.01 (작은 침투 허용), percent = 0.4 (보정 비율)
- `pos_a -= correction * inv_mass_a * normal`, `pos_b += correction * inv_mass_b * normal`

### physics_step (solver.rs)

```rust
pub fn physics_step(world: &mut World, config: &PhysicsConfig)
```

1. `integrate(world, config)`
2. `let contacts = detect_collisions(world)`
3. `resolve_collisions(world, &contacts)`

## ECS Integration Notes

- RigidBody는 Transform + Collider와 함께 entity에 추가
- 물리 스텝은 게임 루프에서 fixed_update 타이밍에 호출
- detect_collisions는 Phase 5-1의 기존 함수 재사용

## Conventions

- 단위: SI (미터, 초, 킬로그램)
- 중력: (0, -9.81, 0) — Y-up 좌표계
- 법선: Phase 5-1과 동일 (entity_a → entity_b 방향)

## Test Plan

### rigid_body.rs
- `dynamic()`: 기본값 확인, inv_mass 정확도
- `statik()`: mass=0, inv_mass=0, is_static=true
- `with_restitution`: 설정 반영

### integrator.rs
- 중력 낙하: 1프레임 후 위치 = (0, -9.81/60/60 ≈ position 변화)
- 정적 물체: integrate 후 위치 불변
- 초기 속도: velocity가 있는 경우 position 변화 확인

### solver.rs
- 두 동적 구체 정면 충돌: 속도 반전 확인
- 동적 구체 vs 정적 바닥: 구체만 튕김, 바닥 불변
- 위치 보정: 침투 깊이만큼 분리
- physics_step E2E: 구체가 바닥 위에서 낙하 → 충돌 → 반발
