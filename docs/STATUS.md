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

## 🎉 v1.0 release — 2026-05-21 (tag `v1.0.0`)

15 cycle / 37 세션 / 약 4 일에 걸쳐 진행된 v1.0 로드맵 완료. 회고는
[docs/v1.0-retrospective.md](v1.0-retrospective.md). 미뤄진 항목은 v1.1+ 후속.

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

## 테스트: 940개 통과 (Windows host, `cargo test --workspace`). `--lib` 기준은 914 (voltex_cli 는 bin-only 라 lib count 에서 빠지나 `cargo test -p voltex_cli` 로 26 개 실행됨). macOS host 에서는 +2 (CoreAudio 모듈) = 942 예상.

- voltex: 2 (M6: prelude/module 경로 컴파일 검증)
- voltex_math: 39 (8-7 +2: inverse roundtrip × VP+translation)
- voltex_platform: 3
- voltex_ecs: 131 (E3 +10: restore_at_generation ×5 + default_bytes/remove ×5)
- voltex_asset: 36 (B2 +12: vpkg ×6 + vfs ×6; C0a +2: Handle accessors)
- voltex_renderer: 200
- voltex_renderer_ecs: 7 (C0b 신규: RenderAssets material roundtrip ×2, missing handle ×2, items empty/skip-no-mesh/stale/hierarchy)
- voltex_physics: 106
- voltex_audio: 89 (Windows host). macOS host: 91 (B3 +2: format constants + ASBD struct size, cfg(target_os = "macos"))
- voltex_ai: 34
- voltex_net: 47
- voltex_script: 42 (S1 +12: pool ×6 [get_or_create same/load/reload/unknown/poll_reload/clear], system ×4 [start/update/5-cubes/missing-callbacks], engine_api ×2 [iter_entities/iter_by_tag])
- voltex_editor: 169 (E3 +11: command ×5 add/remove/unknown/dead-entity + session ×4 add_undo/remove_undo/despawn_restores/despawn_history/slot_reused + inspector ×2 - 1 obsolete despawn_clears test)
- voltex_runtime: 11 (B0 신규: config ×5, error ×2, app ×4 [registry/scene order, system order, transform propagation, missing-scene graceful])
- voltex_cli: 26 (B1-spike +3, B1 +16, B2 +1, B3 +6: validate_target ×5 + cross_compile_error_message)

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
cargo test --workspace --lib   # 검증: 760개 통과해야 함
```

진행 상황과 다음 단계는 이 파일과 `docs/DEFERRED.md`, `docs/superpowers/plans/`, `docs/superpowers/specs/` 를 본다. 자동 루프가 master에 계속 작업을 푸시할 수 있으므로 작업 전 `git pull --ff-only` 필수.

## 간소화/미뤄진 항목

상세: `docs/DEFERRED.md`
