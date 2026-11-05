# voltex_physics

## 책임 (한 줄)
충돌 감지 (GJK/EPA + BVH) + 강체 동역학 (sequential impulse) + 레이캐스트.

## 공개 API 진입점
- `Collider` — Sphere/Box/Capsule/ConvexHull (src/collider.rs)
- `ContactPoint` — 좁은 단계 충돌 (src/contact.rs)
- `gjk` — narrow 모듈 (src/gjk.rs, src/narrow.rs)
- `BvhTree` — 넓은 단계 (src/bvh.rs). `query_ray`·`refit` 은 구현·테스트만 있고 엔진 안에서는 호출하지 않는다
- `RigidBody` — 질량/속도/각속도/마찰/sleep. `dynamic(mass)`/`fixed()`(옛 이름 `statik`, deprecated) (src/rigid_body.rs)
- `integrate`, `inertia_tensor`, `inv_inertia` — Semi-implicit Euler (src/integrator.rs)
- `physics_step`, `resolve_collisions` — Sequential Impulse solver (src/solver.rs)
- raycast — `raycast`, `raycast_all` → `RayHit { entity, distance, point, normal }`, ray_vs_triangle, ray_vs_plane (src/raycast.rs, src/ray_shapes.rs)
- `detect_collisions` — ECS 통합 (src/collision.rs)
- CCD — `swept_sphere_vs_aabb` (src/ccd.rs)
- 2D 타일 충돌 — `move_and_collide_2d`, `MoveResult2D` (src/grid2d.rs): XY 평면 AABB vs `is_solid(gx,gy)` 클로저 그리드, 축 분리(X→Y) 스냅 해소 + 접지 프로브. 경계 정책은 호출자 소유, 프레임 이동량 < tile 전제. leap physics2d 승격분 (friction-2 #4)

## 모듈 구조
- `collider` — 콜라이더 타입 (src/collider.rs)
- `gjk`, `narrow`, `contact` — 좁은 단계 (src/gjk.rs 등)
- `bvh` — 넓은 단계 (src/bvh.rs)
- `rigid_body`, `integrator`, `solver` — 강체 동역학 (src/rigid_body.rs 등)
- `ray_shapes`, `raycast` — 레이 인터섹션 (src/ray_shapes.rs, src/raycast.rs). 옛 모듈 경로 `ray`는 deprecated 재export
- `collision` — ECS 통합 (src/collision.rs). crate 내부 `placed_colliders`가 `(Transform, Collider)` 엔티티를 모아 충돌·레이캐스트가 같이 쓴다
- `contact` — `ContactPoint` + narrowphase 결과 `Penetration { normal, depth, point_on_a, point_on_b }`(crate 내부, `flipped()`로 A/B 뒤집기) (src/contact.rs)
- `ccd` — Continuous Collision Detection (src/ccd.rs): `swept_sphere_vs_aabb` + `physics_step`이 integrate 앞뒤로 부르는 `snapshot_positions`/`clamp_tunnelling`(crate 내부)
- `grid2d` — 2D 타일 그리드 충돌 (src/grid2d.rs)
- `experimental` — 구현·단위 테스트는 되어 있지만 엔진 런타임이 쓰지 않는 모듈. API가 바뀌거나 없어질 수 있다 (src/experimental/)
  - `mesh_collider` — `MeshCollider`(정점 + 인덱스 삼각형), `MeshHit`, `ray_vs_mesh`, `ray_vs_mesh_all`. `Collider` variant 가 아니라 충돌 감지·`raycast` 에 안 잡힌다
  - 옛 경로(`voltex_physics::MeshCollider`, `voltex_physics::mesh_collider` 등)는 src/deprecated_paths.rs 의 deprecated 별칭

## 의존성
- voltex crate: voltex_math, voltex_ecs
- 외부 crate: 없음

## 변경 시 주의점
- detect_collisions(world)는 ECS와 강하게 결합 — World 구조 변경 시 영향
- CCD는 sphere vs AABB만 구현 (다른 형상은 DEFERRED). 시작 시점에 이미 겹친 콜라이더(t=0)는 CCD가 무시하고 이산 솔버에 맡긴다. 확장 AABB 근사라 모서리 근처에선 실제보다 일찍 접촉으로 본다.

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_physics --lib`
- 현재 테스트 개수: 119개
