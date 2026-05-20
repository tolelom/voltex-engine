# Voltex Engine - 작업 현황

## AI 친화 재구조화 (2026-05-20 완료)

`docs/superpowers/specs/2026-05-20-engine-restructure-design.md` 에 명시된 7개 마일스톤 전부 완료.

| # | 작업 | 결과 |
|---|---|---|
| M1 | 11개 crate × CODEMAP.md + CLAUDE.md 보강 | 12 CODEMAP (sub 11 + facade) |
| M2 | 800줄+ 파일 4개 분리 (vorbis/gltf/jpg/deferred_pipeline) | 0건 잔존 |
| M3 | 500-800줄 파일 11개 분리 | 0건 잔존 (전체 .rs 600줄 이하) |
| M4 | 에러 표준화 (thiserror 기반 `<Crate>Error` + `Result<T>` alias) | 8 crate 적용, `Result<T, String>` 0건 |
| M5 | ECS 패턴 표준화 (Component trait + `register::<T>()`) | Transform/Tag 마이그레이션, +3 테스트 |
| M6 | voltex facade crate + prelude | L4 facade, triangle 마이그레이션 |
| M7 | 최종 검증 + hierarchy_demo 추가 마이그레이션 | 모든 성공기준 충족 |

성공기준 7개 전부 충족: CODEMAP / 600줄 / thiserror / Component+register / facade / 760+ tests / examples 빌드.

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

### Phase 8-5: Editor Command Foundation
- voltex_editor: EditorCommand (Spawn/Despawn/SetTransform/SetTag/SaveScene/LoadScene), CommandError
- voltex_editor: EditorSession (World + ComponentRegistry + 자동 저장 + jsonl 로그)
- voltex_editor: panels::hierarchy, panels::inspector (text_input 기반 — 8-4의 트리/슬라이더 inspector와 별개 경로로 공존)
- examples/editor_demo: Toolbar(Spawn/Save/Load)를 EditorSession에 dispatch (8-4의 viewport/dock/asset_browser 위에 추가)

### Phase 8-6: Editor 통합 + Undo/Redo + Hierarchy Commands + Lua FFI Fix (2026-05-20)
- voltex_editor: **inspector 통합** — `inspector::*` 제거, `panels::hierarchy::*` 트리 뷰 + `panels::inspector::*` 슬라이더, 모든 mutation이 EditorCommand dispatch 경유 (jsonl 로그 + 자동 저장)
- voltex_editor: **EditorSession Undo/Redo** — Inverse 스택, undo()/redo()/undo_depth()/redo_depth(). v1 undoable: Spawn/SetTransform/SetTag/AddChild/RemoveChild. v1 non-undoable: Despawn/SaveScene/LoadScene (history 클리어)
- voltex_editor: **AddChild/RemoveChild commands** — hierarchy 조작도 동일 dispatch path. 비-attached child의 RemoveChild는 no-op + 미기록
- voltex_script: **Lua FFI 핸들 규약** — Entity = `(id, gen)` 두 정수. spawn_entity → 2 returns, set/get_position → id+gen+payload. 핸들 stale 시 `World::is_alive` 거부 (ABA 버그 fix)
- voltex_script: **engine_api 확장** — 4개 → 12개 함수. 신규: destroy_entity / set/get_rotation / set/get_scale / set/get_tag / find_by_tag. 모두 `(id, gen)` 규약 준수
- voltex_ecs: **World::clear()** — 모든 entity/component/allocator 리셋. LoadScene 같은 source-of-truth replace 시나리오용
- examples/editor_demo: Toolbar에 Undo/Redo 버튼 추가, `session.selected` 가 선택의 단일 소스

### Phase 8-7: Viewport Picking + Seed-as-Commands + Lua Hierarchy + Mat4 Fix (2026-05-20)
- voltex_editor: **viewport raycast picking** — `picking::pick_entity` + 순수 helpers (ndc_from_viewport / ray_from_ndc / ray_vs_sphere_t). 좌클릭 release 시 드래그가 작으면 (≤4px) ray cast → 가장 가까운 sphere hit entity로 `session.selected` 갱신
- voltex_math: **Mat4::inverse 버그 fix** — 이전 구현은 6개 s/c 2x2 minor만으로 cofactor를 표현 → 행 0+1 또는 2+3에 속하지 않는 3x3 minor 엔트리가 잘못된 값. 표준 3x3 minor cofactor 전개로 교체. `VP * VP^-1 ≈ I` 회귀 테스트 추가 (viewport unprojection이 100배 가까운 점을 반환하던 silent bug)
- voltex_script: **engine_api 12 → 15** — `add_child` / `remove_child` (둘 다 `(id,gen)` 4개 인자), `find_all_by_tag(tag) → Lua table`. stale handle은 모두 silent no-op
- examples/editor_demo: 시드 엔티티 (ground + 3 cube + Cube1↔Cube2 hierarchy)를 `EditorSession::dispatch` 경로로 옮김 → jsonl 로그 + Save/Load round-trip 일관성
- v1 한계 (문서화): picking은 sphere 근사 (메시 AABB는 미구현). gizmo translate/rotate/scale 핸들은 미구현

### Cycle 3 (E1): Gizmo + AABB Picking (2026-05-20)
- voltex_editor::picking: **`ray_vs_aabb` + `world_aabb_from_local`** 헬퍼 추가 (voltex_physics 의 동등 구현을 cross-crate 의존성 회피 위해 복제). `pick_entity` 가 `Bounds` 있는 엔티티는 world-space AABB (8-corner 변환, loose), 없으면 sphere fallback. 시그니처 `&World` → `&mut World` (내부에서 `propagate_transforms` 호출).
- voltex_editor::gizmo: **신규 모듈** (mod + translate + rotate + scale + renderer + shader.wgsl). `GizmoMode` (Translate/Rotate/Scale), `GizmoAxis`, `GizmoState` (drag 상태머신, set_mode 가 드래깅 중에는 no-op). per-mode 모듈:
  - `translate.rs`: axis thin-AABB hit + drag-plane projection (가장 카메라 향한 면 선택) → axis 투영 delta
  - `rotate.rs`: ring plane (= axis normal) ray-vs-plane hit + 도넛 반경 검사 → atan2 angle delta (wrap (-π, π])
  - `scale.rs`: translate hit + handle cube AABB, drag projection ratio → 축별 scale multiplier (anchor ≈ 0 보호)
  - `renderer.rs`: LINE-list wgpu pipeline (depth-test Always, alpha blending), procedural translate arrows / rotate rings (32-seg) / scale handle cubes. active axis 노란색 강조.
- examples/editor_demo: Toolbar 에 Move/Rotate/Scale 버튼. 시드 엔티티 (ground + 3 cube) 에 `Bounds` 부착. 좌클릭 down → 8px 이내 + 모드 axis hit 이면 drag 시작 → 매 프레임 World 직접 mutate, release 시 `end_drag` → final 로 `SetTransform` dispatch (단일 undo 엔트리, initial 이 inverse). orbit camera 는 gizmo dragging 중 비활성.
- v1 한계: snap-to-grid, multi-select, OBB picking 미구현 (모두 spec out-of-scope). gizmo 는 world-space 고정 크기 (1.5 단위).
- 시각 검증: GUI 자동화 환경 부재 → 수동 검증 권장. 단위 테스트 +19 (picking +8, gizmo +11).

### Cycle 2 (C0b): Renderer 브릿지 + ECS-driven 렌더링 (2026-05-20)
- **voltex_renderer_ecs**: 신규 L3 crate. `RenderAssets` (id/generation lockstep wrapper around voltex_asset::Assets — C0a 의 phantom-typed `Handle<MeshAsset>` 과 내부 `Handle<Mesh>` 핸들이 lockstep), `BasicRenderer` (mesh_shader.wgsl pipeline + per-entity dynamic UBO, 한 메서드 render), `RenderItem` + `collect_render_items` (자체 PBR pipeline 호출자용 helper)
- voltex (facade): `voltex::renderer_ecs` 모듈 re-export 추가 (prelude 변경 없음 — 도메인 특화 정책)
- C0a 컴포넌트 (MeshHandle / MaterialHandle / Bounds) 가 처음으로 실제 렌더링 경로에서 사용됨
- examples/editor_demo: 평행 `scene_meshes: Vec<Mesh>` (4개, 위치 baked-in) 제거. 단일 unit cube + unit ground mesh 공유. 시드 엔티티에 MeshHandle/MaterialHandle 직접 부착 (E3 의 AddComponent command 도입 전 임시 처치). Toolbar Spawn 버튼이 visible cube 생성. 뷰포트 렌더 패스가 `BasicRenderer::render()` 한 줄로 축소.
- examples/survivor_game: 평행 `Vec<RenderEntity>` / `Vec<Mat4>` / `Vec<material>` 배열 + per-frame 모델/머티리얼 재구축 루프 제거. `RenderHandles` 가 per-type MeshHandle/MaterialHandle 보관, `spawn_player/enemy/projectile` 에 전달. arena 5개 + player + dynamic projectile/enemy 모두 ECS entity. shadow + color pass 둘 다 `collect_render_items` 결과 iterate.
- 부수 정리: player Transform Y=0.5 (render-time +0.5 offset 제거), enemy/projectile scale (0.4/0.3) 을 Transform.scale 에 baked. player 무적 깜빡임 효과는 v1 에서 생략 (material swap 패턴 future work).
- 시각 검증: GUI 자동화 환경 부재로 빌드/타입 검증 + Inspector 슬라이더 ↔ 뷰포트 큐브 이동 흐름은 수동 검증 권장.

## Crate 구조

```
crates/
├── voltex             — Facade (M6): 모든 sub-crate re-export + prelude
├── voltex_math        — Vec2, Vec3, Vec4, Mat4(+inverse), AABB, Ray
├── voltex_platform    — VoltexWindow, InputState, GameTimer
├── voltex_renderer    — GPU, Mesh, OBJ, glTF, JPG, PNG, Camera, PBR, Shadow(CSM/Point/Spot), IBL(SH), Frustum
├── voltex_renderer_ecs — RenderAssets, BasicRenderer, RenderItem + collect_render_items (ECS-driven 렌더링 브릿지)
├── voltex_ecs         — Entity, SparseSet, World, Transform, Hierarchy, Scene(JSON/Binary), Scheduler, MeshHandle/MaterialHandle/Bounds (C0a)
├── voltex_asset       — Handle<T>, AssetStorage<T>, Assets, AssetLoader, FileWatcher
├── voltex_physics     — Collider, GJK/EPA, BVH(refit), RigidBody(angular+sleep), CCD, raycast_all
├── voltex_audio       — WAV(16/24/32), OGG/Vorbis, mixing, WASAPI, Doppler, MixGroup, spatial
├── voltex_ai          — NavMesh(+builder), A*, Funnel, steering, obstacle avoidance
├── voltex_net         — UDP, NetServer/Client, Reliable/Ordered, Snapshot(delta), Interpolation
├── voltex_script      — Lua 5.4, LuaState, table interop, coroutine, sandbox, hot reload
└── voltex_editor      — IMGUI, docking, 3D viewport, inspector, asset browser, TTF font, UiRenderer
```

## 테스트: 838개 전부 통과 (Cycle 3 +19 vs Cycle 2 직후 819)

- voltex: 2 (M6: prelude/module 경로 컴파일 검증)
- voltex_math: 39 (8-7 +2: inverse roundtrip × VP+translation)
- voltex_platform: 3
- voltex_ecs: 116 (C0a +7: MeshHandle/MaterialHandle/Bounds Component roundtrip + register_defaults)
- voltex_asset: 24 (C0a +2: Handle::from_raw_parts / id+generation 접근자)
- voltex_renderer: 200
- voltex_renderer_ecs: 7 (C0b 신규: RenderAssets material roundtrip ×2, missing handle ×2, items empty/skip-no-mesh/stale/hierarchy)
- voltex_physics: 106
- voltex_audio: 84
- voltex_ai: 34
- voltex_net: 47
- voltex_script: 30 (8-7 +5: add_child/remove_child/stale-parent/find_all_by_tag×2)
- voltex_editor: 146 (E1 +19: picking::* aabb/world_aabb/bounds-aware ×8, gizmo::* mode/translate/rotate/scale/state ×11)

## Examples (12개 + survivor_game)

- triangle, model_viewer, many_cubes, hierarchy_demo, asset_demo
- pbr_demo, multi_light_demo, shadow_demo, ibl_demo
- audio_demo, deferred_demo, editor_demo
- survivor_game (ECS + voltex_physics + voltex facade로 재작성 완료, 2026-05-20)

## 다음 작업 후보 (2026-05-20 8-7 이후)

1. ~~**inspector 통합**~~ — ✅ Phase 8-6. 단일 `panels::*` 경로.
2. ~~**scene 영속화 검증**~~ — ✅ Phase 8-7. 시드 엔티티가 `EditorCommand` dispatch 경로 통과 → 로그/Save/Load 일관성.
3. **EditorCommand 확장 추가** — Undo/Redo ✅, AddChild/RemoveChild ✅. 남은 후보: AddComponent/RemoveComponent (Transform 외 컴포넌트 — ComponentRegistry 의 raw byte API 활용).
4. **Despawn 정확한 undo** — 현재 v1은 history 클리어. `EntityAllocator::restore_at_generation(id, gen)` 추가 + 컴포넌트 스냅샷 백업 형태로 풀 undo 지원. World API 변경이라 별 commit 필요.
5. ~~**viewport raycast 픽킹**~~ — ✅ Phase 8-7. 좌클릭 → ray cast → `session.selected` (sphere 근사). **gizmo (translate/rotate/scale 핸들)** 은 미구현 — 다음 chunk 후보.
6. ~~**Lua API 확장**~~ — ✅ Phase 8-7 (15개 함수). 다음: get_world_transform, query 결과를 Lua iterator 로 노출 등.
7. ~~**survivor_game을 ECS/Physics로 재작성**~~ — ✅ 완료 (2026-05-20).
8. **메시 AABB 기반 picking** — 현재 v1은 sphere 근사. ECS Transform 외에 mesh bounds 컴포넌트가 들어오면 OBB/AABB picking 으로 업그레이드.

## 다른 기기에서 이어서 작업하기

```bash
git clone https://git.tolelom.xyz/tolelom/game_engine
cd game_engine
cargo test --workspace --lib   # 검증: 760개 통과해야 함
```

진행 상황과 다음 단계는 이 파일과 `docs/DEFERRED.md`, `docs/superpowers/plans/`, `docs/superpowers/specs/` 를 본다. 자동 루프가 master에 계속 작업을 푸시할 수 있으므로 작업 전 `git pull --ff-only` 필수.

## 간소화/미뤄진 항목

상세: `docs/DEFERRED.md`
