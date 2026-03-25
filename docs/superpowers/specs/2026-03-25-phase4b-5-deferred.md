# Phase 4b-5 Deferred Items Spec

## A. 그림자 확장 + 라이트 컬링 (`voltex_renderer`)

### CSM (2 캐스케이드)
- `CascadedShadowMap` — 2개 depth 텍스처 (near 0~20m, far 20~100m), 각 2048x2048
- 카메라 프러스텀 분할, 캐스케이드별 light VP 행렬 계산
- `ShadowUniform` 확장: `light_view_proj` 2개 + `cascade_split` f32
- 셰이더: fragment depth로 캐스케이드 선택, 해당 맵에서 PCF 샘플링

### Point Light Shadow (큐브맵)
- `PointShadowMap` — 6면 depth 큐브맵 (512x512 per face)
- 6방향 view matrix (±X, ±Y, ±Z), 90° perspective projection
- 셰이더: light→fragment 방향으로 큐브맵 샘플링
- MAX 2개 포인트 라이트 그림자 (성능상 제한)

### Spot Light Shadow
- 기존 ShadowMap 패턴 재사용 (perspective projection)
- spot cone outer angle → FOV 변환
- MAX 2개 스팟 라이트 그림자

### CPU 프러스텀 라이트 컬링
```rust
pub struct Frustum { planes: [Plane; 6] }
pub fn extract_frustum(view_proj: &Mat4) -> Frustum
pub fn cull_lights(frustum: &Frustum, lights: &[LightData]) -> Vec<usize>
```
- Sphere vs 6 plane 테스트
- Directional: 항상 포함, Point: position+range 구, Spot: cone 바운딩 구

---

## B. IBL 개선 + GPU BRDF LUT (`voltex_renderer`)

### 프로시저럴 스카이 개선
- `sample_environment` → Hosek-Wilkie 근사 sky model
- `SkyParams` uniform: sun_direction, turbidity
- 태양 디스크 렌더링 (pow falloff)
- 기존 하드코딩 gradient → 물리 기반 산란

### SH Irradiance
- `compute_sh_coefficients(sun_dir, turbidity) -> [Vec3; 9]` — L2 SH 9계수
- CPU에서 환경맵 반구 적분 → SH 계수
- 셰이더: `evaluate_sh(normal, coeffs)` — O(1) irradiance 조회
- 기존 `sample_environment(N, 1.0)` 대체

### GPU 컴퓨트 BRDF LUT
- `brdf_lut_compute.wgsl` — 컴퓨트 셰이더
- Workgroup 16x16, 1024 importance samples per texel
- 출력: `Rg16Float` (기존 Rgba8Unorm → 정밀도 향상)
- `IblResources::new_gpu(device, queue)` — 컴퓨트 패스로 생성

---

## C. 물리 개선 (`voltex_physics`)

### 각속도/회전 물리
- `integrator.rs`: angular_velocity 적분 → rotation 업데이트
- 관성 텐서 근사: `inertia_tensor(collider, mass) -> Vec3` (대각 성분만)
  - Sphere: 2/5 mr², Box: 1/12 m(h²+d²), Capsule: 근사
- `solver.rs`: 충돌 토크 `τ = r × impulse` → angular impulse / inertia

### Sequential Impulse 솔버
- `PhysicsConfig.solver_iterations: u32` (기본 4)
- 접촉점별 반복: 매 iteration마다 상대속도 재계산 후 추가 임펄스
- 기존 단일 반복 코드를 N회 루프로 래핑

### Sleep/Island 시스템
- `RigidBody` 확장: `is_sleeping: bool`, `sleep_timer: f32`
- `SLEEP_VELOCITY_THRESHOLD: f32 = 0.01`
- `SLEEP_TIME_THRESHOLD: f32 = 0.5` (0.5초 정지 시 sleep)
- sleeping 바디: integrate/collision 스킵
- 충돌 시 wake_up

### Ray vs Triangle/Mesh
- `ray_vs_triangle(ray, v0, v1, v2) -> Option<(f32, Vec3)>` — Möller–Trumbore
- `ray_vs_mesh(ray, vertices, indices) -> Option<(f32, Vec3)>` — 삼각형 순회

### raycast_all
- `raycast_all(world, ray, max_dist) -> Vec<RayHit>` — 모든 hit, 거리순 정렬

### BVH 개선
- `query_pairs` → 재귀 트리 순회 (현재 N² brute force 제거)
- raycast: front-to-back 순회, t_min > current_best 스킵
- `refit()` — 리프 AABB 갱신 후 부모 전파 (incremental update)

### CCD
- `swept_sphere_vs_aabb(start, end, radius, aabb) -> Option<f32>` — 이동 구 vs AABB
- physics_step: `|velocity| * dt > radius` 인 물체만 CCD 적용
