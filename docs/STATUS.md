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
- voltex_renderer: PNG decoder (deflate + filter reconstruction)
- voltex_renderer: JPG decoder (Baseline JPEG: Huffman, IDCT, MCU, YCbCr, subsampling)
- voltex_renderer: glTF 2.0 / GLB parser (JSON parser, accessor extraction, PBR material)
- examples/model_viewer

### Phase 3a: ECS
- voltex_ecs: Entity(id+generation), SparseSet<T>, World(type-erased storage)
- voltex_ecs: query<T>, query2<A,B>, query3, query4, Transform component
- voltex_ecs: query_with/query_without, query2_with/query2_without (With/Without filters)
- voltex_ecs: System trait, Scheduler (ordered execution)
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

### Phase 6-1: Audio System Foundation
- voltex_audio: WAV parser (PCM 16-bit, mono/stereo)
- voltex_audio: AudioClip (f32 samples), mixing (volume, looping, channel conversion)
- voltex_audio: WASAPI backend (Windows, shared mode, COM FFI)
- voltex_audio: AudioSystem (channel-based audio thread, play/stop/volume)
- examples/audio_demo (sine wave playback)

### Phase 6-2: 3D Audio
- voltex_audio: Listener, SpatialParams
- voltex_audio: distance_attenuation (inverse distance), stereo_pan (equal-power)
- voltex_audio: mix_sounds spatial integration (per-sound attenuation + panning)
- voltex_audio: play_3d, set_listener API

### Phase 6-3: Mixer
- voltex_audio: MixGroup (Master, Bgm, Sfx, Voice), MixerState
- voltex_audio: GroupState with linear fade (tick-based)
- voltex_audio: effective_volume (group * master)
- voltex_audio: set_group_volume, fade_group API

### Phase 7-1: Deferred Rendering
- voltex_renderer: GBuffer (4 MRT: Position/Normal/Albedo/Material + Depth)
- voltex_renderer: G-Buffer pass shader (MRT output, TBN normal mapping)
- voltex_renderer: Lighting pass shader (fullscreen triangle, Cook-Torrance BRDF, multi-light, shadow, IBL)
- voltex_renderer: Deferred pipeline (gbuffer + lighting bind group layouts)
- examples/deferred_demo (5x5 sphere grid + 8 orbiting point lights)

### Phase 7-2: SSGI (Screen-Space Global Illumination)
- voltex_renderer: SsgiResources (hemisphere kernel 64 samples, 4x4 noise, output texture)
- voltex_renderer: SSGI shader (SSAO + color bleeding in one fullscreen pass)
- voltex_renderer: SSGI pipeline + bind group layouts
- voltex_renderer: Lighting pass SSGI integration (ambient * ssgi_ao + indirect)
- deferred_demo updated with 3-pass rendering (GBuffer → SSGI → Lighting)

### Phase 7-3: RT Shadows (Hardware Ray Tracing)
- voltex_renderer: RtAccel (BLAS/TLAS acceleration structure management)
- voltex_renderer: RT Shadow compute shader (ray query, directional light)
- voltex_renderer: RT shadow pipeline + bind group layouts
- voltex_renderer: Lighting pass RT shadow integration
- deferred_demo updated with hardware RT shadows (4-pass: GBuffer → SSGI → RT Shadow → Lighting)

### Phase 7-4: Post Processing (HDR + Bloom + ACES)
- voltex_renderer: HdrTarget (Rgba16Float), Lighting → HDR output
- voltex_renderer: BloomResources (5-level mip chain, downsample/upsample)
- voltex_renderer: Bloom shader (bright extract + tent filter upsample)
- voltex_renderer: Tonemap shader (ACES filmic + bloom merge + gamma)
- deferred_demo updated with full post-processing (7 passes)

### Phase 8-1: AI System
- voltex_ai: NavMesh (manual triangle mesh, find_triangle, edge/center queries)
- voltex_ai: A* pathfinding on triangle graph (center-point path)
- voltex_ai: Steering behaviors (seek, flee, arrive, wander, follow_path)

### Phase 8-4: Immediate Mode UI (Editor Foundation)
- voltex_editor: FontAtlas (8x12 비트맵 ASCII, 코드 생성)
- voltex_editor: DrawList (정점/인덱스/커맨드), DrawVertex (Unorm8x4 color)
- voltex_editor: UiContext (IMGUI 상태, hot/active, 커서 레이아웃)
- voltex_editor: Widgets (text, button, slider, checkbox, panel)
- voltex_editor: UiRenderer (wgpu 2D pipeline, alpha blending, orthographic)
- examples/editor_demo (IMGUI 위젯 데모)

### Phase 8-3: Lua Scripting
- voltex_script: Lua 5.4 내장 빌드 (cc crate)
- voltex_script: Lua C API FFI 바인딩
- voltex_script: LuaState 안전 래퍼 (exec, exec_file, register_fn, globals)
- voltex_script: 기본 바인딩 (voltex_print)

### Phase 8-2: Networking Foundation
- voltex_net: Packet protocol (Connect, Accept, Disconnect, Ping, Pong, UserData)
- voltex_net: Binary serialization/deserialization
- voltex_net: Non-blocking UDP socket wrapper
- voltex_net: NetServer (client management, broadcast)
- voltex_net: NetClient (server connection, handshake)

## Crate 구조

```
crates/
├── voltex_math      — Vec2, Vec3, Vec4, Mat4, AABB, Ray
├── voltex_platform  — VoltexWindow, InputState, GameTimer
├── voltex_renderer  — GPU, Mesh, OBJ, Camera, Material, PBR, Shadow, IBL, Sphere
├── voltex_ecs       — Entity, SparseSet, World, Transform, Hierarchy, Scene, WorldTransform
├── voltex_asset     — Handle<T>, AssetStorage<T>, Assets
├── voltex_physics   — Collider, ContactPoint, BvhTree, RigidBody, detect_collisions, physics_step, raycast
├── voltex_audio     — AudioClip, WAV parser, mixing, WASAPI backend, AudioSystem, MixGroup, spatial
├── voltex_ai        — NavMesh, A* pathfinding, steering behaviors
├── voltex_net       — UDP packets, NetServer, NetClient
├── voltex_script    — Lua 5.4 FFI, LuaState, scripting bindings
└── voltex_editor    — IMGUI framework, UiRenderer, widgets
```

## 테스트: 324개 전부 통과

- voltex_asset: 14
- voltex_audio: 35 (audio_clip 2 + wav 5 + mixing 11 + audio_system 2 + spatial 8 + mix_group 7)
- voltex_ecs: 50 (world 16 + sparse_set 8 + hierarchy 5 + scene 4 + world_transform 4 + entity 4 + transform 2 + scheduler 4 + query filter 3)
- voltex_math: 37 (29 + AABB 6 + Ray 2)
- voltex_physics: 64 (collider 2 + narrow 11 + bvh 5 + collision 7 + rigid_body 3 + integrator 3 + solver 5 + ray 10 + raycast 6 + gjk 12)
- voltex_platform: 3
- voltex_ai: 15 (navmesh 4 + pathfinding 5 + steering 6)
- voltex_net: 8 (packet 7 + integration 1)
- voltex_script: 11 (state 8 + bindings 1 + engine_api 2)
- voltex_editor: 10 (font 2 + draw_list 3 + widgets 3 + layout 1 + renderer 1)
- voltex_renderer: 77 (20 + SSGI 3 + RT 3 + bloom 3 + tonemap 4 + png 10 + jpg 11 + json_parser 10 + gltf 8 + deflate 5)

## Examples (12개)

- triangle — Phase 1 삼각형
- model_viewer — OBJ 큐브 + Blinn-Phong
- many_cubes — 400 ECS 엔티티 렌더링
- hierarchy_demo — 태양계 씬 그래프
- asset_demo — Handle 기반 에셋 관리
- pbr_demo — metallic/roughness 구체 그리드
- multi_light_demo — 다중 색상 라이트
- shadow_demo — Directional Light 그림자
- ibl_demo — Normal map + IBL
- audio_demo — 사인파 오디오 재생
- deferred_demo — 디퍼드 렌더링 + 다중 포인트 라이트
- editor_demo — IMGUI 위젯 데모

## 전체 완료!

스펙 참조: `docs/superpowers/specs/2026-03-24-voltex-engine-design.md`

## 간소화/미뤄진 항목

상세: `docs/DEFERRED.md`
