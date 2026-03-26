# Voltex Engine - 작업 현황

## 완료된 Phase

### Phase 1: Foundation (삼각형 렌더링)
- voltex_math: Vec3
- voltex_platform: VoltexWindow, InputState, GameTimer
- voltex_renderer: GpuContext, Vertex, shader, pipeline
- examples/triangle

### Phase 2: Rendering Basics
- voltex_math: Vec2, Vec4, Mat4 (transforms, look_at, perspective, orthographic, inverse)
- voltex_renderer: MeshVertex(+tangent), Mesh, depth buffer, OBJ parser, Camera, FpsController
- voltex_renderer: Blinn-Phong shader, BMP texture loader, GpuTexture
- voltex_renderer: PNG decoder (deflate + filter reconstruction)
- voltex_renderer: JPG decoder (Baseline JPEG: Huffman, IDCT, MCU, YCbCr, subsampling)
- voltex_renderer: glTF 2.0 / GLB parser (JSON parser, accessor extraction, PBR material, nodes/skins/animations)
- examples/model_viewer

### Phase 3a: ECS
- voltex_ecs: Entity(id+generation), SparseSet<T>, World(type-erased storage)
- voltex_ecs: query<T>, query2<A,B>, query3, query4, Transform component
- voltex_ecs: query_with/query_without, query2_with/query2_without (With/Without filters)
- voltex_ecs: System trait, Scheduler (ordered execution)
- examples/many_cubes (400 entities, dynamic UBO)

### Phase 3b: Scene Graph + Serialization
- voltex_ecs: Parent/Children hierarchy, add_child/remove_child/despawn_recursive
- voltex_ecs: WorldTransform propagation (top-down)
- voltex_ecs: Scene serialization (.vscn text, JSON, binary VSCN format), Tag component
- voltex_ecs: ComponentRegistry (등록 기반 임의 컴포넌트 직렬화)
- voltex_ecs: Mini JSON writer/parser (자체 구현)
- examples/hierarchy_demo (solar system)

### Phase 3c: Asset Manager
- voltex_asset: Handle<T>(generation), AssetStorage<T>(ref counting), Assets(type-erased)
- voltex_asset: AssetLoader (워커 스레드 비동기 로딩, LoadState)
- voltex_asset: FileWatcher (mtime 폴링 변경 감지), replace_in_place (핫 리로드)
- examples/asset_demo

### Phase 4a: PBR Rendering
- voltex_renderer: MaterialUniform (base_color, metallic, roughness, ao)
- voltex_renderer: Cook-Torrance BRDF shader (GGX NDF + Smith geometry + Fresnel-Schlick)
- voltex_renderer: ORM 텍스처 맵 (AO/Roughness/Metallic) + Emissive 텍스처
- voltex_renderer: Full PBR texture layout (8 bindings: albedo+normal+ORM+emissive)
- voltex_renderer: Procedural UV sphere generator
- examples/pbr_demo (7x7 metallic/roughness sphere grid)

### Phase 4b: Lighting + Shadows
- voltex_renderer: LightData (Directional/Point/Spot), LightsUniform (MAX_LIGHTS=16)
- voltex_renderer: ShadowMap (2048x2048, 3x3 PCF)
- voltex_renderer: CascadedShadowMap (2 캐스케이드, 프러스텀 분할)
- voltex_renderer: PointShadowMap (큐브맵 6면), SpotShadowMap (perspective)
- voltex_renderer: Frustum 라이트 컬링 (Gribb-Hartmann, sphere vs 6 planes)
- examples/shadow_demo, multi_light_demo

### Phase 4c: Normal Map + IBL
- voltex_renderer: TBN normal mapping, Hosek-Wilkie procedural sky
- voltex_renderer: SH Irradiance (L2, 9계수, CPU 계산, 셰이더 평가)
- voltex_renderer: GPU Compute BRDF LUT (Rg16Float, 컴퓨트 셰이더)
- voltex_renderer: SkyParams (sun_direction, turbidity)
- examples/ibl_demo

### Phase 5-1: Collision Detection
- voltex_math: AABB, Ray
- voltex_physics: Collider (Sphere, Box, Capsule), ContactPoint, GJK/EPA
- voltex_physics: BVH (재귀 트리 순회, query_ray, refit incremental)
- voltex_physics: detect_collisions(world) ECS integration

### Phase 5-2: Rigid Body Simulation
- voltex_physics: RigidBody (mass, velocity, angular_velocity, friction, sleep)
- voltex_physics: Semi-implicit Euler (linear + angular, 관성 텐서)
- voltex_physics: Sequential Impulse solver (N회 반복, angular impulse)
- voltex_physics: Sleep/Island (velocity threshold + timer)
- voltex_physics: CCD (swept_sphere_vs_aabb)

### Phase 5-3: Raycasting
- voltex_physics: ray_vs_aabb, ray_vs_sphere, ray_vs_box, ray_vs_capsule, ray_vs_triangle
- voltex_physics: raycast (closest hit), raycast_all (다중 hit, 거리순)
- voltex_physics: BVH-accelerated query_ray

### Phase 6-1: Audio System Foundation
- voltex_audio: WAV parser (PCM 16/24/32-bit, mono/stereo)
- voltex_audio: OGG/Vorbis decoder (자체 구현)
- voltex_audio: AudioClip, mixing, WASAPI backend, AudioSystem
- examples/audio_demo

### Phase 6-2: 3D Audio
- voltex_audio: distance_attenuation, stereo_pan, play_3d, set_listener
- voltex_audio: Doppler effect (doppler_shift)

### Phase 6-3: Mixer
- voltex_audio: MixGroup (Master/Bgm/Sfx/Voice), fade, effective_volume

### Phase 7-1~7-4: Advanced Rendering
- Deferred rendering (G-Buffer 4 MRT + Lighting pass)
- SSGI (SSAO + color bleeding)
- RT Shadows (BLAS/TLAS, compute shader ray query)
- Post Processing (HDR + Bloom + ACES tonemap)

### Phase 8-1: AI System
- voltex_ai: NavMesh (manual + auto builder via voxelization)
- voltex_ai: A* pathfinding + Funnel algorithm (String Pulling)
- voltex_ai: Steering behaviors + Dynamic obstacle avoidance

### Phase 8-2: Networking
- voltex_net: Packet protocol, UDP socket, NetServer/NetClient
- voltex_net: ReliableChannel (sequence, ACK, retransmission, RTT)
- voltex_net: OrderedChannel (in-order delivery)
- voltex_net: Snapshot serialization + delta compression
- voltex_net: InterpolationBuffer (client-side linear interpolation)

### Phase 8-3: Lua Scripting
- voltex_script: Lua 5.4 내장 빌드, FFI 바인딩
- voltex_script: LuaState (exec, register_fn, push_table/read_table, push_vec3/read_vec3)
- voltex_script: LuaCoroutine (create/resume), Sandbox (os/io 차단)
- voltex_script: Hot reload (reload_file), Engine API (spawn, position, velocity, rotation, scale, destroy, find_by_tag)

### Phase 8-4: Immediate Mode UI (Editor)
- voltex_editor: FontAtlas (8x12 bitmap ASCII)
- voltex_editor: DrawList, UiContext, UiRenderer
- voltex_editor: Widgets (text, button, slider, checkbox, panel, text_input, scroll_panel)
- voltex_editor: Drag and Drop (begin_drag, drop_target)
- voltex_editor: DockTree (binary split layout, resize, tab switching)
- voltex_editor: OrbitCamera, ViewportTexture, ViewportRenderer (3D scene viewport)
- voltex_editor: hierarchy_panel, inspector_panel (Transform/Tag/Parent editing)
- voltex_editor: AssetBrowser (file system browsing, directory navigation)
- voltex_editor: TtfParser, Rasterizer, GlyphCache, TtfFont (자체 TTF 렌더링, Unicode)
- examples/editor_demo

## Crate 구조

```
crates/
├── voltex_math      — Vec2, Vec3, Vec4, Mat4(+inverse), AABB, Ray
├── voltex_platform  — VoltexWindow, InputState, GameTimer
├── voltex_renderer  — GPU, Mesh, OBJ, glTF, JPG, PNG, Camera, PBR, Shadow(CSM/Point/Spot), IBL(SH), Frustum
├── voltex_ecs       — Entity, SparseSet, World, Transform, Hierarchy, Scene(JSON/Binary), Scheduler
├── voltex_asset     — Handle<T>, AssetStorage<T>, Assets, AssetLoader, FileWatcher
├── voltex_physics   — Collider, GJK/EPA, BVH(refit), RigidBody(angular+sleep), CCD, raycast_all
├── voltex_audio     — WAV(16/24/32), OGG/Vorbis, mixing, WASAPI, Doppler, MixGroup, spatial
├── voltex_ai        — NavMesh(+builder), A*, Funnel, steering, obstacle avoidance
├── voltex_net       — UDP, NetServer/Client, Reliable/Ordered, Snapshot(delta), Interpolation
├── voltex_script    — Lua 5.4, LuaState, table interop, coroutine, sandbox, hot reload
└── voltex_editor    — IMGUI, docking, 3D viewport, inspector, asset browser, TTF font, UiRenderer
```

## 테스트: 617개 전부 통과

- voltex_math: 37
- voltex_platform: 3
- voltex_ecs: 91
- voltex_asset: 22
- voltex_renderer: 137
- voltex_physics: 103
- voltex_audio: 46
- voltex_ai: 34
- voltex_net: 36
- voltex_script: 18
- voltex_editor: 85

## Examples (12개 + survivor_game)

- triangle, model_viewer, many_cubes, hierarchy_demo, asset_demo
- pbr_demo, multi_light_demo, shadow_demo, ibl_demo
- audio_demo, deferred_demo, editor_demo
- survivor_game (완성 게임)

## 간소화/미뤄진 항목

상세: `docs/DEFERRED.md`
