# voltex_physics

## 책임 (한 줄)
충돌 감지 (GJK/EPA + BVH) + 강체 동역학 (sequential impulse) + 레이캐스트.

## 공개 API 진입점
- `Collider` — Sphere/Box/Capsule/ConvexHull (src/collider.rs)
- `MeshCollider` — triangle mesh (src/mesh_collider.rs)
- `ContactPoint` — 좁은 단계 충돌 (src/contact.rs)
- `gjk` — narrow 모듈 (src/gjk.rs, src/narrow.rs)
- `BvhTree` — 넓은 단계 + refit + query_ray (src/bvh.rs)
- `RigidBody` — 질량/속도/각속도/마찰/sleep (src/rigid_body.rs)
- `integrate`, `inertia_tensor`, `inv_inertia` — Semi-implicit Euler (src/integrator.rs)
- `physics_step`, `resolve_collisions` — Sequential Impulse solver (src/solver.rs)
- raycast — `raycast`, `raycast_all`, ray_vs_triangle, ray_vs_plane, ray_vs_mesh (src/raycast.rs, src/ray.rs)
- `detect_collisions` — ECS 통합 (src/collision.rs)
- CCD — `swept_sphere_vs_aabb` (src/ccd.rs)
- 2D 타일 충돌 — `move_and_collide_2d`, `MoveResult2D` (src/grid2d.rs): XY 평면 AABB vs `is_solid(gx,gy)` 클로저 그리드, 축 분리(X→Y) 스냅 해소 + 접지 프로브. 경계 정책은 호출자 소유, 프레임 이동량 < tile 전제. voltex_leap physics2d 승격분 (friction-2 #4)

## 모듈 구조
- `collider`, `mesh_collider` — 콜라이더 타입 (src/collider.rs, src/mesh_collider.rs)
- `gjk`, `narrow`, `contact` — 좁은 단계 (src/gjk.rs 등)
- `bvh` — 넓은 단계 (src/bvh.rs)
- `rigid_body`, `integrator`, `solver` — 강체 동역학 (src/rigid_body.rs 등)
- `ray`, `raycast` — 레이 인터섹션 (src/ray.rs, src/raycast.rs)
- `collision` — ECS 통합 (src/collision.rs)
- `ccd` — Continuous Collision Detection (src/ccd.rs)
- `grid2d` — 2D 타일 그리드 충돌 (src/grid2d.rs)

## 의존성
- voltex crate: voltex_math, voltex_ecs
- 외부 crate: 없음

## 변경 시 주의점
- solver.rs(574줄), gjk.rs(524줄)는 spec 만족 (600줄 이하), 분리 대상 아님
- detect_collisions(world)는 ECS와 강하게 결합 — World 구조 변경 시 영향
- CCD는 sphere vs AABB만 구현 (다른 형상은 DEFERRED)

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_physics --lib`
- 현재 테스트 개수: 119개
