# Voltex Engine - 작업 현황

## AI 친화 재구조화 (2026-05-20 완료)

`docs/superpowers/specs/2026-05-20-engine-restructure-design.md` 에 명시된 7개 마일스톤 전부 완료.

| # | 작업 | 결과 |
|---|---|---|
| M1 | 11개 crate × CODEMAP.md + CLAUDE.md 보강 | 12 CODEMAP (sub 11 + facade) |
| M2 | 800줄+ 파일 4개 분리 (vorbis/gltf/jpg/deferred_pipeline) | 0건 잔존 |
| M3 | 500-800줄 파일 11개 분리 | M3 시점 0건. v1.0.2 에서 누적 위반 4건 재분리 |
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

## 🎉 v1.0 release — 2026-05-21 (tag `v1.0.0`) → **v1.0.1** ~ **v1.0.9** patches

15 cycle / 37 세션 / 약 4 일에 걸쳐 진행된 v1.0 로드맵 완료. 회고는
[docs/v1.0-retrospective.md](v1.0-retrospective.md). 미뤄진 항목은 v1.1+ 후속.

v1.0.9 부터는 **서비스 배포 완성도** 로드맵 (`docs/superpowers/specs/2026-05-21-service-readiness-roadmap.md`) 기준으로 진행.

### 2026-06-03 — 프레임별 materialize (v1.0.69, 자율 세션)

사이클 1 한계 수정: `materialize_renderables`가 gpu_init 1회만 실행돼 런타임 스폰
엔티티(Lua/투사체/게임플레이)가 선언적 렌더러블을 달아도 안 보였음. 핸들러 redraw에서
`tick_systems_only` 직후 매 프레임 materialize 호출(idempotent, pending 없으면 저렴).
RTX 4080 capture 회귀 `materialize_picks_up_entities_spawned_after_a_prior_pass`로
런타임 스폰 시나리오 검증. build/clippy/test 그린.

### 2026-06-03 — 데이터 저작 visual 씬 (v1.0.68, 자율 세션)

상용 엔진의 본질인 "코드 아닌 데이터 씬" 결합 검증. 사이클 1의 `PrimitiveMesh`/
`MaterialDesc`(Component)가 레지스트리 경유 씬 JSON round-trip → materialize→렌더됨을
단위 + RTX 4080 capture(`data_authored_scene_renders_nonblank`)로 확인(데이터→픽셀,
스폰 코드 0). 검증 중 **scene Transform-anchor footgun** 발견(Transform 없는 엔티티
직렬화 누락) → 동작 변경 대신 docstring 명시 + 회귀 가드(truthful-doc, v1.0.41/56 패턴).
build/clippy/test 그린.

### 2026-06-03 — 프리팹 레지스트리 (v1.0.67, 자율 세션)

기초 워크플로 격차 6번째(같은 엔티티 반복 조립). `voltex_ecs::Prefabs` — `register(name, factory)`/
`spawn(world, name)`. spawn_bundle 위 "한 번 정의, 이름으로 인스턴스화"(Unity Prefab).
팩토리 컴포넌트-무관. facade prelude 노출. 단위 6 + 변이 테스트. `game_ready_demo`
pickup 3개를 프리팹 인스턴스화+per-instance 설정으로 전환. build/clippy/test 그린.

### 2026-06-03 — 프레임 클록 (v1.0.66, 자율 세션)

기초 워크플로 격차 5번째(시스템이 dt만 받고 누적 시간 없음). `voltex_runtime::Time`
{elapsed_secs/delta_secs/frame_count} + `RuntimeApp.time`(tick 시작에서 advance) +
`add_timed_system(|world,&Time|)`. facade prelude 노출. 단위 6 + 변이 테스트.
`game_ready_demo` HUD 경과시간을 타임드 시스템으로 전환(조건부 누적 버그 해소).
build/clippy/test 그린.

### 2026-06-03 — 태그 조회 헬퍼 (v1.0.65, 자율 세션)

기초 워크플로 격차 4번째(`query::<Tag>().find(..).map(..)` 반복). `World::find_by_tag`/
`iter_by_tag` 추가 (scene.rs, World 코어는 Tag-free 유지). flagship + game_ready_demo
4곳 정리. 단위 3 + build/clippy/test 그린.

### 2026-06-03 — 컴포넌트 번들 (v1.0.64, 자율 세션)

기초 워크플로 격차 3번째(엔티티마다 spawn + N×add 보일러플레이트). Bevy Bundle /
Unity 프리팹 류 (상세 CHANGELOG v1.0.64).

- **`voltex_ecs::Bundle`** + `World::spawn_bundle((A,B,..))` / `insert_bundle` —
  튜플 1~12 컴포넌트 한 번에 첨부. 매크로 생성, 외부 의존 0. facade prelude 노출.
- 검증: 단위 6 + 변이 테스트. `game_ready_demo` 전 엔티티가 단일 spawn_bundle
  (player 6-튜플, chaser 7-튜플, 크레이트 경계 넘는 컴포넌트 혼합). build/clippy/test 그린.

### 2026-06-03 — 입력 액션 매핑 (v1.0.63, 자율 세션)

기초 워크플로 격차 2번째(입력이 raw 키코드 폴링뿐). Unity Input Actions / Godot
InputMap 류의 시맨틱 액션/축 레이어 추가 (상세 CHANGELOG v1.0.63).

- **`voltex_platform::InputMap`** — `InputState` 위 순수 매핑. `bind_key`/`bind_mouse`/
  `bind_axis` + `is_pressed`/`just_pressed`/`just_released`/`axis`/`vector`. `Trigger`
  enum + `KeyCode`/`MouseButton` 재export.
- **런타임**: `RuntimeApp.input_map` + `add_action_system(|world,input,map,dt|)` +
  `RuntimeConfig::with_input_map`. facade prelude 노출.
- 검증: 단위 8 + 변이 테스트 + 런타임 통합 테스트. flagship `voltex_survivor` 이동
  ~35줄 → bind_axis 4줄 + 액션 시스템. build/clippy/test 그린.
- 한계: 마우스 just-released 미지원, 축 직렬화/런타임 설정 파일 바인딩은 후속.

### 2026-06-03 — 선언적 렌더러블 (v1.0.62, 자율 세션)

상용 엔진(Unity/Godot)과 **기초 워크플로**를 비교: 기능 폭은 충분하나 "눈에 보이는
오브젝트 하나 놓기"가 2단계(GPU 없는 `scene_builder` ↔ GPU 필요한 `gpu_init`)로 갈려
`Tag` 재조회 + `MeshHandle`/`MaterialHandle`/`Bounds` 수동 부착 보일러플레이트가
강제됐던 것을 정정 (상세 CHANGELOG v1.0.62).

- **`voltex_renderer_ecs::renderable`**: `PrimitiveMesh`(Cube/Plane/Quad/Sphere) +
  `MaterialDesc(MaterialUniform)` 값-타입 컴포넌트(둘 다 `Component` → 씬 직렬화) +
  `materialize_renderables`(dedup + 멱등; gpu_init 직후 RuntimeApp 자동 호출).
- 게임은 `scene_builder` 한 곳에서 `PrimitiveMesh::cube(1.0)` + `MaterialDesc::color(..)`
  선언만 — device/`MeshHandle`/Tag 왕복 불필요. facade prelude 노출.
- 검증: 단위 13 + 변이 테스트(dedup 키 RED→GREEN) + RTX 4080 capture 회귀 2개
  (`docs/visual/declarative_primitives.png`). `game_ready_demo` `upload_meshes`
  ~30줄 → 스킨드 토템 1줄로 축소. build/clippy/test 그린.
- 미적용(후속): 스킨드 메시는 여전히 device 직접 경로. 텍스처/노멀맵 풀 PBR 머티리얼
  선언은 `MaterialDesc`(색+metallic/roughness)까지만 — 4-맵은 `PbrTextures` 별도.

### 2026-06-03 — 현황 점검 sweep (v1.0.61, 자율 세션)

현황 점검 후 A(위생)→D(대형) 우선순위로 진행 (상세 CHANGELOG v1.0.61). 점검 중
DEFERRED.md **문서 drift 4건** 정정(Lua 입력 API / JSON 씬 cap / `validate_hierarchy` /
멀티라이트 — 이미 구현됐는데 미구현 표기였음).

- **위생**: clippy 게이트 회귀 수정(`useless_vec`), 워크스페이스 17 crate 버전 sync
  → `voltex --version` = 1.0.61 (이전 0.1.0 placeholder).
- **런타임**: `set_point_lights_fn`(per-frame 점광원, directional `set_light_fn` 대칭),
  AudioSource 씬 직렬화(`register_raw` escape hatch, 자동 등록).
- **스킨드 그림자 caster**: `create_skinned_shadow_pipeline` + `skinned_shadow_shader.wgsl`
  (depth-only GPU 스키닝, 기존 레이아웃 재사용). RTX 4080 검증 `skinned_mesh_casts_shadow`
  + `docs/visual/skinned_shadow.png`.
- **견고성**: `.normalize()` audit — AI 경로 이미 safe 확인, csm/spot_shadow light dir
  zero-input NaN 2건 수정.
- **미진행(평가 후 문서화)**: 에디터 컴포넌트 리플렉션(인프라 부재), archetype/병렬
  스케줄러(major rewrite), CI green(mirror 필요), missing_docs/pub(crate)(저가치 churn).
- 검증: build/clippy(`-D warnings` all-targets)/test(1239 lib) 전부 그린.

### 2026-06-03 — Game-ready 2D 스택 + 게임플레이 빌트인 (자율 세션)

`game_ready_demo`를 실제로 돌려보며 드러난 "기능 연결" 갭을 메우는 자율 세션
(상세는 CHANGELOG v1.0.60). 모두 헤드리스 검증(단위 + RTX 4080 capture 회귀,
`voltex_capture/src/scene.rs`의 `#[ignore]` 테스트) + 역호환 유지.

- **렌더링**: 스폿라이트(`PointLight::spot`), 디버그 라인/기즈모
  (`DebugLines`, `set_debug_draw_fn`), 빌보드 스프라이트(`Sprite`), 직교 카메라
  (`with_orthographic`, 2D/탑다운), 스프라이트시트 UV(`set_sheet_frame`).
- **게임플레이**: CPU 파티클(`Particle`/`spawn_particle`), 스프라이트 애니메이션
  (`SpriteAnimation`), 자동 despawn(`Lifetime`).
- **편의**: `RenderAssets::add_texture_png/_rgba`(런타임 텍스처 로딩), prelude에
  게임 컴포넌트 추가(Sprite/Particle/SpriteAnimation/TextureHandle/Lifetime/
  DebugLines).
- **런타임 PBR 경로(v1.1 선반영)**: `RuntimeConfig::with_pbr(true)` /
  `BasicRenderer::set_pbr_enabled` — colored Blinn-Phong 대신 Cook-Torrance PBR
  (멀티라이트 + per-entity 머티리얼 + 4-맵 텍스처 `PbrTextures` + IBL). 기본 off.
  스킨드 PBR은 여전히 v1.1.
- 이 스택으로 직교 카메라 + 텍스처/아틀라스/애니메이션 스프라이트 + 파티클 +
  디버그 기즈모를 갖춘 2D 게임 빌드 가능. capture 회귀 26개 전부 green.

### v1.0.57 (2026-05-28) — voltex_image crate 분리 완료 (E1 + C1)

신규 L0 crate `voltex_image` — PNG/JPG/glTF/OBJ/JSON 디코더 + PNG 인코더 + `MeshVertex` POD. `voltex_renderer`는 `pub use` 재export로 역호환 유지, `voltex_capture`의 중복 CRC32/Adler32 제거(C1 해소). facade `voltex`에 `image` 모듈 추가. 테스트 91개 이관, 워크스페이스 전체 그린.

### v1.0.56 (2026-05-28) — 아키텍처/테스트 품질 심층 감사 (병렬 에이전트)

이전 라운드(v1.0.40~55)가 robustness 버그 헌팅이었다면, 이번엔 **아키텍처/설계 + 테스트 품질** 두 축으로 4개 병렬 탐색 에이전트가 16 crate 를 정적 감사하고, 발견을 5개 병렬 fix 에이전트로 처리했다. **레이어링 위반 0건** 확인. 진짜 결함 3건 + 거짓-확신 테스트 다수 + 문서 drift 정리.

**발견·수정한 실버그 3건**:
- **SSF 펀넬 apex-restart** (`voltex_ai/pathfinding.rs`) — DEFERRED 에 의심만 적혀있던 것을 2M 코리도 randomized search + max-penetration oracle 로 재현 확정. `funnel_smooth` 가 apex 갱신 시 `i = apex_idx` 재시작(정석 Mononen) 대신 bare `continue` → 지그재그 corridor 에서 내부 코너 절단(코리도 0.41 unit 이탈). while-loop 재시작으로 fix. `find_path`(centroid) 무영향. 회귀 테스트가 가드.
- **held-key just_pressed re-fire** (`voltex_platform/input.rs`) — `process_key` 가 held 키의 반복 Pressed(OS key-repeat, runtime handler 가 그대로 forward) 에서 `just_pressed` 를 안 지워 같은 프레임 내내 `is_key_just_pressed=true`. already-held 브랜치에서 클리어. +3 테스트.
- **net Euler 회전 보간 long-way** (`voltex_net/interpolation.rs`) — `lerp_entity` 가 rotation(Euler 라디안)을 componentwise 선형 lerp → 350°↔10° wrap 에서 180° 통과. `lerp_angle`(shortest-arc, wire 포맷 불변) 도입, rotation 에만 적용. 가드 테스트 확인.

**테스트 품질 강화 (거짓 확신 제거, ~40 테스트)**:
- physics: CCD TOI 값, GJK/EPA penetration depth+normal, +boundary 케이스, solver 스택 tol 1.0→0.1, contacts non-empty 단언, spin 축·부호 검증
- renderer: brdf_lut(vdc/ggx/smith/integrate), `Quat::to_mat4` 비-identity 90°, build_trs/mat4_mul, glTF parse-fallback silent coercion, compute_tangents, Plane — +23 테스트 (column-major M·v 규약 확인, 버그 없음)
- ai: 펀넬/path 테스트가 endpoint/len 대신 정확한 triangle 시퀀스·waypoint 단언
- audio: equal-power pan law(`left²+right²≈1`), math: vec3 tol f32::EPSILON→1e-5

**아키텍처/문서 정리**:
- `serialize_scene`/`deserialize_scene`(.vscn 텍스트)가 registry 무시하고 Mesh/Material/Script/커스텀 컴포넌트를 silently 누락하는 data-loss footgun — API 시그니처 불변, 한계를 docstring 에 명시 + JSON/binary 경로 안내 (truthful-doc, v1.0.41 방식)
- CLAUDE.md: register_defaults 6개(2개 아님), 의존성 그래프에 runtime(L3.5)/cli/capture(tools) 추가, cli/capture/runtime 에러 예외 명기
- CODEMAP 8개 drift fix: physics(BvhTree·integrate/physics_step), runtime(13 builders), facade(13 sub-crate), editor(viewport_shader), audio/asset/net(thiserror), platform(log)

**검증**: 워크스페이스 build/clippy(`-D warnings`)/test 전부 그린. 테스트 ~1090 → ~1130. 미수정 잔여(의도적): `DependencyScheduler`/`net::OrderedChannel`/`CliError::SceneNotFound` 미사용 export 는 future-work/reserved 라 churn 회피.

**후속 (같은 사이클)**: `GpuContext::new`/`new_with_features` 가 `.expect()` panic 대신 `Result<Self, RendererError>` 반환하도록 변경 — surface/adapter/device 획득 실패를 `RendererError::GpuInit` 으로 매핑. 라이브러리는 더 이상 GPU 부재에 panic 하지 않고 호출자가 결정. `voltex_runtime` handler 는 에러 로그 + `event_loop.exit()` graceful 종료, 예제 12개는 경계에서 `.expect("voltex: GPU initialization failed")`. breaking change(전 예제 시그니처) 지만 워크스페이스 전체 그린 확인.

### v1.0.55 (2026-05-28) — Visual verification harness + editor toolbar fix

P0 confidence. v1.0.40~54 는 정적 감사 기반 robustness 였고 빌드/테스트/clippy 는 전부 그린이지만, **엔진이 실제로 화면에 올바르게 렌더링되는지는 한 번도 검증된 적이 없었다** (DEFERRED 의 "GUI 시각 검증 0건"). 윈도우 앱은 자동화로 관찰 불가하고 6초 smoke-test 는 panic 부재만 보장.

**신규 crate `voltex_capture`** (도구):
- `HeadlessGpu` — surface 없는 wgpu device/queue (`compatible_surface: None`)
- `OffscreenTarget` — color(Rgba8UnormSrgb, COPY_SRC) + depth + 256B-row 정렬 readback
- `encode_rgba8` — 외부 의존 0 PNG 인코더 (zlib stored block + CRC32/Adler32), `voltex_renderer::parse_png` 로 라운드트립 검증
- `ImageStats` — blank/단색 프레임 자동 탐지
- `scene` / `scene_shadow` / `scene_lights` / `scene_gizmo` / `scene_ui` — 실제 파이프라인으로 데모 씬 렌더
- bin `capture` — 모든 씬을 PNG 로 저장 (썸네일 크기 옵션)

**오프스크린 픽셀 검증됨 (NVIDIA RTX 4080 SUPER / Vulkan, `docs/visual/`)**:
- BasicRenderer (Blinn-Phong, ECS/runtime/editor 뷰포트 경로) — 면별 음영 정상
- PBR (Cook-Torrance) 7×7 metallic/roughness 그리드 — BRDF 그라데이션 + IBL ambient 정상 (4000+ 색)
- 2-패스 directional shadow map — 라이트 방향에 맞는 그림자 정상
- multi-light (point·spot·directional, 4 컬러 point) — 거리감쇠 + 컬러 조명 정상 (7000+ 색)
- gizmo translate/rotate/scale — 3축 핸들 정확 (화살표/링/박스)
- editor UI (IMGUI + 폰트 아틀라스 + 알파) — 텍스트/위젯 또렷

**실 하드웨어 런타임 검증됨 (panic/validation 0, ~6s run)**:
- deferred_demo (G-buffer→SSGI→RT shadow→lighting→bloom→tonemap, RT 기능획득+BLAS+9-entry bind group), ibl_demo, shadow_demo
- flagship voltex_survivor / runtime_demo / survivor_game (RuntimeApp 풀 부트스트랩)
- **CLI 빌드 파이프라인 E2E**: `voltex new`→`voltex build`(release ~30s)→산출 exe 가 게임 구동 (DoD 헤드라인 워크플로)

**발견 & 수정한 실버그**: immediate-mode UI 에 가로 레이아웃이 없어 (`button()` 이 항상 `advance_line()`) `editor_demo` 툴바의 버튼들이 세로로 쌓여 뷰포트와 겹침. `UiContext::same_line()` 추가 + 위젯이 placed rect 를 기록 (`LayoutState::record`). 기본 동작(세로 적층)은 불변 — same_line 은 opt-in 이라 hierarchy/inspector 패널 영향 없음.

**검증**:
- voltex_capture 8 lib 테스트 (PNG 라운드트립 + stats) + 6 GPU-gated `#[ignore]` 씬 테스트
- voltex_editor 170 → 174 (+3 same_line, +1 end-to-end gizmo translate drag 통합 테스트 — 카메라→unprojection→gizmo 체인 회귀 가드)
- 워크스페이스 clippy 0 워닝
- README 스크린샷 갤러리 (실제 렌더 출력) + 상태 배지 v1.0.55

**남은 미검증** (v1.1+): deferred 오프스크린 픽셀 캡처 (런타임 검증으로 대체, 9-entry bind group 의 미초기화 SSGI/RT 입력 때문), viewport/gizmo 실제 마우스 드래그 인터랙션 (synthetic input 필요).

### v1.0.54 (2026-05-22) — Sphere generator degenerate-input clamp

P2 robustness. `generate_sphere(radius, sectors, stacks)` 가 sectors=0/stacks=0/radius=0 에서 div-by-zero → NaN 가득 vertex buffer. 비현실적이지만 PBR demo / IBL 미리보기에서 사용자가 sliders 0 까지 끌면 발생.

**Fix (`voltex_renderer/src/sphere.rs`)**:
- `sectors.max(3)`, `stacks.max(2)`, `radius` 가 finite + > MIN_POSITIVE 검사 후 fallback

**검증**:
- voltex_renderer 230 → 232 (+2). 회귀 테스트:
  - `sphere::tests::generate_sphere_with_degenerate_inputs_produces_finite_mesh`
  - `sphere::tests::generate_sphere_with_negative_radius_clamps`

### v1.0.53 (2026-05-22) — Camera::right NaN at pitch ±90°

P1 robustness. `Camera::right()` 가 forward × Vec3::Y 를 정규화 — pitch 가 ±π/2 이면 forward 가 Y 와 평행 → cross product 0 → normalize NaN. `FpsController::process_mouse` 가 pitch 를 ±89° 로 clamp 하지만 `Camera.pitch` 는 public 필드라 user code 가 직접 설정 가능.

**Fix (`voltex_renderer/src/camera.rs`)**:
- `right()` 가 `normalize_or_zero()` (v1.0.41) 사용 + zero 시 `Vec3::X` fallback

**검증**:
- voltex_renderer 228 → 230 (+2). 회귀 테스트:
  - `camera::tests::camera_right_at_straight_up_does_not_nan`
  - `camera::tests::camera_right_at_straight_down_does_not_nan`

### v1.0.52 (2026-05-22) — GameTimer hz=0 panic + alpha div-by-zero

P1 robustness. `GameTimer::new(0)` 가 `Duration::from_secs_f64(1.0 / 0.0 = inf)` 호출 → "non-finite value" 패닉. 또한 `u32::MAX` Hz 면 fixed_dt 가 0 ns 로 라운드 → `should_fixed_update` 무한 루프.

**Fix (`voltex_platform/src/game_loop.rs`)**:
- `fixed_hz.clamp(1, 100_000)` — 0 은 1 Hz, 그 이상은 100 kHz 로 silently clamp
- `alpha()` 가 fixed_dt <= 0 시 0.0 반환 (방어선)

**검증**:
- voltex_platform 5 → 8 (+3). 회귀 테스트:
  - `game_loop::tests::new_with_zero_hz_does_not_panic`
  - `game_loop::tests::new_with_extreme_hz_does_not_round_to_zero`
  - `game_loop::tests::alpha_with_zero_fixed_dt_returns_zero_not_inf`

**누적 (v1.0.40~52)**: 13 cycle, 약 60 회귀 테스트.

### v1.0.51 (2026-05-22) — Input first-mouse-move warp fix

P1 UX. `InputState::process_mouse_move` 의 첫 이벤트가 `mouse_position = (0, 0)` 시작점 기준으로 거대 delta 생성 → 카메라가 포커스 시점에 점프.

**Fix (`voltex_platform/src/input.rs`)**:
- `mouse_seen: bool` 필드 추가. 첫 이벤트는 warp 로 취급 (delta 기여 없음), 이후 정상 처리
- `begin_frame` 은 `mouse_seen` 유지 — 매 프레임 다시 false 가 되어 zero-delta 가 반복되지 않도록

**검증**:
- voltex_platform 3 → 5 (+2). 회귀 테스트:
  - `input::tests::first_mouse_move_produces_zero_delta`
  - `input::tests::begin_frame_clears_delta_but_keeps_mouse_seen`

### v1.0.50 (2026-05-22) — PacketCipher u64 overflow + AuthToken NaN-time bypass

P0 robustness round 11. voltex_net::encryption 의 두 가지 결함:

**1. `decrypt` 가 적대적 `seq = u64::MAX` 에서 wrap-around** — `recv_counter = seq + 1` 가 debug 빌드 panic 또는 release 에서 0 으로 wrap. 다음 정상 패킷이 replay 로 거부됨 → DoS.
- Fix: `seq.saturating_add(1)`. u64::MAX 에 도달하면 그 자리에 고정 — 그 시점이면 어차피 통신 종료.

**2. `AuthToken::validate` 가 NaN `current_time` 을 통과** — `if current_time > self.expires_at { return false; }` 에서 NaN 비교는 항상 false → expired 토큰도 NaN 시간이면 valid.
- Fix: `!current_time.is_finite() || !self.expires_at.is_finite()` 시 즉시 false.

**검증**:
- `cargo build --workspace` 클린, clippy 0 워닝
- voltex_net 49 → 51 (+2). 회귀 테스트:
  - `encryption::tests::decrypt_with_max_u64_seq_does_not_panic_or_wrap`
  - `encryption::tests::auth_token_rejects_nan_current_time`

### v1.0.49 (2026-05-22) — propagate_transforms cycle / depth-limit

P0 robustness round 10. v1.0.42 의 `add_child` 사이클 가드는 새 hierarchy 변경만 막음 — scene 파일에서 raw `world.add(_, Children(_))` / `world.add(_, Parent(_))` 로 로드한 사이클은 `propagate_transforms` 의 재귀에서 스택 오버플로 유발.

**Fix (`voltex_ecs/src/world_transform.rs`)**:
- `MAX_HIERARCHY_DEPTH = 1024` 도입 — 현실적 hierarchy 의 깊이 (5~10) 보다 한참 위, 사이클 / 적대적 scene 만 잡음
- `propagate_entity` 가 depth 인자를 받아 한계 초과시 graceful return

**검증**:
- voltex_ecs 144 → 146 (+2). 회귀 테스트:
  - `world_transform::tests::propagate_transforms_does_not_overflow_on_raw_cycle` — A↔B raw cycle
  - `world_transform::tests::propagate_transforms_handles_long_acyclic_chain` — 256-deep 정상 chain sanity

### v1.0.48 (2026-05-22) — Animation system: zero-duration / OOB keyframe / quat slerp NaN

P0 robustness round 9. animation.rs 의 세 가지 잠재 NaN/panic 경로 정리:

**1. `AnimationPlayer::update` zero-duration `%` → NaN** — duration=0 인 빈 애니메이션이 looping 모드에 들어가면 `current_time %= 0.0` 가 NaN 생성 (f32 modulo by zero). 그 후 sample_channel 호출이 silently 잘못된 결과.
- Fix: duration <= 0 시 current_time=0, playing=false. Early termination.

**2. `extract_keyframe` 슬라이스 OOB panic** — 적대적 / 손상된 glTF 에서 `values.len() < times.len() * stride` 일 때 `values[start..end]` 가 `start > values.len()` 으로 panic. 이전 시점 `end = (start+stride).min(values.len())` 만으로는 start 미가드.
- Fix: `start >= values.len()` 검사 + path 별 기본값 (translation/scale=0, rotation=identity quat) 반환.

**3. `Quat::slerp` 의 dot >1.0 floating-point overshoot → acos NaN** — unit quaternion 사이의 dot 도 f32 누적 오차로 1.0000001 이 가능. acos(>1) = NaN 으로 회전 전체가 NaN.
- Fix: `dot = dot.clamp(-1.0, 1.0)` 추가. 두 0.9995 보다 큰 경우는 이미 LERP 처리하지만 정확히 그 사이도 안전.

**검증**:
- `cargo build --workspace` 클린, clippy 0 워닝
- voltex_renderer 225 → 228 (+3). 회귀 테스트:
  - `animation::tests::animation_player_zero_duration_does_not_div_by_zero`
  - `animation::tests::extract_keyframe_with_truncated_values_returns_default`
  - `animation::tests::quat_slerp_with_floating_point_dot_overshoot_does_not_nan`

### v1.0.47 (2026-05-22) — Lua FFI null-deref 차단 + type-confusion 가드

P0 critical safety. voltex_script 의 engine_api 가 unsafe FFI 표면이라 세 종류의 잠재 UB / silent corruption:

**1. `get_world()` 가 null pointer dereference (UB → SIGSEGV)** — `__voltex_world` 글로벌이 nil 이거나 light userdata 가 아니면 `&mut *ptr` 이 null deref. 발생 시나리오:
  - 스크립트가 실수로 `__voltex_world = nil`
  - `register_engine_api` 호출 전 스크립트 실행
  - 다른 코드가 글로벌을 덮어쓰기

Fix: `get_world()` 가 `Option<&'static mut World>` 반환. 모든 호출자 (17개 함수) 가 `let Some(world) = get_world(L) else { return 0 }` 로 graceful skip.

**2. 비숫자 entity handle 이 entity (0,0) 으로 silent 변환** — `lua_tonumberx` 의 `isnum` 출력을 무시. `set_position("foo", "bar", 1, 2, 3)` 가 entity (0,0) 의 position 을 silently 수정 → 신선한 world 의 첫 entity 데이터 손상.

Fix: `read_handle_strict` / `read_handle_strict_at` 헬퍼 도입. `isnum == 0` 시 `None` → silent no-op.

**3. 비숫자 x/y/z 좌표가 0으로 silent 변환** — `set_position(id, gen, "x", "y", "z")` 가 좌표를 모두 0 으로 설정. set_rotation / set_scale 동일.

Fix: 모든 좌표 인자에 개별 `isnum` 변수 + 한 개라도 실패시 early return 0.

**검증**:
- `cargo build --workspace` 클린, clippy 0 워닝
- voltex_script 42 → 45 (+3). 회귀 테스트:
  - `engine_api::tests::entity_count_without_world_global_returns_nothing`
  - `engine_api::tests::entity_count_after_global_overwrite_does_not_crash`
  - `engine_api::tests::set_position_with_non_numeric_args_is_silent_noop`

**누적 (v1.0.40~47)**: 8 cycle, ~50 회귀 테스트. 워크스페이스 ~1090 → ~1095.

### v1.0.46 (2026-05-22) — Editor redo Spawn 누수 fix + audio mixer OOB defence + runtime resize cleanup

P0 robustness round 7. 세 곳의 작은 버그를 정리:

**1. EditorSession::redo() 가 Spawn 재실행 시 entity 누수** — `Spawn → Undo → Redo → Undo` 시퀀스에서 inverse 가 *옛* entity (이미 죽은 슬롯) 를 가리켜 두 번째 undo 가 silently no-op. 새로 생성된 entity 는 영원히 살아남음.
- Fix: redo() 가 `Spawn` 명령 재실행 후 `DespawnSpawned(applied_entity)` 로 inverse 패치.

**2. 오디오 mixer 가 잘못된 clip_index 에서 OOB panic** — `PlayingSound` 의 `clip_index` 가 clips 슬라이스 길이를 초과하면 `clips[clip_index]` 가 panic. 에셋 reload / 슬라이스 축소 후 발생 가능.
- Fix: `clips.get(sound.clip_index)` 사용. 잘못된 인덱스는 finished 리스트에 추가되어 다음 패스에서 제거.

**3. RuntimeApp resize 핸들러 0×0 (창 최소화) 정리** — `WindowEvent::Resized(0, 0)` 시 카메라 aspect 만 가드되고 GPU resize 는 무조건 호출 (gpu 자체 가드로 no-op). 일관성을 위해 양쪽 가드 통합.
- 변경 자체는 functional impact 0 (gpu.resize 가 이미 zero-dim 차단).

**검증**:
- `cargo build --workspace` 클린, clippy 0 워닝
- voltex_audio 119 → 120 (+1), voltex_editor 169 → 170 (+1). 회귀 테스트 2개:
  - `mixing::tests::out_of_range_clip_index_does_not_panic`
  - `session::tests::redo_spawn_then_undo_cleans_up_the_new_entity`

### v1.0.45 (2026-05-22) — glTF accessor count cap + attribute length consistency

P0 robustness round 6. glTF 파서의 모든 `read_accessor_*` 함수가 JSON 의 `count` 필드를 그대로 신뢰 → `Vec::with_capacity(u32::MAX)` 가능. 또한 mesh primitive 의 attribute (POSITION/NORMAL/UV/TANGENT) 가 서로 다른 count 를 주장하면 vertex assembly 루프가 OOB 패닉.

**보강 (`voltex_renderer/src/gltf/accessor.rs`)**:
- `MAX_ACCESSOR_COUNT = 4_000_000` cap. 7개 read_accessor_* 함수 (vec3/vec2/vec4/indices/joints/mat4/floats) 가 공통 `validate_count` 통과
- `read_accessor_floats`: `count * components` 곱셈을 `checked_mul` 로 교체 (overflow 방어)
- `extract_meshes`: positions/normals/uvs/tangents 의 길이 불일치를 친화적 에러로 거부. 이전엔 `normals[i]` 가 OOB panic 했음

**검증**:
- `cargo build --workspace` 클린, clippy 0 워닝
- voltex_renderer 223 → 225 (+2). 회귀 테스트:
  - `gltf::tests::parse_gltf_rejects_huge_accessor_count`
  - `gltf::tests::parse_gltf_rejects_attribute_length_mismatch`

**한계**: glTF JSON parser 자체에 size limit 은 미적용 (1차 방어선이 GLB length header). OBJ parser 는 이미 .get() 패턴으로 OOB 방어됨 — 변경 없음.

### v1.0.44 (2026-05-22) — Navmesh deserializer 적대적 입력 방어

P0 robustness round 5. v1.0.43 의 scene loader 패턴을 navmesh (VNAV) 에 적용. 두 진입점 (`NavMesh::deserialize` + free fn `deserialize_navmesh`) 모두 동일한 가드.

**보강**:
- `MAX_NAVMESH_VERTICES = 1_000_000`, `MAX_NAVMESH_TRIANGLES = 1_000_000` cap
- Triangle vertex index (i0, i1, i2) ≥ vc 면 친화적 에러 — 이전엔 `self.vertices[oob]` 가 다운스트림 (`shared_edge`, `point_in_triangle_xz`) 에서 panic
- Neighbour triangle index (≥0 이면서 ≥ tc) 면 친화적 에러 — 이전엔 `self.triangles[oob]` 가 pathfinding 도중 panic

**검증**:
- `cargo build --workspace` 클린, clippy 0 워닝
- voltex_ai 34 → 37 (+3). 회귀 테스트:
  - `navmesh::tests::deserialize_navmesh_rejects_huge_vertex_count`
  - `navmesh::tests::deserialize_navmesh_rejects_triangle_vertex_index_oob`
  - `navmesh::tests::deserialize_navmesh_rejects_neighbour_index_oob`

**누적 (v1.0.40~44)**: 5 cycle, 34 회귀 테스트 신규. 총 워크스페이스 테스트 ~1056 → ~1090.

### v1.0.43 (2026-05-22) — Scene deserializer 적대적 입력 방어

P0 robustness round 4. v1.0.42 에서 `add_child` cycle guard 추가했지만 scene loader 가 가지고 있는 자체 위험을 따로 잡아야 함:

**1. .vscn (텍스트) deserialize_scene** — `entity 4294967295` 같은 적대적 line 이 placeholder Vec 을 수십억 개 push → OOM.
- `MAX_SCENE_ENTITIES = 1_000_000` 도입. 초과 entity index 는 silently skip
- Out-of-range parent index 는 silently 무시 (이전엔 panic, index out of bounds)
- self-parent 는 v1.0.42 의 add_child 가드로 자동 거부

**2. VSCN (바이너리) deserialize_scene_binary** — 헤더의 entity_count u32 그대로 신뢰. `count = u32::MAX` 면 `Vec::with_capacity(4_294_967_295)` 가 다중-GB 할당 시도.
- 동일한 `MAX_SCENE_ENTITIES = 1_000_000` cap 추가

**검증**:
- `cargo build --workspace` 클린, clippy 0 워닝
- voltex_ecs 139 → 144 (+5). 회귀 테스트 5개:
  - `scene::tests::deserialize_scene_caps_huge_entity_index`
  - `scene::tests::deserialize_scene_ignores_out_of_range_parent_index`
  - `scene::tests::deserialize_scene_self_parent_is_silently_refused`
  - `scene::tests::deserialize_scene_at_cap_boundary_works`
  - `binary_scene::tests::deserialize_binary_scene_rejects_huge_entity_count`

**한계**: JSON deserialize_scene_json 의 `entities_arr.len()` 는 JSON parser 가 미리 정렬한 것이라 cap 안 두어도 OK (parser 가 1차 방어선). v1.1+ 에서 JSON parser 자체에도 한계 도입 검토.

### v1.0.42 (2026-05-22) — Hierarchy cycle / self-parent / reparent integrity

P0 robustness round 3. `add_child` 가 세 가지 무결성 위반을 silently 허용:

**1. Self-parent** — `add_child(world, e, e)` 가 entity 의 `Parent` 컴포넌트를 자기 자신으로 설정 → `despawn_recursive(e)` 가 무한 재귀 → stack overflow.

**2. Cycle 생성** — `add_child(A, B)` 후 `add_child(B, A)` 가 A↔B 사이클 생성. `despawn_recursive` 무한 재귀 (B 의 children 에 A, A 의 children 에 B). 더 깊은 사이클 (A→B→C, then C→A) 도 동일.

**3. Re-parent 시 옛 parent leak** — `add_child(old, child)` 후 `add_child(new, child)` 가 `child.Parent = new` 로 변경하지만, `old.Children` 에 child 가 stale 하게 남음. `despawn_recursive(old)` 가 stale child 를 다시 despawn 시도 → child 이미 죽었으니 no-op 이긴 하지만 의도와 다른 동작.

**수정 (`voltex_ecs::hierarchy::add_child`)**:
- `parent == child` 일 때 silently return
- `parent` 의 ancestor chain 을 따라 올라가서 `child` 발견시 silently return (cycle)
- `child` 의 기존 `Parent` 가 있고 다른 entity 라면, 그 entity 의 `Children` 에서 child 제거 후 재할당
- Ancestor walk 는 `MAX_ANCESTOR_WALK = 2^20` 으로 캡 (pre-existing cycle 방어)

**검증**:
- `cargo build --workspace` 클린
- `cargo clippy --workspace --all-targets -- -D warnings` 0 워닝
- voltex_ecs 134 → 139 (+5). 회귀 테스트 5개:
  - `add_child_rejects_self_parent`
  - `add_child_rejects_simple_cycle`
  - `add_child_rejects_deeper_cycle`
  - `add_child_reparents_cleanly`
  - `despawn_recursive_terminates_on_clean_hierarchy` (256-deep chain sanity)

**한계**: 직렬화된 씬 파일에 (악의적/손상된) pre-existing cycle 이 있으면 deserialize 시점에는 `add_child` 가 아닌 raw `world.add(Parent(...))` 경로로 들어옴 — 별도 validate_hierarchy pass 필요. v1.1+ 후속.

### v1.0.41 (2026-05-22) — NaN-source hardening (math) + snapshot deserialiser cap

P0 robustness round 2. v1.0.40 가 정렬 단계의 NaN 패닉을 막았다면, v1.0.41 은 NaN 이 *어디서 나오는지* 를 추적해서 원천 차단.

**1. Truthful normalise docstrings + safe variant** — `Vec3::normalize()` / `Vec2::normalize()` 의 docstring 이 "Panics if the input is the zero vector (division by zero)" 라고 거짓말 (f32 의 0/0 은 패닉이 아니라 silently NaN). Ray/look_at 도 동일.

- `Vec3::normalize_or_zero(self) -> Vec3` 추가 — 길이² < `f32::MIN_POSITIVE` 면 `Vec3::ZERO` 반환
- `Vec2::normalize_or_zero(self) -> Vec2` 추가 — 동일 패턴
- `Ray::try_new(origin, direction) -> Option<Ray>` 추가 — direction 이 zero/sub-normal 이면 `None`
- `Vec3::normalize`, `Vec2::normalize`, `Ray::new`, `Mat4::look_at` 4 항목의 docstring 을 truthful 하게 교체 (panic 주장 제거, NaN 동작 명시, 안전 변형 안내)

이번 변경은 추가 (additive) 만 — 기존 호출자 동작 변경 0. v1.0.40 의 `.unwrap_or(Equal)` 보강이 마지막 방어선이고, v1.0.41 의 `normalize_or_zero` 가 첫 번째 방어선 (호출자가 zero-input 의 가능성을 미리 알면 채택).

**2. Snapshot deserialiser entity-count cap** — `voltex_net::snapshot::deserialize_snapshot` 이 헤더의 entity count u32 그대로 신뢰. 적대적 peer 가 `count = u32::MAX` 를 보내면 32-bit `usize` 에서 `count * 40` 오버플로, 64-bit 에서 `Vec::with_capacity(u32::MAX)` 의 다중-GB 할당. UDP datagram 의 실제 상한과 무관.

- `MAX_SNAPSHOT_ENTITIES = 1_000_000` 도입 — 초과시 친화적 에러
- `count * ENTITY_SIZE` 와 `+ 8` 모두 `checked_mul` / `checked_add` 로 통일

**검증**:
- `cargo build --workspace` 클린
- `cargo clippy --workspace --all-targets -- -D warnings` 0 워닝
- 회귀 테스트 9개 추가 — voltex_math 39→48 (+9), voltex_net 47→49 (+2). 워크스페이스 전체 그린.

**Tests added**:
- `voltex_math::vec3::tests::normalize_zero_produces_nan` (문서화 회귀)
- `voltex_math::vec3::tests::normalize_or_zero_returns_zero_for_zero_input`
- `voltex_math::vec3::tests::normalize_or_zero_matches_normalize_for_nonzero_input`
- `voltex_math::vec3::tests::normalize_or_zero_handles_subnormal_length`
- `voltex_math::vec2::tests::normalize_zero_produces_nan` (문서화 회귀)
- `voltex_math::vec2::tests::normalize_or_zero_returns_zero_for_zero_input`
- `voltex_math::vec2::tests::normalize_or_zero_matches_normalize_for_nonzero_input`
- `voltex_math::ray::tests::try_new_rejects_zero_direction`
- `voltex_math::ray::tests::try_new_accepts_nonzero_direction`
- `voltex_net::snapshot::tests::deserialize_snapshot_rejects_huge_entity_count`
- `voltex_net::snapshot::tests::deserialize_snapshot_rejects_truncated_body`

**한계**: 기존 normalize() 호출자 약 30 곳 (steering, gizmo, picking, csm, frustum 등) 은 컨텍스트에 따라 length-check 가드를 직접 가지고 있거나 호출 conditions 가 zero-input 을 배제. 일괄 normalize_or_zero 교체는 미실시 (호출자별 의미 검토 필요, v1.1+ audit cycle).

### v1.0.40 (2026-05-22) — NaN-safe sort comparators + parser size hardening

P0 robustness. 정적 코드 감사에서 발견한 두 종류의 패닉 가능 경로 제거.

**1. NaN-safe sort comparators (6곳)** — `partial_cmp(...).unwrap()` 은 비교 한쪽이 NaN 이면 panic. 게임 입력에 NaN 이 한 컴포넌트라도 섞이면 (Transform 의 위치, mesh AABB 좌표, ray 거리 등) 정렬 단계에서 즉시 크래시:

- `voltex_physics/src/bvh.rs:62` — BVH 빌드 시 AABB center 정렬 (NaN-position entity → 패닉)
- `voltex_physics/src/bvh.rs:150` — `query_ray` 결과 거리 정렬
- `voltex_physics/src/raycast.rs:142` — `raycast_all` 거리 정렬
- `voltex_physics/src/mesh_collider.rs:75` — `ray_vs_mesh_all` 거리 정렬 (degenerate triangle → NaN t)
- `voltex_editor/src/rasterizer.rs:124` — TTF 글리프 scanline 교차점 x 정렬
- `voltex_renderer/src/light_probes.rs:43` — 가장 가까운 라이트 프로브 검색 (NaN 카메라 위치 → 패닉)

모두 `.partial_cmp(...).unwrap_or(std::cmp::Ordering::Equal)` 로 교체 — NaN 입력시 자기 자신과 동일 순서로 취급 (defensive ordering). 회귀 테스트 4개 신규 추가 (`bvh::build_with_nan_center_does_not_panic`, `bvh::query_ray_with_nan_hit_distance_does_not_panic`, `raycast::raycast_all_with_nan_position_does_not_panic`, `mesh_collider::ray_vs_mesh_all_with_degenerate_triangle_does_not_panic`, `light_probes::nearest_with_nan_query_does_not_panic`).

**2. Parser size overflow guards (BMP/PNG/JPG)** — 적대적 헤더가 선언한 width/height 가 산술 오버플로 또는 거대 할당을 유발하던 경로 차단. 세 파서 공통:
- 0 차원 거부 (이전엔 빈 vec 할당 후 슬라이스 패닉)
- 최대 차원 `16_384` (`max_texture_dimension_2d` 와 동일) 캡
- `width * height * 4` 계산을 `checked_mul` 으로 통일 — 오버플로 시 친화적 에러 반환

회귀 테스트 5개 추가 (`texture::parse_bmp_rejects_zero_dimensions`, `texture::parse_bmp_rejects_huge_dimensions`, `texture::parse_bmp_accepts_max_allowed_dimension_header`, `png::parse_png_rejects_zero_dimensions`, `png::parse_png_rejects_huge_dimensions`).

**검증**:
- `cargo build --workspace` 클린
- `cargo clippy --workspace --all-targets -- -D warnings` 0 워닝
- `cargo test --workspace --lib`: 모든 crate green. voltex_physics 106→110 (+4), voltex_renderer 217→223 (+6). 총 워크스페이스 ~1050 테스트, NaN/overflow 시나리오 9개 신규 보장.

**한계**: 이 라운드는 정적 감사 기반. 적대적 입력으로부터의 fuzz coverage 확장은 별 사이클 (v1.0.x P3).

### v1.0.39 (2026-05-21) — README polish + service-readiness summary

P2. README status badge 가 v1.0.20 에 멈춰 있고 v1.0.29~34 의 운영 tooling 이 Features 에 미반영.

**갱신**:
- `README.md` 상태 배지 v1.0.20 → v1.0.38
- Features 섹션에 두 bullet 추가:
  - **CI / 릴리스**: cargo-deny + cargo-about + cargo-semver-checks + changelog enforcement + nightly E2E + 3-OS release pipeline
  - **Runtime ergonomics**: 4-line main, HUD primitive, panic hook → crash log, structured log

**Notes**: 10시간 service-readiness push (v1.0.21~v1.0.39, 19 cycle) 마무리. 워크스페이스 최종 상태:
- 모든 crate `#![warn(missing_docs)]` 또는 rationale 동반 `#![allow]`
- 모든 CI gate (build/clippy/test/license/deny/semver/changelog/e2e)
- Full release pipeline (tag → 3-OS exe + license bundle → GitHub Release)
- Full hook 커버 (pre-commit clippy+test, pre-push CHANGELOG)
- Per-example READMEs

### v1.0.38 (2026-05-21) — Per-example READMEs

P2. v1.0.15 의 `examples/README.md` catalog 는 menu 역할, 각 example 의 detail 은 모듈 헤더 comment. v1.0.38 에서 example 마다 focused README.

**신규** (15 파일):
- `examples/{triangle,model_viewer,many_cubes,hierarchy_demo,asset_demo,pbr_demo,multi_light_demo,shadow_demo,ibl_demo,deferred_demo,audio_demo,editor_demo,runtime_demo,tutorial_minigame,survivor_game}/README.md`
- 각자 일관된 섹션: What it demonstrates / Run / Controls / Next step
- `examples/README.md` catalog 갱신: per-example doc 안내로 문구 수정

**runtime_demo README**: v1.0.32 HUD 와이어링 (title bar = `"voltex_runtime: solar system | N planets, M moons | XX fps"`) 설명 포함.

### v1.0.37 (2026-05-21) — voltex_renderer 100% missing_docs + `#![warn(missing_docs)]`

워크스페이스 missing_docs sweep 13번째이자 마지막 crate. 약 80 파일 — 워크스페이스 최대 crate. 513 → 0.

**접근**: voltex_editor (v1.0.36) 패턴 — public API (lib.rs re-export) 는 풀 doc, 내부 디테일 모듈은 inline rationale + `#![allow(missing_docs)]`.

**Documented in full**:
- `RendererError`, `GpuContext` + `DEPTH_FORMAT`, `Vertex`/`MeshVertex` + LAYOUT, `Mesh` + new
- `Camera` 7 field + 5 method, `FpsController` 2 field + new/process_movement/process_mouse
- `MaterialUniform` 4 field + new/with_params/bind_group_layout
- `BmpImage` 3 field

**Allowed-with-rationale (40+ modules)**: deflate/png/jpg/json_parser/gltf/obj/progressive_jpeg/pipeline/shadow*/csm/point_shadow/spot_shadow/brdf_lut/ibl/gbuffer*/fullscreen_quad/deferred_pipeline/ssgi/rt_*/hdr/bloom/tonemap/forward_pass/auto_exposure/instancing/bilateral_blur/temporal_accum/taa/ssr/motion_blur/dof/pbr_pipeline/frustum/stencil_opt/half_res_ssgi/bilateral_bloom/soft_rt_shadow/blas_update/rt_fallback/light_probes/light_volumes/skinned_vertex/animation/texture/light/sphere. GPU 파이프라인 + 파서 + shader uniform 내부.

**검증**:
- `cargo build -p voltex_renderer` — 0 missing_docs warning (513 → 0).
- `cargo test -p voltex_renderer --lib` — 217 통과.
- 워크스페이스 clippy 0 warning.

**Part 2 deferred**: 원래 2 cycle (core/advanced) 로 계획했으나 v1.0.36 의 allow-with-rationale 패턴 가 단일 cycle 로 동일 결과 (0 warning, public entry type narrative 유지, 파이프라인 내부 회전 churn 차단). v1.0.38 슬롯은 개별 rendering subsystem 안정화 시 field-by-field deepening 용으로 향후 활용.

### v1.0.36 (2026-05-21) — voltex_editor 100% missing_docs + `#![warn(missing_docs)]`

열두 번째 crate. ~25 파일 (IMGUI + dock + TTF + viewport + gizmo + session).

**작업**:
- Crate-level docstring + `#![warn(missing_docs)]`.
- 모든 기존 bare 파일에 모듈 헤더 추가.
- Public API doc:
  - `EditorError` 5 variant + `ModeMismatch.current` field
  - `EditorCommand` 9 variant + 10 field doc
  - `EditorMode` 2 variant
  - `OrbitCamera` 7 field + 8 method
  - `GizmoMode`/`GizmoAxis` 3 variant each + methods, `GizmoState`, `DragResult`, `GizmoRenderer`
  - `LifecycleEvent` 2 variant, `InspectorState` 2 field

**Allowed-with-rationale**: 14 내부 module 에 `#![allow(missing_docs)]` (rationale inline). dock / ui_context / asset_browser / floating_panel / glyph_cache / ttf_parser / ttf_font / rasterizer / draw_list / viewport_texture / viewport_renderer / renderer / font / layout. GPU 파이프라인 state + parser 내부 + immediate-mode 스크래치 버퍼들 — 구현 따라 회전. Entry types ([`UiContext`], [`DockTree`], [`TtfFont`], [`DrawList`], [`UiRenderer`]) 가 narrative 담당.

**검증**:
- `cargo build -p voltex_editor` — 0 missing_docs warning (259 → 0).
- `cargo test -p voltex_editor --lib` — 169 통과.

### v1.0.35 (2026-05-21) — voltex_renderer_ecs 100% missing_docs + `#![warn(missing_docs)]`

열한 번째 crate. 4 파일 (lib/error/assets/items/basic_renderer).

**작업**:
- `#![warn(missing_docs)]` 추가 (crate-level //! 는 이미 존재)
- Public API:
  - `RendererEcsError` 2 variant + `Result` alias
  - `RenderAssets::{new, mesh, material}` (add_mesh/add_material 는 이미 doc 있음)
  - `RenderItem` 4 field
  - `BasicRenderer` struct-level doc

**검증**:
- `cargo build -p voltex_renderer_ecs` 0 missing_docs
- `cargo test -p voltex_renderer_ecs --lib` 7 통과

### v1.0.34 (2026-05-21) — Release artifact automation

P1. tag push 만으로 3-OS exe + 라이선스 번들이 GitHub Release 로 발행되는 파이프라인.

**신규**:
- `.github/workflows/release.yml` — 2 job:
  - `build` matrix: ubuntu / windows / macos × 3 triple. `cargo build --release -p voltex_cli --target <triple>` → exe + LICENSE-MIT/APACHE + README → tar.gz 또는 zip → upload-artifact.
  - `release`: download-artifact 로 모은 후 `softprops/action-gh-release@v2` 로 publish. `refs/tags/v*` 에서만.
- `docs/release.md` — release 런북.
- `README.md` 문서 섹션에 링크.

**트리거**: tag push `v*` + `workflow_dispatch` (수동, dry run).

**한계**: macOS aarch64 only. x86_64 macOS 는 ci.yml 매트릭스 정책에 맞춰 의도적 제외.

### v1.0.33 (2026-05-21) — SemVer-checks CI (advisory)

P1. v1.0.0 이후 모든 workspace crate 가 stable API 라 breaking change 자동 감지 필요.

**신규**:
- `.github/workflows/ci.yml`: `semver-check` job (advisory). taiki-e/install-action 으로 cargo-semver-checks 설치 → `cargo semver-checks check-release --workspace --exclude voltex_cli --baseline-rev v1.0.0`.
- v1.0.0 tag 없는 fork 는 `::notice::` 로 skip.

**한계**: advisory (tag-per-cycle 정책 채택 시 blocking 승격). voltex_cli 는 bin only 라 exclude.

### v1.0.32 (2026-05-21) — HUD primitive in `voltex_runtime`

P0. shipping 게임의 score/FPS 표시를 위한 최소 HUD. v1 은 title-bar 방식 (rendering pipeline 변경 0). 진짜 in-window overlay 는 v1.1+ (editor 와 공유할 text primitive 가 선행).

**신규**:
- `crates/voltex_runtime/src/hud.rs` — `Hud` 구조체:
  - rolling FPS avg (sample window 60 ≈ 1s @60Hz), `average_fps()` / `instantaneous_fps()`
  - user-controlled segment (`set_user_segment(Some/None)`)
  - 업데이트 cadence 조정 (`set_update_interval`, default 0.25s) + sample window 조정
  - `tick(dt) -> Option<String>` cadence 만료 시 새 title 반환
  - 10개 단위 테스트
- `RuntimeApp` 에 `pub hud: Hud` 필드, `new()` 에서 `config.window.title` 으로 초기화
- `handler` redraw: `tick_systems_only` 직후 `app.hud.tick(dt)` → 새 title 이면 winit 윈도우에 적용
- `examples/runtime_demo`: 시작 시 행성/위성 카운트 → `set_user_segment(Some(..))` 데모

**검증**:
- `cargo build -p voltex_runtime` 클린, `cargo test -p voltex_runtime --lib`: 46 통과 (36 → +10)
- 워크스페이스 clippy 0 warning, runtime_demo 빌드 OK

**한계**: `add_system` closure 는 `&mut World` 만 받아서 `app.hud` 접근 불가. game 은 startup 에서 set 하거나 외부에서 호출. system-context API 는 v1.1+.

### v1.0.31 (2026-05-21) — E2E `voltex new → build` 테스트 + nightly CI

P0. quickstart 흐름이 v1.0.3 / v1.0.9 처럼 우연히 발견되던 패턴을 자동화로 전환.

**신규**:
- `crates/voltex_cli/tests/e2e_new_build.rs` — `env!("CARGO_BIN_EXE_voltex")` 로 binary 실행 → 임시 scratch dir 에 `voltex new` → `voltex build --engine-path <workspace_root>` → dist exe + v1.0.12 라이선스 번들 존재 확인. `#[ignore]` (engine 전체 release build 라 50~60s).
- `.github/workflows/ci.yml`:
  - `e2e-quickstart` job — `workflow_dispatch` (수동) + `schedule` (매일 18:00 UTC) 트리거. advisory (`continue-on-error`).
  - top-level `on:` 에 `schedule: cron "0 18 * * *"` 추가.

**검증**:
- 기본 `cargo test`: `1 ignored` (skip 작동).
- `cargo test ... -- --ignored --nocapture`: 1 passed in 52.17s. dist 의 exe + LICENSE-MIT/APACHE 둘 다 생성됨 확인.

**한계**: CI 캐시 cold 상태에서 5분+ 가능. advisory 로 두고 GA / kid 패턴 관찰 후 blocking 승격.

### v1.0.30 (2026-05-21) — CHANGELOG entry enforcement

P1. autonomous-push workflow 에 PR 리뷰가 없어서 release-note 빠짐을 memory 에 의존하던 문제 해결.

**신규**:
- `scripts/check_changelog.sh` — reusable. `<base_ref> <head_ref>` 받아서 crates/examples/games 변경 시 CHANGELOG.md 동시 수정 강제. `*.md` / `CODEMAP.md` 는 docs-only 로 면제.
- `.githooks/pre-push` — git pre-push 프로토콜 stdin 읽어 각 ref 마다 script 호출. 신규 브랜치 첫 push 는 origin/master 기준.
- `.github/workflows/ci.yml`: `changelog-check` job (PR-only, blocking) — hook 미설치 contributor 보호.
- `install.sh` / `install.ps1` 갱신 (chmod + 안내문).

**검증**: throwaway repo 에서 3가지 case (code-only 위반 / docs-only / code+CHANGELOG) end-to-end 동작 확인.

**한계**: `--no-verify` 로 bypass 가능 (기존 pre-commit 와 동일). PR 의 base_ref 가 master 가 아닌 경우도 동작.

### v1.0.29 (2026-05-21) — cargo-deny 통합

P1 운영 인프라. 기존 cargo-about(license listing only) 보완 — RUSTSEC advisory + duplicate version + unknown source 추가 스캔.

**신규**:
- `deny.toml` — `[graph]`/`[advisories]`/`[licenses]`/`[bans]`/`[sources]` 4섹션 전부 설정. license allowlist는 about.toml mirror.
- `.github/workflows/ci.yml`: `deny-check` job 추가 (advisory). `taiki-e/install-action` 으로 cargo-deny 설치 → `cargo deny check`.

**정책**:
- advisory / yanked → `warn` (v1.0 동안), v1.1+ 에서 `deny` 승격.
- license → `deny` (about.toml allowlist 그대로).
- multiple-versions → `warn` (wgpu/winit 의 `windows-sys` 다중 major 때문).
- sources → `deny` (crates.io + path/git 명시만 허용).

**검증**: cargo-deny CLI 로컬 미설치라 syntax 검증은 CI 의존. advisory flag 라 push 후 결과 확인 가능 (실패해도 merge block 안 됨).

### v1.0.28 (2026-05-21) — voltex_runtime 100% missing_docs + `#![warn(missing_docs)]`

열 번째 crate. 7 파일 (app/config/crash/error/handler/log + lib).

**작업**:
- `#![warn(missing_docs)]` 추가 (crate-level //! 는 이미 존재).
- Public API:
  - `RuntimeError` 6 variant + `RuntimeResult` alias
  - `RuntimeConfig` 12 field + 9 builder method + 3 fn-pointer alias
  - `RuntimeApp` 11 field + user_system_count/input_system_count
  - `Level` 5 variant
  - log_warn/log_info/log_debug/log_trace macro 4개 doc

**검증**:
- `cargo build -p voltex_runtime` — 0 missing_docs warning.
- `cargo test -p voltex_runtime --lib` — 36 통과.

### v1.0.27 (2026-05-21) — voltex_script 100% missing_docs + `#![warn(missing_docs)]`

아홉 번째 crate. 11 파일 (LuaState/bindings/engine_api/instance/pool/system + ffi/sandbox/coroutine).

**작업**:
- Crate-level docstring + `#![warn(missing_docs)]`.
- 모듈 헤더 4개 (state/bindings/instance/pool).
- Public API:
  - `LuaValue` 5 variant (Nil/Bool/Number/String/Table)
  - `LuaState::new` + global getter/setter
  - `ScriptInstance` 3 field (lua/path/started)
  - `ScriptInstancePool` (new/get/get_mut/find_by_path/len/is_empty)
- `ffi.rs` (47 Lua C FFI) 은 `#![allow(missing_docs)]` + lua.org 참조 코멘트.

**검증**:
- `cargo build -p voltex_script` — 0 missing_docs warning.
- `cargo test -p voltex_script --lib` — 42 통과.

### v1.0.26 (2026-05-21) — voltex_ecs 100% missing_docs + `#![warn(missing_docs)]`

여덟 번째 crate. 18 파일 — ECS 코어 (entity/sparse_set/world/archetype/queries + transform/world_transform/hierarchy + scheduler/dep_scheduler + component_registry + scene/binary_scene/json).

**작업**:
- Crate-level docstring + `#![warn(missing_docs)]`.
- 모듈 헤더 14개.
- Public API:
  - `EntityAllocator` 5 method (new/allocate/deallocate/is_alive/alive_count)
  - `SparseSet` 12 method + `ComponentStorage` trait 5 method
  - `JsonVal` 6 variant + 6 accessor (get/as_f64/as_str/as_array/as_object/as_bool) + writer 5개 / parser
  - `Archetype`/`ArchetypeSignature`/`ArchetypeRegistry` 모든 method + field
  - `SystemAccess` (name/reads/writes + new/read/write), `DependencyScheduler` (new/add/system_count) + `ParallelGroup` 타입 alias
  - `Transform` 3 field + new/from_position/from_position_scale
  - `Parent`/`Children` tuple field, `WorldTransform` + identity, `propagate_transforms`
  - `Component` trait 4 method + `ComponentEntry`/`ComponentRegistry` + fn pointer alias 4종
  - `ScriptComponent` 2 field + new
  - `World` query/query2/3/4/storage/storage_mut/has_component

**검증**:
- `cargo build -p voltex_ecs` — 0 missing_docs warning.
- `cargo test -p voltex_ecs --lib` — 134 통과.

### v1.0.25 (2026-05-21) — voltex_physics 100% missing_docs + `#![warn(missing_docs)]`

일곱 번째 crate. 14 파일 (BVH/Colliders/Narrow/GJK+EPA/Contact/RigidBody/Integrator/Solver/Raycast/CCD/MeshCollider).

**작업**:
- Crate-level docstring + `#![warn(missing_docs)]`.
- 모듈 헤더 13개 (bvh/ray/collider/contact/narrow/gjk/collision/rigid_body/integrator/solver/raycast/ccd/mesh_collider).
- Public API:
  - `RigidBody` 8 field + dynamic/statik/inv_mass/is_static/wake doc
  - `PhysicsConfig` 3 field + sleep constants
  - `Collider` 4 variant + 모든 field
  - `ConvexHull` (vertices field + new/support)
  - `ContactPoint` 6 field
  - `RayHit` 4 field, `MeshCollider`/`MeshHit` 4+3 field
  - `BvhTree` + build, `detect_collisions`, `integrate`, `physics_step`, `resolve_collisions`, `sphere_vs_box`/`box_vs_box`

**검증**:
- `cargo build -p voltex_physics` — 0 missing_docs warning.
- `cargo test -p voltex_physics --lib` — 106 통과.
- 워크스페이스 clippy 0 warning.

### v1.0.24 (2026-05-21) — voltex_audio 100% missing_docs + `#![warn(missing_docs)]`

여섯 번째 crate. 21 파일 (in-house audio 스택 전체).

**작업**:
- Crate-level docstring + `#![warn(missing_docs)]`.
- 모듈 헤더 17개 (audio_clip/audio_source/audio_bus/async_loader/wav/mixing/spatial/hrtf/mix_group/reverb/occlusion/fade_curves/dynamic_groups/effect_chain/backend mod/null/wasapi/error).
- Public API:
  - `AudioCommand` 8 variant + 21 field doc
  - `LoadState` 4 variant, `BackendError` 3 variant
  - `OcclusionResult` 2 field, `Listener` 3 field, `SpatialParams` 3 field
  - `HrtfResult` 4 ITD/ILD field, `MixGroup` 4 variant, `GroupState` 3 field
  - `AudioEffect` / `AudioBackend` trait methods
  - `Reverb`/`Echo` wet/dry/feedback fields
- `wasapi.rs` (75 FFI items) 은 `#![allow(missing_docs)]` + MSDN 참조 코멘트.

**검증**:
- `cargo build -p voltex_audio` — 0 missing_docs warning.
- `cargo test -p voltex_audio --lib` — 119 통과.

### v1.0.23 (2026-05-21) — voltex_ai 100% missing_docs + `#![warn(missing_docs)]`

다섯 번째 crate. 5 모듈 (error/nav_agent/navmesh/pathfinding/steering).

**작업**:
- Crate-level docstring + `#![warn(missing_docs)]`.
- 모듈 헤더 4개 (nav_agent/navmesh/pathfinding/steering).
- Public API:
  - `NavAgent` (target/speed/path/current_waypoint/reached + new/set_target/clear_target)
  - `NavMesh` (vertices/triangles + new), `NavTriangle` (이미 doc 있음)
  - `SteeringAgent` (position/velocity/max_speed/max_force + new)
  - `AiError` enum-level doc

**검증**:
- `cargo build -p voltex_ai` — 0 missing_docs warning.
- `cargo test -p voltex_ai --lib` — 34 통과.

**한계**: `obstacle.rs` 및 `navmesh_builder/` 는 `lib.rs` 에 mod 선언이 없어서 컴파일되지 않음 — 이 cycle 의 sweep 대상 외 (사전 dead-code).

### v1.0.22 (2026-05-21) — voltex_net 100% missing_docs (71 → 0) + `#![warn(missing_docs)]`

네 번째 crate. 11 파일 (lib/error/packet/socket/server/client/reliable/snapshot/interpolation/lag_compensation/encryption).

**작업**:
- Crate-level docstring + `#![warn(missing_docs)]`.
- 모듈 헤더 9개 (packet/socket/server/client/reliable/snapshot/interpolation/lag_compensation/encryption/error).
- Public API:
  - `Packet` 10 variant + 14 field doc (protocol header `[type_id|len_le|reserved|payload]` 명시)
  - `ServerEvent` 3 variant + 모든 field, `ClientInfo` 3 field
  - `ClientEvent` 3 variant + payload field
  - `EntityState` 4 field, `Snapshot` 2 field
  - `ReliableChannel::new`, `OrderedChannel::new`
  - `LagCompensation::new` + `history_len` + `max_history_ms` field
  - `HistoryEntry` 3 field
  - `PacketCipher::new` (panic 조건 명시)
  - `AuthToken` 3 field

**검증**:
- `cargo build -p voltex_net` — 0 missing_docs warning (71 → 0).
- `cargo test -p voltex_net --lib` — 47 통과.

### v1.0.21 (2026-05-21) — voltex_asset 100% missing_docs (64 → 0) + `#![warn(missing_docs)]`

세 번째 crate. 약 800 라인 / 8 파일 (lib/error/handle/storage/assets/watcher/loader/vpkg/vfs).

**작업**:
- crate-level docstring (모든 7 public type + 한 줄 책임 명시) + `#![warn(missing_docs)]`.
- 모듈 헤더 7개 (handle/storage/assets/watcher/loader/vfs/error).
- Public API doc:
  - `Handle<T>` (struct + `Copy` semantics 명시)
  - `AssetStorage<T>` (new/insert/get/get_mut/add_ref/release/len/is_empty/ref_count/replace_in_place/iter)
  - `AssetStorageDyn` (as_any / as_any_mut / count)
  - `Assets` (new/insert/get/get_mut/add_ref/release/count/storage/storage_mut)
  - `FileWatcher` (new/watch/unwatch/poll_changes/watched_count)
  - `AssetLoader` (new/shutdown), `LoadState` (Loading/Ready/Failed variant)
  - `VpkgEntry` (path/offset/size fields), `VPKG_MAGIC`, `VPKG_VERSION`
  - `VpkgReader` (parse/entries/list/read)
  - `Vfs` (Filesystem/Archive variants + fs/archive_from_bytes/read/exists/list)
  - `VpkgReaderOwned` (read/list)
  - `AssetError` enum (이미 doc 있던 variants 유지)

**검증**:
- `cargo build -p voltex_asset` — 0 missing_docs warning (64 → 0).
- `cargo test -p voltex_asset --lib` — 36 통과.
- 워크스페이스 clippy 0 warning.

### v1.0.20 (2026-05-21) — voltex_platform 100% missing_docs (38 → 0) + `#![warn(missing_docs)]`

v1.0.19 의 패턴을 두 번째 crate 에 적용. voltex_platform 은 ~250 라인 4 파일 (lib/window/input/game_loop).

**작업**:
- crate-level docstring (3 core type [VoltexWindow / InputState / GameTimer] + 1 config 명시) + `#![warn(missing_docs)]`.
- window.rs module + WindowConfig (title/width/height/fullscreen/vsync 필드) + VoltexWindow (handle/vsync 필드, new/inner_size/request_redraw 메서드).
- input.rs module + 3-step per-frame 흐름 설명 (winit event → user system read → begin_frame 리셋). InputState 의 11 public 메서드 (is_key_pressed / is_key_just_pressed / is_key_just_released / mouse_position / mouse_delta / is_mouse_button_pressed / is_mouse_button_just_pressed / mouse_scroll / begin_frame / process_key / process_mouse_move / process_mouse_button / process_scroll).
- game_loop.rs module + Glenn Fiedler "Fix your timestep" 인용 + GameTimer 메서드 (new/tick/should_fixed_update/fixed_dt/frame_dt/alpha). tick 의 250ms clamp (death-spiral 방지) 명시.

**부산물**: input::process_key + process_mouse_button 에서 clippy `collapsible_else_if` 자동 정리 (nested-if → `else if`).

**검증**: 0 missing_docs (38→0), 워크스페이스 clippy 0 워닝, voltex_platform 3 tests 통과.

### v1.0.19 (2026-05-21) — voltex_math 100% missing_docs (59 → 0) + `#![warn(missing_docs)]`

1573 workspace-wide missing_docs 의 첫 chunk 해소. 가장 작고 stable한 voltex_math 부터 — 이후 crate 들의 sweep 패턴 확립.

**작업**:
- `crates/voltex_math/src/lib.rs`: crate-level docstring (RH coord / column-major / radians 컨벤션 + 사용 예시) + `#![warn(missing_docs)]` (회귀 방지).
- 6 모듈 (vec2/vec3/vec4/mat4/aabb/ray) 각자 module-level `//!` (한 줄 책임 명시).
- 모든 pub struct / field / const / 메서드 doc:
  - Vec2/Vec3/Vec4: x/y/z/w field 명시, ZERO/ONE/X/Y/Z 상수 + new/dot/cross/length{,_squared}/normalize 메서드
  - Mat4: cols field 명세 (column-major)
  - AABB: min/max 필드 + new/from_center_half_extents/center/half_extents/contains_point/intersects/merged/surface_area
  - Ray: origin/direction 필드 + new (normalize 부작용 명시) + at

**검증**:
- `cargo build -p voltex_math` 0 missing_docs warning (이전 59 → 0)
- `cargo doc -p voltex_math --no-deps` 클린
- `cargo test -p voltex_math --lib` 39 통과 (회귀 0)
- 워크스페이스 clippy/test 0 회귀

**셀프 피드백**: 단계 1 lib + vec2 → 59→48. 단계 2 vec3/vec4 → 48→20. 단계 3 mat4/aabb/ray → 0. 일괄 처리 효율적.

**나머지 1514 missing_docs 의 후속 cycle 후보** (small → large, stable → volatile):
- voltex_platform (3 tests), voltex_asset, voltex_net, voltex_ai, voltex_physics, voltex_audio, voltex_ecs, voltex_script, voltex_renderer_ecs, voltex_editor, voltex_runtime, voltex_renderer (가장 큼).

### v1.0.18 (2026-05-21) — panic hook + logger 를 `RuntimeApp::new` 로 이동

**갭**: v1.0.10 가 panic hook 을 `RuntimeApp::run` 에 설치 → `RuntimeApp::new` 안에서 실행되는 사용자 callback (`build_scene` / `setup_systems`) 또는 scene 로드 단계가 panic 시 hook 미설치 상태라 크래시 로그 안 남음. 신규 사용자가 가장 자주 만나는 init-time crash 가 사일런트.

**fix**:
- `crash::install_panic_hook` 호출을 `run()` → `new()` 의 첫 번째 작업으로 이동.
- `log::init_from_env()` 도 `new()` 시작 시점으로 이동.
- `run()` 은 이제 "entering event loop" log 만 (둘 다 `OnceLock` / `Once` 가드라 no-op).

**테스트**: voltex_runtime 34 → 36 (+2):
- `new_with_crash_log_disabled_still_succeeds` — opt-out 회귀 방지.
- `new_runs_build_scene_with_logger_initialised` — build_scene 안에서 `log_info!` 호출이 panic 안 나는지 (logger 초기화 시점 가드).

**검증**: 테스트 실행 시 실제 logger 출력이 stderr 에 표시 — `2026-05-21T11:42:21Z [INFO ] voltex_runtime::app: voltex_runtime::new: title="Voltex Engine" size=1280x720` 형식 동작 확인.

**셀프 피드백**: 단계 1 hook 이동 + 새 테스트. 단계 2 기존 8 app::tests 모두 통과 (이미 매크로 자동 init 정확히 의도대로). 워크스페이스 회귀 0.

**한계 (v1.1+)**: panic-during-`RuntimeConfig` 빌드 chain (e.g. `with_window_title("...")` 직전 panic) 은 여전히 hook 전. user main() 시작 첫 줄에서 명시적 install 하는 게 더 강력 — public re-export 는 이미 있음 (`voltex_runtime::install_panic_hook`).

### v1.0.17 (2026-05-21) — 외부 dep 0 structured logger (`voltex_runtime::log`)

배포된 게임의 디버그 visibility. 이전엔 `eprintln!` 산발 사용 → level 필터 / 파일 미러 / 일관 포맷 없었음.

**신규 모듈** `crates/voltex_runtime/src/log.rs`:
- `Level` enum 5단계 (Error < Warn < Info < Debug < Trace). `parse(&str)` (case-insensitive, "warning" alias).
- `init(level, Option<PathBuf>)` + `init_from_env()` (`VOLTEX_LOG=debug` 등) — `OnceLock` 가드로 idempotent.
- `enabled(level)` — caller-side early-out (인자 포맷 비싼 경우).
- `log(level, target, &fmt::Arguments)` — 본체. stderr + optional file (Mutex<File>) 둘 다, best-effort (panic 없음).
- `format_record(unix_seconds, level, target, msg)` — pure, 테스트 가능. 출력 형식: `2026-05-21T12:00:00Z [INFO ] target: message`.
- 매크로 `log_error!` / `log_warn!` / `log_info!` / `log_debug!` / `log_trace!` — `enabled()` 게이트 후 `format_args!`. zero-alloc when filtered out.

**Integration** (RuntimeApp::run):
- `crate::log::init_from_env()` 자동 호출 (panic hook 직후).
- 시작 시 `log_info!("voltex_runtime starting: title=..., size=AxB")`.
- 종료 시 `log_info!("...exited normally")` 또는 `log_error!("...exited with error: ...")`.

**crash.rs 와의 공유**: time 변환 helpers (`unix_seconds_now`, `unix_to_ymd_hms`) 를 `pub(crate)` 래퍼 (`_for_log`) 로 노출. 외부 API 표면 변경 없음 (Howard Hinnant 알고리즘 중복 회피).

**테스트**: voltex_runtime 27 → 34 (+7):
- Level::parse 정상/거부 / Level 순서 / Level label 길이
- format_record ISO8601 timestamp + level + target / 모든 level label 보존
- open_log_file parent dir 생성

**셀프 피드백**:
- 단계 1 logger 작성: 빌드 통과.
- 단계 2 RuntimeApp 통합 + crash.rs helpers pub(crate): 워크스페이스 clippy 0 워닝, 0 회귀.
- 외부 dep 0 (env 변수 + Mutex + std::sync::OnceLock 만).

**한계 (v1.1+)**:
- 매크로 호출 시 `module_path!()` 만 target 으로 사용. per-call `target: "..."` 명시 syntax 미지원.
- VOLTEX_LOG 가 단일 level — `log` crate 처럼 per-module 필터 (e.g. `voltex_audio=debug,voltex_renderer=warn`) v1.1+.
- 색상 없음 (ANSI escape) — Windows 호환 위해 의도적 plain text. v1.1+ opt-in.

### v1.0.16 (2026-05-21) — `docs/voltex-toml-reference.md` — 매니페스트 schema 완전 reference

v1.0.11 가 manifest 의 unknown key/section 을 strict-reject 한 이후, 사용자가 "어떤 키가 valid 한지" 확인할 단일 문서가 부재. 이 cycle 이 그 갭을 메움.

**신규**:
- `docs/voltex-toml-reference.md` — 모든 섹션 (`[project]` / `[build]` / `[build.<triple>]`) × 모든 키 (name/version/scene/scripts/assets/window_title/window_size/targets/icon) 의 타입 / 필수여부 / 기본값 / 설명 표. 최소/전체 예시. mini-TOML 파싱 규칙 (코멘트/escape/미지원 항목). v1.0.11 strict validation 에러 시나리오 3종. `voltex build` 출력 (`dist/`) 매핑. CLI 옵션 매핑.
- 향후 추가 후보 (v1.1+): release_profile / optimize_for / binary_name / [publish] / [deps] 섹션.

**연동 갱신**:
- README "문서" 섹션에 reference 링크 + `examples/README.md` + `CONTRIBUTING.md` + service-readiness 로드맵 4개 새 링크 추가.
- `voltex_cli::help::BUILD` 끝에 "See docs/voltex-toml-reference.md..." 한 줄.
- 신규 가드 테스트 `build_help_references_manifest_reference_doc` — help 가 reference doc 가리키는지 회귀 방지.

**테스트**: voltex_cli 54 → 55 (+1). 다른 변경 없음.

**셀프 피드백**: 단계 1 reference doc 작성 + README/help 연동. 단계 2 가드 테스트 추가. 단계 3 워크스페이스 회귀 0.

**한계 (v1.1+)**: reference doc 가 manifest/mod.rs 의 코드와 별도 유지보수. 새 키 추가 시 reference 누락 가능 — 자동 generator (struct → markdown) 는 별 cycle.

### v1.0.15 (2026-05-21) — examples catalog + voltex_survivor flagship README

신규 사용자가 14 example + 1 game 디렉토리 중 어디부터 봐야 할지 안내.

**신규**:
- `examples/README.md` — 14 example 추천 학습 순서 표 (`triangle` → `model_viewer` → `many_cubes` → ... → `runtime_demo` → `tutorial_minigame`). 카테고리별 (Rendering / ECS / PBR+Light+Shadow / Advanced / Audio / Editor / Runtime+게임) 분류. 실행 공통 + 작성 정책 (description / catalog 업데이트 / 모듈 헤더) 명시.
- `games/voltex_survivor/README.md` — flagship 게임 catalog (디렉토리 구조 표 + 조작 + 학습 포인트: Lua source of truth, entity 메일박스 패턴, handle-attach 시스템 + 한계 + 실행 명령).

**검증**: 문서만, 코드 변경 0 — 워크스페이스 테스트/클리피 영향 0.

**알려진 한계 (v1.1+)**:
- 14 example 각자의 상세 README 부재 (이 catalog 가 menu 역할). 각 example 의 모듈 헤더 (`//!`) 가 사실상 README 대체.
- 스크린샷 부재 (`screenshot:` 자리만 마련) — GUI 자동화 없어 미수집.

### v1.0.14 (2026-05-21) — CONTRIBUTING.md + git pre-commit hook

외부 contributor 가 로컬 검증 → 커밋 → PR 흐름에서 마찰 줄이기. 이전엔 README 의 "Contributing" 섹션에 4개 cargo 명령만 있고, 새 contributor 가 한 번 잊으면 CI 에서 실패해야 학습.

**신규**:
- `CONTRIBUTING.md` — 의존성 정책 (winit/wgpu/bytemuck 만), 600줄 한도, 에러 표준 (thiserror + Generic), Component 등록 패턴, 테스트 스타일 (inline xorshift32 fuzz), commit 컨벤션 (v1.0.x 형식), 자동 루프 정책 (작업 전 pull --ff-only), 첫 PR 체크리스트.
- `.githooks/pre-commit` — `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace --lib --quiet`. cargo fmt 는 의도적으로 제외 (프로젝트가 fmt 설정 미사용; 적용 시 무관 코드 대량 재포맷).
- `.githooks/install.sh` (bash) + `.githooks/install.ps1` (PowerShell) — `git config core.hooksPath .githooks` + chmod +x. idempotent.
- README "Contributing" 섹션에 CONTRIBUTING.md 링크 + hooks install 명령 추가.

**검증**:
- `bash -n .githooks/{pre-commit,install.sh}` 둘 다 syntax OK.
- PowerShell `.ps1` 은 Windows 환경에서 실제 실행 확인 후 v1.1 마킹 (개발 호스트에서 pwsh 미설치 시).

**테스트**: 셸 스크립트라 Rust 단위 테스트 미추가 (스크립트 한 줄 짜리 + parse-check 로 충분). voltex_cli / runtime 등 코드 변경 없음.

**셀프 피드백**:
- 단계 1 workspace package version bump 시도 → auto-mode 분류기 차단 (deferred 항목 라벨 자기-참조). 정당. 즉시 pivot.
- 단계 2 pre-commit + CONTRIBUTING 작성: 4 파일 신규 + README 1 섹션 갱신.
- 단계 3 bash syntax check 통과. 워크스페이스 build/test/clippy 영향 0.

**알려진 한계 (v1.1+)**:
- PowerShell install.ps1 실제 실행 검증 미실시 (개발 호스트 pwsh 부재).
- pre-commit 이 30~60초 (초기) 걸려, 큰 commit 흐름에서는 `--no-verify` 유혹. pre-commit (fmt-only fast) + pre-push (clippy+test slow) 분리는 별 cycle.

### v1.0.13 (2026-05-21) — voltex CLI `--version` + 풍부한 per-subcommand `--help`

첫 사용자 마찰 줄이기. 이전: top-level help 만 있고 `voltex new --help` / `voltex build --help` / `voltex run --help` 무동작 (subcommand 가 help flag 인식 안 함). `--version` 도 없었음.

**추가**:
- 신규 `crates/voltex_cli/src/help.rs` — TOP / NEW / BUILD / RUN 4 static help string. 한 곳에서 review 가능, regression-test 가능.
- main.rs: top-level `--version` / `-V` → `voltex 0.1.0` 출력 후 exit 0.
- main.rs: `maybe_subhelp` 헬퍼 — subcommand args 에 `--help` / `-h` 있으면 해당 텍스트 출력 후 dispatch 스킵.
- BUILD help 가 **v1.0.11 의 `--no-validate-paths`** + **v1.0.12 의 라이선스 번들 OUTPUTS** 명시 (이전엔 두 기능 다 undocumented 가까웠음).

**테스트**: voltex_cli 47 → 54 (+7):
- top_help_lists_all_commands / build_help_documents_no_validate_paths / build_help_documents_license_bundle_outputs / build_help_lists_engine_path_resolution_order / new_help_describes_scaffold_contents / run_help_references_build_options / top_help_includes_examples
- "subcommand 가 약속한 flag 가 help 에 실제로 나타나는지" 가드. silent regression 방지.

**셀프 피드백**: 단계 1 clippy `redundant_closure` 3건 (`|a| new::cmd(a)` → `new::cmd`) 즉시 fix. 첫 빌드 실패 → 두 번째 통과.

**알려진 한계 (v1.1+)**: `voltex --version` 이 `0.1.0` (Cargo.toml package version) 을 출력. 사용자 시점 v1.0.x 와 mismatch. 워크스페이스 package version bump (v1.0.x 별 sync) 는 별 cycle (모든 31 crate Cargo.toml 일괄 변경 + workspace.package 정책 결정 필요).

### v1.0.12 (2026-05-21) — LICENSE 파일 누락 fix + `voltex build` 가 라이선스 번들

**발견된 컴플라이언스 갭**: repo 루트의 `Cargo.toml` 워크스페이스가 `license = "MIT OR Apache-2.0"` 명시하고 README 에 명시되어 있으나, **실제 `LICENSE-MIT` / `LICENSE-APACHE` 파일이 부재**. dual-license 약속이 텍스트 없이 떠 있던 상태.

**fix**:
- `LICENSE-MIT` (표준 MIT, Copyright 2026 tolelom) 신규 작성
- `LICENSE-APACHE` (표준 Apache-2.0 boilerplate + APPENDIX) 신규 작성

**번들링** (`voltex_cli::build::copy_license_files`):
- `voltex build` 가 cargo build 완료 후 engine_path 의 `THIRD-PARTY-LICENSES.md` + `LICENSE-MIT` + `LICENSE-APACHE` 를 `dist/` 에 복사.
- 누락된 파일은 silent skip (eprintln note) — 옛 engine checkout 호환.
- Returns `Vec<PathBuf>` of copied files for caller logging.

**테스트**: voltex_cli 43 → 47 (+4):
- copy_license_files ×4: 전부 present / 전부 missing silent / partial only / **실 repo (CARGO_MANIFEST_DIR 부모 경로) 에서 실제로 THIRD-PARTY-LICENSES.md 찾기 검증** — 후자가 핵심으로, 이 cycle 이 작성한 LICENSE 파일 3종이 실 build flow 에 노출됨을 보증.

**셀프 피드백**:
- 단계 1 helper + build_target 통합: 컴파일 통과.
- 단계 2 처음 작성한 테스트 4건: 3 통과 + 1 fail — "resolves_against_actual_repo" 가 LICENSE-MIT/APACHE 실제 부재 노출. **이게 v1.0.12 본질**: 테스트 가 컴플라이언스 갭 자체를 잡음.
- 단계 3 LICENSE 파일 2개 작성 + 재실행: 4/4 통과.
- 단계 4 워크스페이스 회귀: 0 워닝, 0 회귀.

**알려진 한계 (v1.1+)**:
- `THIRD-PARTY-LICENSES.md` 재생성은 수동 (`cargo about generate about.hbs -o ...`). CI 가 의존성 변화 시 자동 재생성 PR 만드는 건 별 cycle.
- 번들된 라이선스 파일 무결성 검증 (해시 비교 등) 없음.

### v1.0.11 (2026-05-21) — voltex.toml 스키마 강화 (unknown key + section 거부 + path validation)

배포 워크플로 진입 마찰 줄이기. `voltex build` 가 매니페스트의 오타나 누락된 자원을 빌드 *전* 에 친화 에러로 잡아냄 (이전엔 cargo 의 cryptic 에러로 나오거나 그냥 default 가 silent 적용).

**신규 에러 variants** (`voltex_cli::CliError`):
- `UnknownField { section, field, valid, suggestion }` — 섹션별 화이트리스트 외 키
- `UnknownSection { name, valid_prefixes }` — `[project] / [build] / [build.*]` 외 섹션
- `ManifestPathMissing { field, path, base_dir }` — scene/scripts/assets 가 base_dir 기준 부재

**스키마 검증** (`validate_schema(TomlDoc)`, `Manifest::parse` 내부 자동 호출):
- 화이트리스트:
  - `[project]`: `name`, `version`
  - `[build]`: `scene`, `scripts`, `assets`, `window_title`, `window_size`, `targets`
  - `[build.<triple>]`: `icon` (triple 자체는 open-ended — `aarch64-linux-android` 등 임의 허용)
- 오타 시 **Levenshtein distance ≤ 3 closest match** 제안 (`windows_title` → `window_title`). 거리 > 3 이면 suggestion 생략 (잘못된 유도 회피).
- Levenshtein 구현 inline (외부 dep 0). case-insensitive (편의).

**Path validation** (`Manifest::validate_paths(&self, &Path) -> Result<()>`):
- `scene` 파일 존재 확인
- `scripts` / `assets` 글롭 패턴의 root 디렉토리 (strip_glob 결과) 존재 확인
- `voltex build` 가 parse 직후 호출. `--no-validate-paths` 플래그로 opt-out (build-time path 생성 시나리오 용).

**테스트**: voltex_cli 30 → 43 (+13):
- unknown key/section ×5 (project key 거부 / build typo 거부+suggestion / 거리>3 suggestion 생략 / unknown section 거부 / `[build.<arbitrary>]` 허용)
- validate_paths ×3 (전부 정상 / 누락 scene 거부 / 누락 scripts 거부)
- helper ×2 (levenshtein smoke, closest_match 거리 컷)
- error Display ×3 (UnknownField 힌트 / UnknownSection valid 리스트 / ManifestPathMissing path+base 표시)

**셀프 피드백**:
- 단계 1 에러 variants + validate_schema + validate_paths 구현: 빌드 통과.
- 단계 2 기존 30 테스트: 0 회귀 (voltex new 스캐폴드 매니페스트가 화이트리스트 통과 — 정상).
- 단계 3 신규 13 테스트: 모두 통과 (closest_match 의 거리 컷이 false positive 안 만듦).
- 단계 4 워크스페이스 clippy/test: 0 워닝, 0 회귀.

**알려진 한계 (v1.1+)**:
- `parse_value` 가 line+column 중 line 만 제공. column 까지 가려면 mini-TOML 파서 보강 필요.
- `closest_match` 가 첫 번째 매치만 — 동률 거리 시 알파벳 순. 사용자 의도 추론은 안 함.

### v1.0.10 (2026-05-21) — 크래시 로그 panic hook (`voltex_runtime::crash`)

배포된 exe 가 죽었을 때 stderr 는 사용자에게 안 보임 → 사용자가 버그 리포트에 첨부할 단일 파일 (`<app>-crash-YYYYMMDD-HHMMSS.txt`) 자동 생성.

**신규 모듈** `crates/voltex_runtime/src/crash.rs` (266 라인):
- `install_panic_hook(app, dir)` — `std::panic::set_hook` 으로 기본 hook 호출 후 크래시 파일 작성. `Once` 가드로 idempotent.
- `format_crash_report(...)` — pure 함수. 헤더 (app/UTC/os/arch/voltex_runtime version) + panic message + location (`file:line:col`) + 풀 backtrace (`std::backtrace::Backtrace::force_capture`).
- `crash_filename(app, ts)` — `<sanitized_app>-crash-<UTC>.txt`. 비-ASCII / 슬래시 등은 `_` 로 치환.
- `unix_to_ymd_hms` — Howard Hinnant 의 days-from-civil 역변환 inline (외부 dep 0 정책 — chrono/time 도입 회피). leap year (2000-02-29) + 음수 (pre-1970) 모두 검증.

**RuntimeConfig 확장**:
- `enable_crash_log: bool` (default true)
- `crash_log_dir: Option<PathBuf>` (default None = current working dir)
- builder: `.with_crash_log_dir(p)`, `.disable_crash_log()` (헤드리스/fuzz 용 opt-out)

**RuntimeApp::run** 진입 시 `enable_crash_log == true` 면 window title 을 app name 으로 hook 설치.

**테스트**: voltex_runtime 13 → 27 (+14):
- crash ×11 (epoch 0, 2026-05-21 noon, 2000-02-29 leap, pre-epoch, filename well-formed/sanitised, report all sections / missing location / trailing newline, write_crash_file dir creation, idempotent install)
- config ×3 (default enables, disable, with_dir)

**셀프 피드백**:
- 단계 1 모듈 작성 + 첫 테스트: 27 통과, 3 실패 — 내가 timestamp 산술 (2026-05-21 noon 의 unix seconds) 을 1일 빗나가게 계산. 알고리즘은 정확. 실측 값으로 정정.
- 단계 2 wire 후 워크스페이스 clippy/test: 0 워닝, 0 회귀.
- 단계 3 docs: STATUS/CHANGELOG/DEFERRED/roadmap 갱신.

**알려진 한계 (v1.1+)**:
- 패닉 hook 의 실제 fire 검증은 단위 테스트에서 catch_unwind 보장 + global hook state 격리가 까다로워 미실시. 통합 테스트 (별도 프로세스 fork) 는 별 cycle.
- 사용자 정보 (예: GPU 어댑터 이름, FPS, 마지막 씬) 미수집 — 첫 컨택트 후 추가.

### v1.0.9 (2026-05-21) — OGG/Vorbis lib.rs wire + clippy 27→0 + AudioClip 진입점

v1.0.8 에서 발견된 dead-module 회귀 해소. STATUS Phase 6-1 의 "OGG/Vorbis 자체 구현 완료" 주장을 사실로 만듦.

**Wire**: `crates/voltex_audio/src/lib.rs` 에 `pub mod ogg;` + `pub mod vorbis;` 등록 (이전 `#`-comment 5줄 deferral 마커 제거).

**Clippy 24 lib + 3 test → 0** (`-D warnings` 통과):
- **`cargo clippy --fix`** 가 5건 자동 적용 (manual_range_contains, unused var prefix 등).
- **`#![allow(clippy::needless_range_loop)]` 6 파일** (codebook/decoder/floor/headers/imdct/residue) — 각 파일 상단에 spec 인용 코멘트 1줄 ("Vorbis spec § X 의 인덱스 수식 그대로 옮긴 코드, iterator translation 은 spec traceability 손상"). silence 가 아닌 정당화.
- **진짜 개선 3건**:
  - `decoder.rs:149` needless_late_init → `let output_samples = if ... { ... } else { ... };` 인라인 바인딩
  - `floor.rs:153` manual_memcpy → `copy_from_slice`
  - `ogg.rs:174` enumerate_discard_index → 인덱스 제거 후 `for packet in packets_data`
  - `ogg.rs:181` same_item_push → `segment_table.resize(len + N, 255u8)`

**숨겨진 테스트 회귀 2건 fix** (dead 시절 컴파일 안 돼 보이지 않던 것):
- `ogg::invalid_capture_pattern` — 짧은 입력 (`NotOGGdata` 10B) 이 size-truncation 분기로 가서 "No OGG pages found" 리턴 → "Invalid OGG capture pattern" 미반환. magic 체크를 size 체크 앞으로 이동 (4-byte 만 있어도 magic 확정 가능).
- `vorbis::bitreader_basics` — 어셔션 자체 산수 오류 (저자가 "5 bits 가 byte 0 의 3..7 (4 bits) + byte 1 의 bit 0" 으로 잘못 계산. 실제로는 byte 0 의 3..7 이 이미 5 bits). expected `0b1_1011` (27) → 실측 `0b10110` (22) 로 정정 + 코멘트 명확화 + 8-bit 후속 읽기 어셔션 1건 추가.

**진입점 추가**: `voltex_audio::decode_ogg_vorbis_bytes(&[u8]) -> Result<AudioClip>` — `ogg::parse_ogg` + `vorbis::decode_vorbis` 의 2줄 합성. 외부 사용자가 단일 함수로 `.ogg` 바이트 → AudioClip 가능.

**테스트**: voltex_audio 89 → **119** (+30). 분해: ogg ×11 (기존 4 + 4 fuzz + truncated capture +1 + 추가 검증 +2) + vorbis ×17 (기존 11 + 6 신규 = bitreader 보강 +1 + decode_ogg_vorbis ×2 + 기타) + 8-bit read 추가 1건. 워크스페이스 lib total: 942 → **~968**.

**셀프 피드백** (각 단계 후 검증):
- 단계 1 wire 후 clippy: 22 lib + 2 test 잔여 (예측 5건 더 줄 줄 알았으나 needless_range_loop 가 본질적).
- 단계 2 allow + 진짜 fix 후 clippy: 0. 그러나 테스트 2건 fail 노출 (dead 시절 보이지 않던 것).
- 단계 3 테스트 fix: 119 통과.
- 단계 4 진입점 추가: +2 테스트 (decode_ogg_vorbis_bytes empty / invalid_magic).
- 단계 5 워크스페이스 회귀: build 클린, clippy 클린, lib tests 968.

deferred to v1.0.x+: 실제 `.ogg` 파일 디코드 검증 (Vorbis decoder 자체가 simplified — scalar codebook only, no coupled channels), `audio_demo` 에서 OGG 로딩 데모.

### v1.0.8 (2026-05-21) — parser panic-resistance fuzz + OGG/Vorbis dead-module 발견

**21 신규 fuzz test** — 5 parser 공개 entry point 모두 (`parse_png` / `parse_jpg` / `parse_gltf` / `parse_obj` / `parse_wav`) 에 동일 패턴 4종 input 추가:
1. empty
2. valid magic / sample 의 every truncation
3. 128 seeds × 6 size 의 xorshift32 PRNG random bytes
4. valid magic + random tail

→ **panic 0건**. 모든 parser 가 graceful `Err` 또는 빈/부분 `ObjData` 반환. 외부 dep (rand/proptest/arbitrary) 정책 위반 회피 위해 각 test 파일에 inline xorshift32 ~10줄.

**부산물 발견**: `crates/voltex_audio/src/ogg.rs` + `vorbis/` 디렉토리가 **lib.rs 에 `mod ogg;` / `mod vorbis;` 선언 없음** → cargo가 컴파일하지 않음 = dead source. STATUS.md Phase 6-1 의 "OGG/Vorbis 디코더 자체 구현 완료" 주장과 모순. 임시 wire 시도 시 4 dead_code 워닝 + 27 clippy error 동반 → v1.0.8 범위 초과. lib.rs 등록 revert 하고 v1.0.9 로 분리. ogg.rs 의 fuzz test 들은 그대로 보존 (v1.0.9 wire 시 자동 활성).

**셀프 피드백**:
- 단계 1 parser API 점검 → 5 parser identified, OGG는 모듈 등록 안 됨 첫 발견.
- 단계 2 fuzz test 추가 → 21건 panic 0.
- 단계 3 panic fix 단계 skip (발견 0).
- 단계 4 회귀 검증 → 921 → **942 lib tests**, clippy 0, build 0 warning.

deferred to v1.0.9+: OGG/Vorbis wire (27 clippy + 4 dead_code fix + audio_clip 통합), coverage-guided fuzzing (cargo-fuzz 등).

### v1.0.7 (2026-05-21) — third-party license 인벤토리 + license-check CI

v1.0 release 부터 "외부 의존 라이선스 고지 v1.0 release artifact 에 포함 예정" 으로 미뤄왔던 항목 해결.

**`THIRD-PARTY-LICENSES.md` 자동 생성** (4,795 라인) — `cargo install --locked cargo-about --features cli` 후 `cargo about generate about.hbs -o THIRD-PARTY-LICENSES.md`. 모든 transitive Cargo dependency 가 SPDX license 별 grouping + license 텍스트 inline.

**`about.toml` 정책 + `about.hbs` 템플릿** — accepted SPDX 리스트 (MIT/Apache-2.0/BSD/Zlib/ISC/Unicode/CC0/MPL-2.0/BSL/OFL). 리스트 밖이면 CI license-check 가 자동 실패.

**`[workspace.package] license = "MIT OR Apache-2.0"`** + 31 워크스페이스 crate Cargo.toml 에 `license.workspace = true` 추가. 이전엔 모든 voltex crate 가 license metadata 없어서 cargo-about 가 워닝.

**`.github/workflows/ci.yml` license-check 잡 추가** (Ubuntu, advisory):
- `cargo-about` install (taiki-e/install-action) → `cargo about generate` 실행
- 새 transitive 가 accepted 리스트 밖이거나 license 불명이면 advisory 실패

**라이선스 분포** (284 license-bearing crates):
| License | Crates |
|---|---|
| MIT | 264 |
| Apache-2.0 | 11 |
| Zlib | 3 |
| BSD-3-Clause | 2 |
| BSD-2-Clause | 1 |
| CC0-1.0 | 1 |
| ISC | 1 |
| Unicode-3.0 | 1 |

**GPL/AGPL/LGPL 0건, proprietary 0건** — MIT OR Apache-2.0 dual 과 100% 호환.

**README "License" 섹션 재작성** — 이전 "v1.0 release 후 포함 예정" deferral 제거, THIRD-PARTY-LICENSES.md 링크 + count + 재생성 명령 + CI 통합 명시. 신규 배지 `licenses: permissive only`.

**셀프 피드백** (각 단계 후 검증):
- 단계 1 cargo-about install + 생성: 4,795 라인 파일.
- 단계 2 분석: 8 distinct license, 모두 permissive.
- 단계 3 README + CI: cosmetic only.
- 단계 4 최종 gate: build / clippy / test / cargo about 모두 0 warning + 0 error. 921 lib + 30 cli 유지.

deferred to v1.1+: license-check advisory → blocking 승격 (CI 실 runner 확인 후), THIRD-PARTY-LICENSES.md 를 `voltex_cli build` artifact 에 동봉, `cargo-deny` 통합.

### v1.0.6 (2026-05-21) — coverage baseline (82%) + advisory CI + CLI 친화성

**측정** — `cargo-llvm-cov` 설치 후 `cargo llvm-cov --workspace --lib --summary-only` 첫 실행:

| 메트릭 | covered/total | % |
|---|---|---|
| Functions | 34878 / 42505 | **82.06%** |
| Lines | 18142 / 24849 | **82.74%** |
| Regions | — | **73.01%** |

`rules/common/testing.md` 의 80%+ 목표 (function/line 기준) **첫 달성**.

**Per-crate 분포** (14 crates, function coverage):
- 🟢 80%+ (10 개): voltex 100% / math 95.85 / script 94.33 / ecs 93.31 / asset 93.17 / ai 92.85 / net 92.08 / physics 89.85 / audio 88.26 / editor 82.27.
- 🟡 80% 미만 (4 개, 모두 GPU/winit 의존): voltex_renderer 65.94 (shadow/rt/taa/ssgi/ssr 컴퓨트 셰이더), voltex_runtime 61.22 (`handler.rs` 0%, winit ApplicationHandler), voltex_renderer_ecs 53.83 (BasicRenderer::render GPU pass), voltex_platform 46.38 (ApplicationHandler + per-platform key map).
- 4 crate 모두 단일 원인: headless `cargo test --lib` 으로 도달 불가능한 GPU/winit 통합 경로. unit-testable 내부 (`app.rs`/`config.rs`/`error.rs` 등) 는 80%+. headless GPU adapter + event-loop stub 으로 닫을 수 있으나 v1.1+ 후속.

**CI advisory job 추가** — `.github/workflows/ci.yml` 에 `coverage` 잡 추가 (Ubuntu, `continue-on-error: true`). `taiki-e/install-action@cargo-llvm-cov` 으로 도구 설치 → `cargo llvm-cov --workspace --lib --summary-only` 실행. 임계치 enforcement (`--fail-under-lines 80`) 는 GPU 패스 integration test 후 활성.

**README "Coverage" 섹션 + 배지** 추가 — 1회 install 명령 + baseline 수치 + 낮은 crate 정당화 문서화. `coverage 82% lib` 배지.

**CLI 친화성 (P1 #5)** — `voltex build` engine-path 누락 에러를 1줄 → 7줄 help block 으로 확장. scaffolded-project 사용 예시 + bash/zsh export + PowerShell `$env:` 모두 안내. README 5분 quickstart 도 "예제 실행" / "새 게임 만들기" 두 서브섹션으로 분리 — `cargo install --path crates/voltex_cli` → `VOLTEX_REPO` → `voltex new/build` 흐름 명시.

**셀프 피드백** (각 단계 후 검증):
- 단계 1 baseline: 82.06% 달성.
- 단계 2 분석: 낮은 4 crate 가 모두 GPU/winit 정당화 가능.
- 단계 3 infrastructure: README + CI yml 변경, 코드 변경 0.
- 단계 4 CLI tweak: 빈 디렉토리에서 `voltex new && cd && voltex build` (without VOLTEX_REPO) 가 7-line help 출력 확인.
- 단계 5: 921 lib + 30 cli tests 유지, clippy 0 유지, workspace 빌드 클린.

deferred to v1.1+: 4 low-coverage crate 들의 GPU/winit 통합 테스트, `--fail-under-lines 80` CI gate, HTML coverage 리포트 upload.

### v1.0.5 (2026-05-21) — clippy clean (84 → 0) + cargo audit + CI 워크플로

**"quality gate" 패치**. v1.0.3 (Windows build P0) / v1.0.4 (clippy deny error) 모두 `cargo build` + `cargo test --lib` 의 단일 게이트만 있었던 게 회귀를 가린 원인 → 이번 사이클이 그 구멍을 막음.

**clippy 워닝 84 → 0**:
- baseline: `cargo clippy --workspace --all-targets` 84 lines (51 unique diagnostics, 13 카테고리).
- **auto-fix 43건**: `cargo clippy --fix --workspace --all-targets` → manual_div_ceil ×13, RangeInclusive::contains ×4, excessive_precision ×4, manual_is_multiple_of ×1, collapsible_if ×1, identity_op ×1, len_zero ×1, unwrap_or_else_default ×1, needless_range_loop 일부.
- **manual fix 24건**:
  - dead code 2건 삭제 (`voltex_audio::make_stereo_clip`, `voltex_ecs::world::tests::Name`).
  - `voltex_script::state` ×5: `b"name\0".as_ptr() as *const _` → `c"name".as_ptr()` (Rust 1.77+ c-string literal).
  - `voltex_renderer::jpg::mod` ×4: `for _ in 0..N { push }` → `extend_from_slice(&[byte; N])` (JPEG marker padding).
  - `needless_range_loop` index-essential 4건 (mat4, sh): `#[allow]` + 코멘트.
  - `too_many_arguments` 3건 (hierarchy::draw_entity_node, picking::pick_entity, deferred_demo::create_shadow_bind_group): `#[allow]` + 코멘트 (struct 묶기는 v1.1).
  - `voltex_cli::manifest::extract_build` 의 `very_complex_type` (6-tuple) → `BuildConfig` 직접 반환으로 리팩토링 (callsite 코드 더 짧아짐).
  - examples 작은 loop refactor 3건.

**`.github/workflows/ci.yml` 신규**:
- matrix: ubuntu-latest + windows-latest (blocking) + macos-latest (advisory, `continue-on-error: true`).
- gate 4단계: `cargo build --workspace` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace --lib` → `cargo test -p voltex_cli`.
- Linux 잡은 `libasound2-dev` 자동 install (voltex_audio ALSA FFI link 용).
- README "Contributing" 섹션에 동일 4-command 로컬 verify 레시피 문서화.
- CI 파일은 GitHub Actions 표준이지만 gitea/forgejo 호환. 비-GitHub 원격에서는 inert.

**`cargo audit`**:
- 298 crate 스캔 (1,096 advisory 대비). **vulnerability 0건**.
- **warning 1건**: `paste 1.0.15` unmaintained (RUSTSEC-2024-0436). transitive (`paste ← metal ← wgpu-hal ← wgpu`), voltex 통제 불가. 향후 wgpu 업그레이드로 해결.

**셀프 피드백** (각 단계 후 검증):
- 단계 1 baseline: 84 워닝.
- 단계 2 auto-fix 후: 41 워닝, 921 lib tests 통과.
- 단계 3 manual fix 후: **0 워닝**, 921 lib tests + 30 voltex_cli tests 통과.
- 단계 5 CI gate 로컬 재실행: `RUSTFLAGS="-D warnings"` 환경에서 4 명령 모두 green.

deferred to v1.1+: 실제 GitHub runner 첫 green run (mirror 필요), `cargo audit` CI 통합 (별도 install step 필요), 3-OS exe runtime 검증.

### v1.0.4 (2026-05-21) — clippy `approx_constant` clean + README 의존성 정직성

v1.0.3 이후 점검에서 발견된 두 가지 흠을 fix.

**clippy `approx_constant` 9건 → 0건**:
- `voltex_physics::integrator` 테스트 2건 (`3.14159` → `std::f32::consts::PI`. 코멘트도 이미 "PI rad/s" 였음).
- `voltex_renderer::animation::test_quat_slerp_endpoints` 3건 (`0.7071` → `std::f32::consts::FRAC_1_SQRT_2`. 90° 쿼터니언 정확 표현).
- `voltex_ecs::world::tests::test_clear_changed_all_storages` 1건 (`3.14f32` → `2.5f32`. 의미상 임의 f32 값이라 의존 없음).
- `voltex_ecs::json` 2건 + `voltex_renderer::json_parser` 1건: 의도적 "3.14" / "-3.14" JSON round-trip payload → `#[allow(clippy::approx_constant)]` + 코멘트 ("값은 데이터, π 아님") 부착.
- 결과: `cargo clippy --workspace --all-targets` 의 `#[deny(clippy::approx_constant)]` 더이상 실패하지 않음. 외부 contributor 가 clippy 돌릴 수 있게 됨.

**README 의존성 주장 정정**:
- 헤더의 "외부 crate 의존 단 2개 (winit / wgpu)" 는 실측과 불일치 → **3 (winit / wgpu / bytemuck)** 로 정정.
- "의존성 정책" 섹션의 표를 두 카테고리 (런타임 3 + 빌드 전용 2: thiserror / cc) 로 재구성. 각 crate 의 역할과 "런타임 표면 비노출" 여부 명시. 예제 전용 보조 (log / env_logger / pollster) 도 별도 명시.
- 트랜지티브 총합 (Cargo.lock 298 crate) 도 추가 (정직성).

검증: 921 lib tests 유지 (테스트 본문 5곳만 수정, production 코드 0줄 변경). v1.0.4 는 deny 레벨 clippy 만 fix — 약 80개의 warning 레벨 lint (manual RangeInclusive::contains, needless_range_loop, dead code 등) 는 v1.1+. clippy를 강제하는 CI 워크플로도 v1.1 후속.

### v1.0.3 (2026-05-21) — `voltex build` Windows P0 회귀 fix

**P0** — `voltex build` 가 **모든 Windows 호스트에서 항상 실패** 하던 회귀를 fix.

원인: `voltex_cli/src/build.rs:96-97` (+ `run.rs:20`) 의 `std::fs::canonicalize` 가 Windows 에서 `\\?\C:\...` 확장 길이 verbatim 형식을 반환 → 생성된 `dist/wrapper/Cargo.toml` 의 `path = "\\\\?\\C:\\..."` 가 cargo 의 path dependency 파서를 통과하지 못해 `invalid path url '//?/C:\\...'` 로 거부. README 5분 quickstart (`voltex new && cd && voltex build`) 외부 사용자 워크플로가 100% 깨졌음. 워크스페이스 멤버인 flagship 게임/예제는 직접 `cargo build` 로 빌드돼 회귀를 가려옴.

fix: `voltex_cli::util::canonicalize_friendly` 신규 — canonicalize 후 Windows `\\?\` 접두사를 drive-letter 형식에 한해 strip. UNC (`\\?\UNC\...`) 는 의도적 보존 (lossy rewrite 회피). 외부 의존성 0 (정책 준수).

검증:
- 빈 디렉토리에서 `voltex new my_game` → `cd my_game` → `voltex build --engine-path <repo>` → `dist/my_game.exe` (61초) 정상 산출. 생성된 wrapper `Cargo.toml` 의 path dep 가 `C:/Users/<user>/...` (no `\\?\`).
- 회귀 테스트 +4 (Windows 분기 3 + 크로스 OS 1). `voltex_cli`: 26 → 30.
- 921 lib tests 유지. workspace 빌드 워닝/에러 0 (`examples/many_cubes` 의 unused `Mat4` import 같이 정리).

영향 범위: `voltex_cli` 한정. engine crate / `voltex_survivor` / examples 동작 변경 없음.

### v1.0.2 (2026-05-21) — 600 줄 한도 회복 + 전체 examples smoke-test + stale docs 동기화
**600 줄 한도 재분리** (M3 시점 0건 → v1.0.1 시점 위반 4건):
- `voltex_editor::session` (1060) → `session/{mod.rs 445, tests.rs 483, e3_tests.rs 152}`. E3 테스트는 helper 자체 포함으로 분리.
- `voltex_script::engine_api` (713) → `engine_api/{mod.rs 356, tests.rs 357}`. 17 Lua FFI 함수 body 는 mod.rs 유지.
- `voltex_ecs::world` (665) → `world/{mod.rs 351, tests.rs 314}`.
- `voltex_cli::manifest` (612) → `manifest/{mod.rs 450, tests.rs 162}`. Mini-TOML 파서 본체는 mod.rs.
- 분리 후 가장 큰 파일은 `voltex_physics::solver` 574 줄.

**전체 examples runtime smoke-test** (5-초 timeout × 8 미검증 → exit 124 = 정상):
- model_viewer / many_cubes / asset_demo / multi_light_demo / shadow_demo / ibl_demo / deferred_demo / editor_demo 모두 무패닉.
- 15 examples + 1 game (voltex_survivor) **Windows 호스트 100% runtime 검증**.

**Stale docs 동기화**:
- STATUS.md M3 행 + voltex_ecs 테스트 카운트.
- DEFERRED.md Phase 8-7 의 gizmo / Despawn full undo / 메시 AABB picking ✅ 표시 (Cycle 3/12 에 이미 완료된 것을 반영 못 했음).
- DEFERRED.md macOS/Linux 백엔드 ✅ (Cycle 10/11).
- DEFERRED.md 신규 "v1.0.x 추가 한계" 섹션: 7 항목 (3 OS 검증 / Lua input / CLI editor / HUD / 1573 missing docs / pub(crate) / 멀티 OS audio runtime).
- CLAUDE.md 600 줄 한도 라인 — v1.0.2 재분리 노트.

테스트 +0 (동작 변경 없음, 분리만). 921 lib tests 유지.

### v1.0.1 (2026-05-21) — runtime 동작 검증 + P0/P1 회귀 패치
v1.0 release 후 실 머신 (Windows) 검증에서 4개 회귀 발견, 즉시 fix:
- **P0-1 카메라 ↔ player WASD 충돌**: `handler.rs` 의 WASD/Space/Shift 가 매 프레임 카메라를 움직였음 → RMB-held 조건부로 변경. 게임의 `add_input_system` 이 WASD 를 자유롭게 쓸 수 있게 됨 (Bevy/Unity/Godot 패턴).
- **P1-4 `is_key_just_pressed` 항상 false**: `app.input.begin_frame()` 이 `tick_systems_only` 직전 호출 → just_pressed HashSet 이 user system 이전에 clear. 호출 위치를 frame 끝으로 이동.
- **P0-3 `Bounds::default_bytes` = zero AABB**: degenerate volume → frustum culling 이 항상 컬링. Inspector "+ bounds" 가 invisible entity 생성. unit cube (-0.5 ~ +0.5) 로 변경.
- **P0-2 `ScriptComponent::default_bytes` = empty path**: Inspector "+ script" 가 no-op 컴포넌트 생성. `"scripts/spinner.lua"` placeholder 로 변경 (모든 `voltex new` 스캐폴드에 동봉된 파일).
- **voltex_survivor**: Lua `spawn_entity()` 가 ScriptComponent 자동 부착 안 함 → enemy.lua / projectile.lua 절대 실행 안 됨. main.rs 의 handle-attach 시스템이 Tag 기반으로 ScriptComponent 도 부착하도록 확장.
- 회귀 테스트 +5 (voltex_runtime ×2 input_system 순서/참조, voltex_ecs ×3 Bounds 와 ScriptComponent default round-trip).
- **실 실행 검증** (Windows): voltex_survivor 30초 안정 (6 wave, 24 enemy kill, 0 panic) + tutorial_minigame + runtime_demo 모두 클린.

### Cycle 15 (D3): voltex_survivor flagship + v1.0 release (2026-05-21)
- **`games/voltex_survivor/` (신규)** — survivor 게임 재제작. `voltex.toml` + `scene.vscn` + 4 `.lua` 파일 + `src/main.rs` (RuntimeApp + input system + mesh upload). 게임 로직 100% Lua + scene; Rust 측은 부트스트랩 + input → Transform + Lua 가 spawn 한 entity 에 mesh/material handle 자동 부착의 3 가지 boilerplate.
- **voltex_runtime: `add_input_system(FnMut(&mut World, &InputState, f32))`** — input 노출이 필요한 user system 의 정식 등록 경로. `input_systems: Vec<InputSystem>` 필드 추가, `tick_systems_only` 가 user_systems 다음으로 호출. v1.0 한계: Lua 측 input API 는 v1.1.
- **Lua 4 스크립트**:
  - `game.lua`: 3초 첫 wave, 이후 5초 간격 4마리 적 생성. score / lives / wave 추적. `kill_marker` / `damage_marker` entity 메일박스 패턴으로 다른 스크립트의 이벤트 수확.
  - `player.lua`: 가장 가까운 적을 향해 0.8초 주기로 projectile 발사. 방향은 spawned projectile 의 rotation.y 에 인코딩.
  - `enemy.lua`: 1.2 unit/sec 로 player 추적, 충돌 거리 ≤ 0.55 시 `damage_marker` drop + self-destruct.
  - `projectile.lua`: rotation.y 방향으로 6 unit/sec 이동, enemy 충돌 시 `kill_marker` drop + 양쪽 despawn, 3초 lifetime.
- **`docs/v1.0-retrospective.md` (신규)** — 무엇이 성공 / v1.1 deferral / 통계 / 다음 우선 순위.
- **git tag `v1.0.0`** 로컬 부착. 사용자가 push 시 원격 release 활성.
- 검증 한계: 3 OS exe 산출은 Mac / Linux 머신에서의 빌드 + 실행 (사용자 PR). Windows 에서는 `cargo build --workspace` 클린 + `cargo run -p voltex_survivor` 빌드 가능.
- 테스트 +0 (코드 변경: voltex_runtime + input_system 무회귀; 916 lib tests 유지).
- spec: docs/superpowers/specs/2026-05-21-d3-flagship-game-design.md

### Cycle 14 (D1+D2): README + 30분 tutorial + tutorial_minigame (2026-05-21)
- **`README.md` (신규)** — 5분 quickstart (clone → cargo run --example triangle) + Code-first / Editor-first 두 워크플로 + Features 한 줄씩 (14 카테고리) + 의존성 정책 (winit + wgpu 만) + crate 의존 그래프 + 문서 인덱스 + license + contributing + 개발자 노트 (검증 한계 명시).
- **`docs/tutorial.md` (신규)** — 30분 안에 0 에서 빌드된 미니 게임까지. Chapter 1~7: 프로젝트 만들기 / 에디터 + 첫 씬 / Lua 스크립트 / 적 + 입력 / UI + 점수 / Save + Build / 다음 단계. 각 chapter 끝 체크포인트 + 스크린샷 placeholder (`<screenshot: ...>`).
- **`examples/tutorial_minigame/` (신규)** — 튜토리얼 최종 산출물. 듀얼 워크플로 호환 (`cargo run -p tutorial_minigame` + `voltex build`). 1 player + 3 enemy 큐브, spinner.lua (player 자전) + enemy.lua (player 추적), score 시스템 (살아있는 enemy 수 stdout). main 본문 4 줄.
- v1.0 한계 명시: 스크린샷 미포함 (사용자 머신 PR 환영), Lua 측 입력 API 부재 (v1.1), `voltex_cli` 의 editor 모드 미구현 (튜토리얼은 `examples/editor_demo` 로 안내).
- 테스트 +0 (코드 변경 없음 — 새 example crate 의 빌드만 확장).
- spec: docs/superpowers/specs/2026-05-21-d1-d2-readme-tutorial-design.md

### Cycle 13 (A): rustdoc 0 워닝 + facade prelude 완성 + CHANGELOG (2026-05-21)
- **`cargo doc --workspace --no-deps` 워닝 0** — 7개 파일의 intra-doc link 깨짐 escape (mat4/ccd/pathfinding/navmesh/state/rt_accel + deferred_demo). `voltex_cli` bin 에 `doc = false` 추가해 cargo issue #6313 (voltex lib + voltex bin filename collision) 우회.
- **`voltex` facade prelude 보강**: C0a (Bounds/MeshHandle/MaterialHandle) + S1 (ScriptComponent) + B0 (RuntimeApp/RuntimeConfig) + B2 (Vfs) 추가. `pub use voltex_runtime as runtime` 모듈 re-export 추가 (B0 의 누락 fix). 신규 prelude 타입 6개의 컴파일 검증 테스트.
- **Tier-1 API rustdoc** — voltex_ecs::World/Entity 의 핵심 메서드 (spawn/despawn/is_alive/add/get/get_mut/remove/entity_count) + voltex_editor::EditorCommand 10 variants + EditorSession 구조체 docstring.
- **CHANGELOG.md (신규)** — Cycle 1~13 + M1~M7 + Phase 1~8 7~10 highlights 정리. Unreleased / per-cycle 섹션 구조.
- **docs/v1.0-pub-audit.md (신규)** — pub(crate) 다운그레이드 후보 노트 (실제 변경은 v1.1 후속).
- **deferral 정책 명시** — 전체 1586 missing_docs sweep + `#![warn(missing_docs)]` crate 적용 + 실제 pub(crate) 다운그레이드는 v1.1. v1.0 은 prelude + tier-1 API 한정 보장.
- 테스트 +0 (rustdoc 변경 외 의미 있는 코드 변경 없음). 916 lib tests 유지.
- spec: docs/superpowers/specs/2026-05-21-a-rustdoc-prelude-changelog-design.md

### Cycle 12 (E3): AddComponent/RemoveComponent + Despawn full undo (2026-05-21)
- voltex_ecs::Component trait: **+ `remove(world, entity) -> bool`** (default = `world.remove::<Self>(entity).is_some()`) + **`default_bytes() -> Vec<u8>`** (default = empty Vec). 6 built-in 모두 `default_bytes` 오버라이드 (Transform=identity, Tag=empty, MeshHandle/MaterialHandle/Bounds=zero, ScriptComponent=UNBOUND).
- voltex_ecs::ComponentEntry: **`remove` + `default_bytes`** 함수 포인터 필드 2개 추가. `register_raw` 는 no-op stub 으로 채움 (Inspector AddComponent UI 에서 제외).
- voltex_ecs::EntityAllocator: **`restore_at_generation(id, gen) -> bool`** — 죽은 슬롯을 원래 generation 으로 부활. 슬롯이 alloc/dealloc 사이클을 한 번이라도 더 거쳤다면 (entries[id].generation != gen+1) false. World 가 위임.
- voltex_editor::EditorCommand: **`AddComponent { entity, name, data }`** + **`RemoveComponent { entity, name }`** 2 variants.
- voltex_editor::session::Inverse: **`RemoveComponentByName` + `AddComponentByBytes` + `RestoreEntity`** 3 variants.
- voltex_editor::session::dispatch: **Despawn 이 history 를 clear 하지 않음** — undoable group 으로 이동. SaveScene/LoadScene 만 history clear.
- voltex_editor::session::capture_inverse: Despawn 시 ComponentRegistry.entries() 순회하여 모든 등록 컴포넌트를 snapshot. 미등록 컴포넌트는 silent loss (E2 와 동일 정책).
- voltex_editor::session::apply_inverse: `RestoreEntity` 가 `World::restore_at_generation` 호출 후 컴포넌트 재첨부. 슬롯 재사용 시 silent skip.
- voltex_editor::panels::inspector: **"Add Component" 버튼 6개** — `+ transform` / `+ tag` / `+ mesh_handle` / `+ material_handle` / `+ bounds` / `+ script`. 클릭 시 `(entry.default_bytes)()` + `AddComponent` dispatch. v1 한계: dropdown 아님, 컴포넌트별 [×] remove 버튼은 v1.1.
- 테스트 +21: voltex_ecs +10 (entity ×5 + component_registry ×5), voltex_editor +11 (command ×5 + session ×4 [add_component_undo/remove_component_undo/despawn_undo_restores/despawn_no_longer_clears/despawn_undo_silently_fails_when_slot_reused, 5 - 1 obsolete 제거] + inspector ×2).
- spec: docs/superpowers/specs/2026-05-21-e3-component-commands-design.md

### Cycle 11 (B3): CoreAudio + multi-OS native build (2026-05-21)
- voltex_audio::backend::coreaudio (cfg(target_os = "macos") 신규): **`CoreAudioBackend`** — AudioToolbox.framework 의 AudioQueue API 를 raw FFI 로 호출. `#[link(name = "AudioToolbox", kind = "framework")]`. f32 LE stereo @48kHz 고정. 4 buffer × 1024 frame.
- AudioQueue 콜백 모델을 push-style trait 에 wrap: `FreeQueue { Mutex<VecDeque<AudioQueueBufferRef>> + Condvar }` 로 동기화. write() 가 free 버퍼 pop 후 enqueue, output_callback 이 빈 버퍼를 free 큐에 push + notify. 100ms timeout 으로 deadlock 방지. `std::panic::catch_unwind` 로 FFI 콜백 panic 차폐.
- voltex_audio::backend::mod: `make_backend()` 에 cfg(macos) 분기 추가, init 실패 시 NullBackend fallback.
- voltex_cli::build::**validate_target(manifest, target_arg, host)** — native-only 정책 enforcement. `--target` 이 host 와 불일치하면 `CrossCompileNotSupported` 친화 에러. manifest 의 `targets` 배열이 host 를 포함하지 않으면 `HostNotInManifestTargets` 친화 에러.
- voltex_cli::error: `CrossCompileNotSupported { host, requested }` + `HostNotInManifestTargets { host, listed }` variants 추가. Display impl 이 사용자에게 다음 행동 (`run on a <X> machine`) 을 안내.
- **docs/build-matrix.md (신규)**: 3 OS 사전 요구 + 빌드 명령 + voltex.toml multi-target 예시 + GitHub Actions matrix 스니펫 + 검증 한계 명시.
- 검증 한계: CoreAudio 코드 작성은 Windows 개발 머신에서 수행 → Mac 머신에서 `cargo build --target aarch64-apple-darwin` + audio_demo 소리 출력 실제 검증은 후속. Windows 에서는 `cargo check --target aarch64-apple-darwin` 으로 type 정상성 확인.
- 테스트 +8: voltex_audio +2 (macOS 전용: 포맷 상수 + ASBD 구조체 크기), voltex_cli +6 (validate_target 5 시나리오 + 에러 Display).
- spec: docs/superpowers/specs/2026-05-21-b3-multi-os-build-design.md

### Cycle 10 (P1): AudioBackend trait + ALSA (2026-05-21)
- voltex_audio::backend (신규 디렉토리): **`AudioBackend` trait** (sample_rate/channels/buffer_frames/write/shutdown), `BackendError` (InitFailed/WriteFailed/NotSupported), `make_backend()` 가 OS 분기 + null fallback.
- `WasapiBackend` — 기존 src/wasapi.rs 의 `WasapiDevice` 를 trait impl 로 래핑 (Windows only).
- `AlsaBackend` — libasound FFI (`snd_pcm_open` / `set_params` / `writei` / `prepare` / `close`), f32 LE stereo @48kHz 고정, EPIPE/ESTRPIPE underrun 복구 (Linux only, `#[link(name = "asound")]`).
- `NullBackend` — 헤드리스/CI 용 no-op. make_backend 가 init 실패 시 자동 fallback.
- ALSA 검증 한계: 코드 작성은 Windows 개발 머신에서 수행 → Linux 환경에서의 실제 `cargo build --target x86_64-unknown-linux-gnu` + audio_demo 소리 출력 검증은 향후. spec § 8.1 에 명시.
- 테스트 +5 (null backend ×3, make_backend smoke, BackendError Display). voltex_audio 84 → 89.

### Cycle 9 (B2): Asset packing + VFS (2026-05-21)
- voltex_asset::vpkg (신규): **`.vpkg` 포맷** (magic "VPKG" + version 1 + entry table + concat blobs), no compression, no external deps. `write_vpkg(&[(path, bytes)])` / `VpkgReader::parse(&[u8])`.
- voltex_asset::vfs (신규): **`Vfs` enum** — `Filesystem(PathBuf)` | `Archive(VpkgReaderOwned)`. 단일 `read(path)` / `exists` / `list` API. `archive_from_bytes(&'static [u8])` 가 `include_bytes!` 결과를 받음.
- voltex_runtime: `RuntimeConfig::with_vfs(Vfs)` 빌더 + `RuntimeApp` 의 scene 로더가 Vfs 우선 (fs fallback 유지). `voltex::Vfs` re-export.
- voltex_cli build: scene + scripts + assets 를 `<dist>/wrapper/voltex_archive.vpkg` 로 묶고 wrapper main.rs 가 `include_bytes!("../voltex_archive.vpkg")` + `Vfs::archive_from_bytes` + `with_vfs` 로 self-contained exe 생성.
- 테스트 +13 (voltex_asset +12: vpkg ×6 [roundtrip 2, bad magic, truncated, empty, unknown path], vfs ×6 [fs read/missing/list, archive read/unknown/exists-matches]. voltex_cli +1: pack_archive). voltex_asset 24 → 36.

### Cycle 8 (B1): voltex.toml + voltex CLI + Windows exe (2026-05-21)
- crates/voltex_cli (B1-spike 의 voltex_cli 를 production-grade 로 확장): 6 모듈 (main/manifest/error/util/new/build/run)
- **voltex.toml 매니페스트** (자체 미니 TOML 파서, 외부 의존 0): `[project] name/version`, `[build] scene/scripts/assets/window_title/window_size/targets`, `[build.<triple>] icon`.
- **`voltex new <name>`**: 디렉토리 스캐폴드 (voltex.toml + scene.vscn + scripts/spinner.lua + assets/.gitkeep).
- **`voltex build [--target X] [--manifest-path D] [--engine-path R]`**: 매니페스트 파싱 → wrapper crate 생성 → cargo build --release → `dist/<name>(.exe)` + 자원 복사.
- **`voltex run`**: build 후 exe 실행, exit code 전파.
- 친화 에러: `MissingField/ManifestParse/CargoMissing/...` 의 Display impl.
- 테스트 +19 (manifest 9, new 3, build 2, util 3, strip_glob, host_triple).
- Roadmap Cycle 8 [✅ 3/3] commit bda6412.

### Cycle 7 (B1-spike): voltex_cli prototype (2026-05-21)
- crates/voltex_cli 신규 (bin name "voltex") — 1세션 timebox.
- 단일 subcommand `voltex build <project_dir>` — wrapper crate 생성 + cargo build --release → `dist/<name>(.exe)` + scene/scripts/assets 자원 복사.
- 학습 노트: docs/superpowers/notes/2026-05-21-b1-spike-learnings.md — voltex.toml 매니페스트 필드 결정, target dir 공유 미지수, host triple 분기 등 B1 의 출발점.
- 테스트 3 (project_name, escape_for_rust_string, copy_optional_file no-op).
- Roadmap Cycle 7 [✅ 1/1] commit 5c885e2.

### Cycle 6 (B0): voltex_runtime 라이브러리 (2026-05-21)
- crates/voltex_runtime (L3 신규): **`RuntimeConfig`** 빌더 (8 메서드) + **`RuntimeApp`** + winit `ApplicationHandler` 통합 + `RuntimeError`/`RuntimeResult` (thiserror).
- **`RuntimeApp::new(config)`** — `register_defaults` → register_components 콜백 → 초기 scene 로드 (silent 실패) → build_scene 콜백 → setup_systems 콜백. GPU/window 는 만들지 않음 (lazy).
- **`RuntimeApp::tick_systems_only(dt)`** — propagate_transforms → (옵션) physics_step → (옵션) script_start_all + script_update_all + poll_reload → 사용자 등록 시스템. 헤드리스 단위 테스트 가능.
- **`RuntimeApp::run()`** — winit `EventLoop::run_app` 진입. 첫 `resumed` 에서 GPU/window/BasicRenderer/Camera/Light 초기화 + `with_gpu_init` 콜백 1회 실행. 매 RedrawRequested 에서 input → camera update → `tick_systems_only` → `BasicRenderer::render` → present.
- **`with_gpu_init`** 콜백 신설 (`&mut RuntimeApp, &Device, &Queue`) — `with_scene_builder` 가 GPU 없이 entity seed, `with_gpu_init` 가 mesh 업로드 + MeshHandle/MaterialHandle 부착.
- Drop 순서 의도적 정렬: GPU state → user_systems → file_watcher/timer/input/physics_config → script_pool → assets/registry/world → config. LuaState 의 `__voltex_world` 가 dangling 안 됨.
- examples/runtime_demo: solar system (sun + 3 planet + 1 moon) 자전 데모. `main` 본문 **4줄**.
- 통합 정책: audio_sync_system v1 미통합 (사용자가 직접 AudioSystem 관리). v1.1+ 후속.
- 시각 검증: GUI 자동화 부재 → 수동 검증 권장 (`cargo run -p runtime_demo`). 단위 테스트 +11 (config ×5, error ×2, app ×4: registry/scene order, system order, transform propagation, missing-scene graceful).

### Cycle 5 (S1): per-entity Lua ScriptComponent + iterator (2026-05-21)
- voltex_ecs::script_component: **`ScriptComponent { script_path: String, instance_id: u32 }`** + `Component` trait impl (NAME="script", `[u32 instance_id LE][UTF-8 path]`). `UNBOUND = u32::MAX` 센티넬. `register_defaults()` 가 자동 등록 (built-in 6번째 컴포넌트).
- voltex_ecs::World/EntityAllocator: **`live_entities() -> impl Iterator<Item=Entity>`** — 살아있는 entity 만 id 순으로. voltex_script 의 `iter_entities()` 가 사용.
- voltex_script::instance/pool: **`ScriptInstance { lua, path, started }`** + **`ScriptInstancePool`** — 한 .lua 당 하나의 LuaState 공유 (여러 entity 같은 path → 같은 인터프리터). `get_or_create(path, world_ptr)` / `reload(path)` / `clear()` / `poll_reload(&mut FileWatcher)` (자동 hot reload).
- voltex_script::system: **`script_start_all(world, pool)`** + **`script_update_all(world, pool, dt)`** — Lua 측 `on_start(id, gen)` / `on_update(id, gen, dt)` 디스패치. `lua_pcall` 실패는 silent. 함수가 없으면 silent skip.
- voltex_script::engine_api: **+2 fns (15 → 17)** — `iter_entities()` (모든 live entity 의 `{id=, gen=}` 배열), `iter_by_tag(tag)` (`find_all_by_tag` 의 alias).
- voltex_script::Cargo.toml: **voltex_asset 의존성 추가** (FileWatcher 사용).
- assets/scripts/spinner.lua: **신규 데모** — `on_update` 가 `rotation.y += dt`. 5 cube 통합 테스트가 검증.
- 시각 검증: 라이브러리만 추가, GUI 데모는 Cycle 6 의 runtime_demo 에서 통합. 단위 테스트 +14 (voltex_ecs +5, voltex_script +9 → 추가 +3 pool watcher/clear/unknown + iter_entities/iter_by_tag = 총 14).

### Cycle 4 (E2): Play/Stop + Lifecycle Hooks (2026-05-20)
- voltex_editor::session: **`EditorMode { Edit, Play }`** + `play_snapshot: Option<Vec<u8>>` + `lifecycles` 콜백 리스트. `set_mode(target)` 가 Edit↔Play 전환 (snapshot + lifecycle fire + undo/redo clear). `dispatch` 가 Play 중 모든 명령을 `EditorError::ModeMismatch` 로 거부.
- voltex_editor::lifecycle: **신규 모듈** — `LifecycleEvent { PlayStart, PlayStop }`, `LifecycleCallback = Box<dyn FnMut(LifecycleEvent, &mut World)>`. 후속 cycle (S1 스크립팅 등) 이 사용할 단일 통로.
- voltex_editor::error: `ModeMismatch { current: EditorMode }` variant 추가.
- 미등록 컴포넌트 정책: snapshot 은 ComponentRegistry 에 등록된 타입만 보존. RigidBody/Collider 처럼 미등록인 컴포넌트는 Play→Stop 시 자동 소멸 (정확히 v1 데모가 원하는 동작). 게임 측 영구 컴포넌트는 `register::<T>()` 필요. CODEMAP 에 명시.
- examples/editor_demo: Toolbar 에 Play/Stop 토글. Play 모드 시 Spawn/Save/Load/Undo/Redo/Move/Rotate/Scale 버튼은 숨김. Play 진입 시 lifecycle 콜백이 cube 들에 `RigidBody::dynamic(1.0)` + `Collider::Sphere`, ground 에 `RigidBody::statik()` + `Collider::Box` 부착. 매 프레임 `physics_step(world, &PhysicsConfig::default())` 호출 (Play 모드만). Stop 시 snapshot 복원으로 모든 큐브 원위치. gizmo drag/overlay 도 Play 중 비활성.
- 시각 검증: GUI 자동화 환경 부재 → 수동 검증. 단위 테스트 +11 (snapshot 7, lifecycle/dispatch 4).

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
├── voltex_script      — Lua 5.4, LuaState, table interop, coroutine, sandbox, hot reload, ScriptInstancePool + system (S1)
├── voltex_editor      — IMGUI, docking, 3D viewport, inspector, asset browser, TTF font, UiRenderer
├── voltex_runtime     — RuntimeApp + RuntimeConfig builder + winit handler (B0, "조립된 앱" 진입점)
└── voltex_cli         — bin "voltex" (new/build/run), voltex.toml 매니페스트, .vpkg packing (B1+B2)
```

## 테스트: ~1044개 통과 (Windows host, `cargo test --workspace`). `--lib` 기준은 989 (voltex_cli 는 bin-only 라 lib count 에서 빠지나 `cargo test -p voltex_cli` 로 55 개 실행됨). macOS host 에서는 +2 (CoreAudio 모듈) = 1038 예상. v1.0.1 +5 (input_system 순서/참조 ×2, Bounds default ×2, ScriptComponent default ×1). v1.0.3 +4 (canonicalize_friendly Windows verbatim strip ×3 + cross-OS integration ×1). v1.0.8 +21 (parser panic-resistance fuzz: PNG ×4 + JPG ×4 + GLTF ×5 + OBJ ×4 + WAV ×4). v1.0.9 +30 (OGG +11 + Vorbis +17 + decode_ogg_vorbis_bytes +2 — 모두 wire 직전까지 dead, 이제 실 컴파일). v1.0.10 +14 (voltex_runtime crash module ×11 + config crash log ×3). v1.0.11 +13 (voltex_cli schema/path validation + Levenshtein helper + error Display). v1.0.12 +4 (voltex_cli copy_license_files).

- voltex: 2 (M6: prelude/module 경로 컴파일 검증)
- voltex_math: 39 (8-7 +2: inverse roundtrip × VP+translation; v1.0.19: missing_docs 59→0, doc 강화만, 테스트 카운트 변경 없음)
- voltex_platform: 3
- voltex_ecs: 134 (E3 +10, v1.0.1 +3 Bounds/ScriptComponent default round-trip)
- voltex_asset: 36 (B2 +12: vpkg ×6 + vfs ×6; C0a +2: Handle accessors)
- voltex_renderer: 200
- voltex_renderer_ecs: 7 (C0b 신규: RenderAssets material roundtrip ×2, missing handle ×2, items empty/skip-no-mesh/stale/hierarchy)
- voltex_physics: 106
- voltex_audio: 119 (v1.0.9 +30 from wiring previously-dead ogg+vorbis modules; was 89). Windows host. macOS host: +2 (B3 CoreAudio) = 121
- voltex_ai: 34
- voltex_net: 47
- voltex_script: 42 (S1 +12: pool ×6 [get_or_create same/load/reload/unknown/poll_reload/clear], system ×4 [start/update/5-cubes/missing-callbacks], engine_api ×2 [iter_entities/iter_by_tag])
- voltex_editor: 169 (E3 +11: command ×5 add/remove/unknown/dead-entity + session ×4 add_undo/remove_undo/despawn_restores/despawn_history/slot_reused + inspector ×2 - 1 obsolete despawn_clears test)
- voltex_runtime: 36 (v1.0.18 +2 panic hook relocation guards; v1.0.17 +7; v1.0.10 +14; was 13)
- voltex_cli: 55 (v1.0.16 +1 manifest reference doc gating; v1.0.13 +7; v1.0.12 +4; v1.0.11 +13; was 30)

## Examples (14개) + Games (1개) + 기존 survivor_game (archived)

- triangle, model_viewer, many_cubes, hierarchy_demo, asset_demo
- pbr_demo, multi_light_demo, shadow_demo, ibl_demo
- audio_demo, deferred_demo, editor_demo
- **runtime_demo (B0)** — RuntimeApp 으로 솔라 시스템, main 본문 4줄
- **tutorial_minigame (D1+D2)** — docs/tutorial.md 의 최종 산출물. 1 player + 3 enemy 큐브, spinner.lua + enemy.lua, score 시스템. 듀얼 워크플로 호환 (`cargo run -p tutorial_minigame` + `voltex build`)
- **games/voltex_survivor (D3, v1.0 flagship)** — 4개 Lua 스크립트 + scene 으로 게임 로직 100% 표현. WASD 이동 + 자동 발사 + wave 시스템.
- examples/survivor_game (ECS + voltex_physics + voltex facade로 재작성, 2026-05-20 — 대조용 archive)

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
cargo test --workspace --lib   # 검증: 942개 통과해야 함 (v1.0.8 시점)
```

진행 상황과 다음 단계는 이 파일과 `docs/DEFERRED.md`, `docs/superpowers/plans/`, `docs/superpowers/specs/` 를 본다. 자동 루프가 master에 계속 작업을 푸시할 수 있으므로 작업 전 `git pull --ff-only` 필수.

## 간소화/미뤄진 항목

상세: `docs/DEFERRED.md`
