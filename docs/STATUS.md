# Voltex Engine - 작업 현황

## 완료된 Phase

### Phase 1: Foundation (삼각형 렌더링)
- voltex_math: Vec3
- voltex_platform: VoltexWindow, InputState, GameTimer
- voltex_renderer: GpuContext, Vertex, shader, pipeline
- examples/triangle

### Phase 2: Rendering Basics
- voltex_math: Vec2, Vec4, Mat4 (transforms, look_at, perspective, orthographic)
- voltex_renderer: MeshVertex(+tangent), Mesh, depth buffer, OBJ parser, Camera, FpsController
- voltex_renderer: Blinn-Phong shader, BMP texture loader, GpuTexture
- examples/model_viewer

### Phase 3a: ECS
- voltex_ecs: Entity(id+generation), SparseSet<T>, World(type-erased storage)
- voltex_ecs: query<T>, query2<A,B>, Transform component
- examples/many_cubes (400 entities, dynamic UBO)

### Phase 3b: Scene Graph
- voltex_ecs: Parent/Children hierarchy, add_child/remove_child/despawn_recursive
- voltex_ecs: WorldTransform propagation (top-down)
- voltex_ecs: Scene serialization (.vscn text format), Tag component
- examples/hierarchy_demo (solar system)

### Phase 3c: Asset Manager
- voltex_asset: Handle<T>(generation), AssetStorage<T>(ref counting), Assets(type-erased)
- examples/asset_demo

### Phase 4a: PBR Rendering
- voltex_renderer: MaterialUniform (base_color, metallic, roughness, ao)
- voltex_renderer: Cook-Torrance BRDF shader (GGX NDF + Smith geometry + Fresnel-Schlick)
- voltex_renderer: Procedural UV sphere generator
- voltex_renderer: PBR pipeline (3→4 bind groups)
- examples/pbr_demo (7x7 metallic/roughness sphere grid)

### Phase 4b-1: Multi-Light
- voltex_renderer: LightData (Directional/Point/Spot), LightsUniform (MAX_LIGHTS=16)
- PBR shader: multi-light loop, point attenuation, spot cone falloff
- examples/multi_light_demo (orbiting colored point lights)

### Phase 4b-2: Shadow Mapping
- voltex_renderer: ShadowMap (2048x2048 depth), ShadowUniform, ShadowPassUniform
- Shadow depth-only shader + pipeline (front-face cull, depth bias)
- PBR shader: shadow map sampling + 3x3 PCF
- examples/shadow_demo (directional light shadows)

### Phase 4c: Normal Map + IBL
- MeshVertex: tangent[4] added, computed in OBJ parser + sphere generator
- voltex_renderer: BRDF LUT (CPU Monte Carlo, 256x256), IblResources
- PBR shader: TBN normal mapping, procedural sky IBL, split-sum approximation
- Texture bind group: albedo + normal map (pbr_texture_bind_group_layout)
- IBL merged into shadow bind group (group 3) due to max_bind_groups=4
- examples/ibl_demo

### Phase 5-1: Collision Detection
- voltex_math: AABB type (new, from_center_half_extents, intersects, merged, surface_area)
- voltex_physics: Collider enum (Sphere, Box), ContactPoint
- voltex_physics: BVH broad phase (median split, longest axis)
- voltex_physics: Narrow phase — sphere_vs_sphere, sphere_vs_box, box_vs_box (SAT)
- voltex_physics: detect_collisions(world) ECS integration

### Phase 5-2: Rigid Body Simulation
- voltex_physics: RigidBody (mass, velocity, restitution), PhysicsConfig
- voltex_physics: Semi-implicit Euler integration
- voltex_physics: Impulse-based collision response + positional correction (Baumgarte)
- voltex_physics: physics_step (integrate → detect → resolve)

### Phase 5-3: Raycasting
- voltex_math: Ray type (origin, direction, at)
- voltex_physics: ray_vs_aabb, ray_vs_sphere, ray_vs_box
- voltex_physics: raycast(world, ray, max_dist) BVH-accelerated ECS integration

## Crate 구조

```
crates/
├── voltex_math      — Vec2, Vec3, Vec4, Mat4, AABB, Ray
├── voltex_platform  — VoltexWindow, InputState, GameTimer
├── voltex_renderer  — GPU, Mesh, OBJ, Camera, Material, PBR, Shadow, IBL, Sphere
├── voltex_ecs       — Entity, SparseSet, World, Transform, Hierarchy, Scene, WorldTransform
├── voltex_asset     — Handle<T>, AssetStorage<T>, Assets
└── voltex_physics   — Collider, ContactPoint, BvhTree, RigidBody, detect_collisions, physics_step, raycast
```

## 테스트: 165개 전부 통과

- voltex_asset: 14
- voltex_ecs: 39
- voltex_math: 37 (29 + AABB 6 + Ray 2)
- voltex_physics: 52 (collider 2 + narrow 11 + bvh 5 + collision 7 + rigid_body 3 + integrator 3 + solver 5 + ray 10 + raycast 6)
- voltex_platform: 3
- voltex_renderer: 20

## Examples (9개)

- triangle — Phase 1 삼각형
- model_viewer — OBJ 큐브 + Blinn-Phong
- many_cubes — 400 ECS 엔티티 렌더링
- hierarchy_demo — 태양계 씬 그래프
- asset_demo — Handle 기반 에셋 관리
- pbr_demo — metallic/roughness 구체 그리드
- multi_light_demo — 다중 색상 라이트
- shadow_demo — Directional Light 그림자
- ibl_demo — Normal map + IBL

## 다음: Phase 6 (오디오 시스템)

스펙 참조: `docs/superpowers/specs/2026-03-24-voltex-engine-design.md`

## 간소화/미뤄진 항목

상세: `docs/DEFERRED.md`
