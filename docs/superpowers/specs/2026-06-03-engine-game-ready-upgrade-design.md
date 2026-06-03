# Voltex 엔진 "게임 제작 가능" 업그레이드 설계 (2026-06-03)

## 목표

엔진의 가장 큰 약점은 "기능은 구현돼 있으나 서로 연결되지 않아 실제 게임을 만들 수 없다"는 점이다.
이 업그레이드는 흩어진 부품을 연결해 엔진을 **실사용 가능(game-ready)** 상태로 끌어올린다.

대상 5개 작업 (우선순위 순):

1. **HUD 런타임 API** — 게임 화면에 텍스트/바/아이콘을 그리는 정식 API
2. **nav_agent_system** — NavAgent 컴포넌트를 실제로 굴리는 ECS 시스템
3. **Lua 입력 API** — 스크립트에서 키보드/마우스 입력 읽기
4. **audio_sync_system** — AudioSource 컴포넌트를 실제 3D 오디오로 재생
5. **스킨드 메시 애니메이션 (Blinn-Phong 한정)** — glTF 스킨/애니메이션을 실제 GPU 스키닝으로 재생

## 비목표 (이번 사이클 제외)

- PBR 셰이더 스키닝 (bind group 0~3이 이미 꽉 차 재배치 리스크 큼 → 후속 사이클)
- 조인트를 ECS 엔티티로 노출하는 것 (피킹/직렬화 오염 회피 → 독립 Skeleton struct 사용)
- Lua에서 입력 *쓰기* (읽기 전용만)
- macOS/Linux 오디오 실 디바이스 검증 (기존대로 v1.1)

## 설계 원칙

- 기존 레이어드 의존성 그래프(L0~L4) 보존. 새 의존은 그래프 방향만 따른다.
- 각 기능은 회귀 테스트와 함께 추가 (프로젝트 TDD 관례). `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` 그린 유지.
- 공개 API 변경 시 해당 crate `CODEMAP.md` 갱신.
- 변경은 단일 책임 단위로 atomic commit.

---

## 1. HUD 런타임 API

### 현재 상태
- `voltex_editor`의 `UiContext` / `DrawList` / `UiRenderer` / `FontAtlas`는 완성된 공개 API.
  좌표계는 픽셀 (0,0=좌상단, Y down). `UiRenderer::render`는 `LoadOp::Load`로 기존 픽셀 위에 오버레이.
- `examples/survivor_game`는 RuntimeApp을 쓰지 않고 직접 winit 루프에서 UiContext/UiRenderer를 수동 호출.
- `RuntimeApp`(`voltex_runtime`)에는 UI가 통합돼 있지 않음. `Hud`는 **타이틀 바 텍스트만** 갱신.

### 설계
`voltex_runtime`이 `voltex_editor`에 의존(L3.5 → L3, 그래프 방향 OK)하여 UI를 통합한다.

- `RuntimeState`에 `ui_context: UiContext`, `ui_renderer: UiRenderer` 추가
  (GPU가 필요하므로 `RuntimeState`에 둔다 — `resumed()`에서 GPU 초기화 직후 생성).
- `RuntimeConfig`에 `enable_hud: bool`(기본 false) + builder `with_hud(true)` 추가.
- `RuntimeApp`에 사용자 HUD 콜백 보관:
  ```rust
  pub type HudFn = Box<dyn FnMut(&mut UiContext, &World)>;
  pub fn set_hud_fn<F: FnMut(&mut UiContext, &World) + 'static>(&mut self, f: F);
  ```
- `handler.rs::redraw()`에서 `BasicRenderer::render()` 직후, 같은 `encoder`로 UI 패스 추가:
  1. `ui_context.begin_frame(mouse_x, mouse_y, mouse_down)` (app.input에서 마우스 상태 전달)
  2. 사용자 `hud_fn(&mut ui_context, &world)` 호출
  3. `ui_context.end_frame()`
  4. `ui_renderer.render(device, queue, &mut encoder, &view, &ui_context.draw_list, w, h)`

### Drop 순서
`RuntimeState` 내부에서 `ui_renderer` → `ui_context` 순으로 둔다(렌더러가 GPU 리소스 보유). GPU(`gpu`)보다 먼저 drop되도록 필드 순서 주의.

### 테스트
- `UiContext`에 텍스트/사각형을 쌓은 뒤 `draw_list.vertices/indices`가 비어있지 않음을 단언하는 단위 테스트.
- 헤드리스: `voltex_capture`에 HUD 오버레이 씬(`scene_hud` 류)을 추가해 텍스트가 픽셀에 찍히는지 PNG로 검증(선택, 시간 허용 시).

### 영향 파일
- `crates/voltex_runtime/Cargo.toml` (voltex_editor 의존 추가)
- `crates/voltex_runtime/src/app.rs` (HudFn 필드 + setter)
- `crates/voltex_runtime/src/config.rs` (enable_hud + with_hud)
- `crates/voltex_runtime/src/handler.rs` (resumed: UI 생성, redraw: UI 패스)
- `crates/voltex_runtime/CODEMAP.md`

---

## 2. nav_agent_system

### 현재 상태
- `NavAgent`(`voltex_ai/src/nav_agent.rs`): `target: Option<Vec3>`, `speed`, `path: Vec<Vec3>`, `current_waypoint`, `reached`.
- `find_path(navmesh, start, goal) -> Option<Vec<Vec3>>`, `follow_path(agent, path, current_waypoint, radius) -> (Vec3, usize)` 등 완비.
- `voltex_ai`는 이미 `voltex_ecs` 의존(L2). 컴포넌트를 굴리는 시스템만 없음.

### 설계
`voltex_ai`에 시스템 함수 추가 (새 파일 `nav_agent_system.rs`):
```rust
pub fn nav_agent_system(world: &mut World, navmesh: &NavMesh, dt: f32) {
    // 1) target 있고 path 비었으면 find_path로 경로 계산 (Vec 수집 후 get_mut)
    // 2) path 있는 에이전트: SteeringAgent로 follow_path → 새 위치 계산
    //    (RigidBody 있으면 velocity 갱신, 없으면 Transform.position 직접 갱신)
    // 3) 마지막 waypoint 도달 시 reached=true
}
```
- 패턴은 기존 `physics_step`/`propagate_transforms`와 동일: `query2::<NavAgent, Transform>()`로 수집 → `get_mut`로 개별 갱신 (borrow 충돌 회피 위해 2-pass).
- NavMesh는 게임/호출자가 소유하고 인자로 주입 (월드에 싱글톤 자원 도입하지 않음 — v1 단순성).

### 테스트
- 직선 NavMesh에서 에이전트가 target 방향으로 이동하고 도달 시 `reached=true`가 되는 회귀 테스트.
- target 없으면 idle(위치 불변).

### 영향 파일
- `crates/voltex_ai/src/nav_agent_system.rs` (신규)
- `crates/voltex_ai/src/lib.rs` (pub mod + re-export)
- `crates/voltex_ai/CODEMAP.md`

---

## 3. Lua 입력 API

### 현재 상태
- `InputState`(`voltex_platform`): `is_key_pressed(KeyCode)`, `is_key_just_pressed`, `mouse_position()->(f64,f64)`, `mouse_delta()`, `is_mouse_button_pressed(MouseButton)`, `mouse_scroll()->f32` 등. KeyCode/MouseButton은 winit 재export.
- `engine_api`(`voltex_script`)는 `__voltex_world` 라이트 유저데이터 글로벌 + `get_world()->Option<&mut World>` 패턴.
- `ScriptInstancePool::get_or_create(path, world)`에서 `register_engine_api(lua, world)` 호출.
- 런타임 루프: 입력 이벤트가 `app.input` 갱신 → `tick_systems_only`에서 스크립트 실행 → 이후 `app.input.begin_frame()`로 delta 클리어. 즉 스크립트 실행 시점에 입력은 최신.

### 설계
`__voltex_world`와 동일한 패턴으로 `__voltex_input` 라이트 유저데이터 글로벌 추가.

- `engine_api`에 `register_engine_input(lua, input: *mut InputState)` + `get_input()->Option<&'static InputState>` (읽기 전용).
- Lua 측 전역 `input` 테이블 제공 (함수형 API):
  - `input.is_key_pressed(name) -> bool`
  - `input.is_key_just_pressed(name) -> bool`
  - `input.mouse_position() -> x, y`
  - `input.mouse_delta() -> dx, dy`
  - `input.is_mouse_button_pressed(button) -> bool` (button: "left"/"right"/"middle")
- **키 이름 매핑 규약**: 문자열 → `KeyCode` 매핑 함수 `keycode_from_str(&str)->Option<KeyCode>` 도입.
  지원: 영문자 "A".."Z", 숫자 "0".."9", "Space", "Enter", "Escape", "Left"/"Right"/"Up"/"Down", "LShift"/"LCtrl" 등 흔한 키. 미지원 이름은 false(silent).
- 비숫자/비문자열 인자는 기존 engine_api처럼 silent no-op(또는 false 반환).

### Lifetime 주의점 (Safety)
- `__voltex_input` 포인터는 스크립트 호출 동안 유효해야 함. `RuntimeApp`이 `input`과 `script_pool`을 모두 소유하고 drop 순서가 pool-before-input이면 안전(World와 동일 계약).
- 매 프레임 input 주소가 안정적이어야 함(`RuntimeApp` 필드라 이동 없음). pool 생성 시 1회 register로 충분.
- input은 읽기 전용 노출(`&InputState`)이라 aliasing UB 없음.

### 배선
- `ScriptInstancePool::get_or_create`(또는 인접 초기화 경로)에 `input: *mut InputState` 파라미터 추가, `register_engine_input` 호출.
- `voltex_runtime`에서 스크립트 풀 초기화 시 `&mut self.input` 포인터 전달.

### 테스트
- `register_engine_input` 후 Lua에서 `input.is_key_pressed("W")`가 InputState 상태를 반영하는 회귀 테스트(테스트용 InputState에 키 강제 set).
- 미지원 키 이름이 false 반환(no panic).
- input 글로벌 미설정 시 graceful(크래시 없음).

### 영향 파일
- `crates/voltex_script/src/engine_api/mod.rs` (register_engine_input, get_input, lua_input_* 함수, keycode_from_str)
- `crates/voltex_script/src/pool.rs` (input 포인터 배선)
- `crates/voltex_script/src/lib.rs` (export)
- `crates/voltex_runtime/src/app.rs` 또는 풀 초기화 호출부
- `crates/voltex_script/CODEMAP.md`

---

## 4. audio_sync_system

### 현재 상태
- `AudioSource`(`voltex_audio/src/audio_source.rs`): `clip_id`, `volume`, `spatial`, `looping`, `playing`, `played_once`.
- `AudioSystem`: `play`, `play_3d`, `set_listener`, `stop`, `set_volume` 등. `SpatialParams`, `Listener`, `compute_spatial_gains` 완비.
- **중요**: `voltex_audio`는 L1(math only) — **ECS 의존 없음(의도적)**. 따라서 ECS 시스템을 voltex_audio에 두면 레이어링 위반.
- `voltex_runtime`은 현재 `voltex_audio`에 의존하지 않으며 `AudioSystem`도 소유하지 않음.

### 설계
통합층인 `voltex_runtime`에 audio를 배선한다(L3.5는 모든 하위 crate 의존 가능).

- `crates/voltex_runtime/Cargo.toml`에 `voltex_audio` 의존 추가.
- `RuntimeApp`이 `AudioSystem`을 소유(`Option<AudioSystem>` — 백엔드 초기화 실패 시 graceful None).
- `RuntimeConfig`에 `enable_audio: bool`(기본 false) + `with_audio(true)`.
- `AudioSource`를 ComponentRegistry에 등록(직렬화 가능하게 — `Component` trait 구현은 voltex_runtime 또는 voltex_audio에 두되, 후자라면 ecs 의존 없이 trait만; 단순화를 위해 **등록 함수를 voltex_runtime에 helper로** 둔다).
- `audio_sync_system(world, audio_system, listener_entity)` (voltex_runtime 내 `audio_sync.rs`):
  1. listener_entity의 Transform → `set_listener(pos, forward, right)` (forward=rot*Z, right=rot*X)
  2. `query2::<AudioSource, Transform>()`: playing이고 아직 시작 안 한 소스는 spatial 여부에 따라 `play_3d`/`play`, 정지 요청은 `stop`.
  3. 1회성(non-looping) 재생 중복 방지는 `played_once` 래치 활용.
- `tick_systems_only`에서 physics 다음, user systems 전에 호출(`enable_audio`일 때).

### 리스너 엔티티
- v1은 `RuntimeApp.audio_listener: Option<Entity>` 필드로 호출자가 지정(보통 카메라/플레이어). 미지정 시 listener 갱신 skip.

### 테스트
- AudioSystem 백엔드 없이도 동작하는 부분(listener 좌표 계산, play/stop 분기 결정 로직)을 순수 함수로 분리해 단위 테스트.
- AudioSource playing 토글 시 올바른 호출(play_3d vs play)이 선택되는지 (mock 또는 결정 로직 함수 단언).
- 실 디바이스 출력은 기존 ignored smoke 관례(F6/F7) 따름.

### 영향 파일
- `crates/voltex_runtime/Cargo.toml`
- `crates/voltex_runtime/src/audio_sync.rs` (신규)
- `crates/voltex_runtime/src/app.rs` (AudioSystem 소유, audio_listener)
- `crates/voltex_runtime/src/config.rs` (enable_audio)
- `crates/voltex_runtime/src/handler.rs` 또는 app.rs tick (시스템 호출)
- `crates/voltex_runtime/CODEMAP.md`

---

## 5. 스킨드 메시 애니메이션 (Blinn-Phong 한정)

### 현재 상태
**있는 것**:
- glTF 파서가 `joints`/`weights`(정점당 4), `GltfSkin{joints, inverse_bind_matrices, skeleton}`, `GltfAnimation{channels, samplers}` 모두 파싱(`voltex_image/src/gltf/`).
- `animation.rs`: `sample_channel`(키프레임 보간, rotation은 SLERP), `AnimationPlayer`(시간 관리), `build_trs`(TRS→Mat4), `mat4_mul`.
- `skinned_vertex.rs`: `SkinnedMeshVertex`(position/normal/uv/tangent + joints[u32;4] + weights[f32;4], wgpu LAYOUT 6 locations) + `BonePalette{matrices:[Mat4;128], set_bone}` — **선언만, 미사용**.
- mesh_shader(Blinn-Phong)는 bind group 0(camera+light), 1(texture)만 사용 → **group 2 비어있음**.

**없는 것**:
- 스키닝 행렬 계산(조인트 로컬 TRS → 계층 누적 월드 → inverse_bind 곱).
- 조인트 계층 보관(현재 glTF node 리스트만 반환, 트리 구성 안 함).
- BonePalette GPU 업로드/바인딩.
- glTF가 joints/weights를 실제 정점 버퍼로 업로드(현재 MeshVertex는 joints/weights 없음).
- 셰이더 스키닝 코드.

### 설계
**조인트 표현: 독립 `Skeleton` struct** (ECS 엔티티 아님).
```rust
pub struct Skeleton {
    pub joint_nodes: Vec<usize>,        // glTF node index per joint
    pub inverse_bind: Vec<Mat4>,        // joint당 IBM
    pub parents: Vec<Option<usize>>,    // joint hierarchy (joint-local index)
    pub local_transforms: Vec<(Vec3, Quat, Vec3)>, // 현재 포즈 TRS (애니메이션이 갱신)
}
impl Skeleton {
    pub fn compute_palette(&self) -> BonePalette; // 계층 누적 → world → * inverse_bind
}
```
- glTF 파서(또는 로더)가 `GltfSkin` + nodes로 `Skeleton`을 구성. parents는 node children 관계를 joint 인덱스 공간으로 역매핑.
- `AnimationPlayer`/`sample_channel`이 채널별 target node의 TRS를 `Skeleton.local_transforms`에 기록.
- `Skeleton::compute_palette()`: 조인트를 topological(부모 먼저) 순회하며 `world_j = world_parent * build_trs(local_j)`, `palette_j = world_j * inverse_bind_j`.

**정점 데이터**: glTF skinned mesh는 `SkinnedMeshVertex`로 업로드(joints u16→u32 확장, weights 정규화). 비스킨 메시는 기존 `MeshVertex` 유지.

**GPU 바인딩**: BonePalette를 uniform buffer로 → mesh_shader **group 2 @binding 0**.
- `mesh_shader.wgsl` VertexInput에 `@location(4) joints: vec4<u32>`, `@location(5) weights: vec4<f32>` 추가.
- vertex stage:
  ```
  skin_mat = w.x*palette[j.x] + w.y*palette[j.y] + w.z*palette[j.z] + w.w*palette[j.w];
  world_pos = camera.model * skin_mat * vec4(position,1);
  world_normal = normalize((camera.model * skin_mat * vec4(normal,0)).xyz);
  ```
- 비스킨 경로와 분리: skinned 전용 파이프라인(`create_skinned_mesh_pipeline`)을 만들어 group 2 본 팔레트 레이아웃 포함. 기존 mesh 파이프라인은 불변.

**좌표계 주의**: glTF IBM은 column-major. 엔진 Mat4도 column-major `[[f32;4];4]`. 로드 시 그대로 매핑 가능하나 회귀 테스트로 `M*M^-1≈I` 및 알려진 포즈 검증.

### 통합/데모
- 새 예제 또는 기존 model_viewer 확장: 애니메이션 있는 glTF(간단한 1~2 본 큐브 또는 합성 스켈레톤)를 로드해 시간에 따라 포즈 변화.
- **시각 검증**: `voltex_capture`에 `scene_skinned` 추가 — t=0과 t=0.5에서 두 PNG 캡처, 픽셀 차이로 "실제로 변형됐음" 단언(blank/동일 프레임 탐지 활용).

### 테스트
- `Skeleton::compute_palette` 단위 테스트: 2-본 체인에서 부모 회전이 자식 팔레트에 누적되는지, identity 포즈에서 palette*bind=I 근사.
- `sample_channel`이 Skeleton.local_transforms를 올바른 인덱스에 기록.
- 헤드리스 GPU(`#[ignore]` 또는 gated): 두 프레임 픽셀 차이 단언.

### 영향 파일
- `crates/voltex_renderer/src/skeleton.rs` (신규: Skeleton)
- `crates/voltex_renderer/src/animation.rs` (Skeleton 갱신 연결)
- `crates/voltex_renderer/src/skinned_vertex.rs` (compute_palette 연동, MAX_BONES 확인)
- `crates/voltex_renderer/src/mesh.rs` (SkinnedMesh 타입/버퍼)
- `crates/voltex_renderer/src/mesh_shader.wgsl` (location 4/5 + group 2 팔레트 + skinning)
- `crates/voltex_renderer/src/pipeline.rs` (create_skinned_mesh_pipeline)
- `crates/voltex_image/src/gltf/` (joints/weights → SkinnedMeshVertex 경로, Skeleton 빌드 helper 또는 데이터 노출)
- `crates/voltex_capture/` (scene_skinned + 두 프레임 비교)
- `crates/voltex_renderer/CODEMAP.md`

### 가장 큰 리스크
1. 조인트 계층 역매핑(node index ↔ joint index)의 정확성 — 단위 테스트로 가드.
2. IBM 좌표계/행렬 곱 순서 — `M*M^-1≈I` + 알려진 포즈 회귀.
3. glTF 로더가 엔진에 메시를 올리는 기존 경로 파악 후 skinned 분기 삽입 (model_viewer 경로 확인 필요).

---

## 실행 순서 (리스크 낮은 것부터, 각 단계 그린 확인 후 commit)

1. nav_agent_system (voltex_ai, 자기완결적, 의존 추가 없음)
2. Lua 입력 API (voltex_script, 기존 패턴 복제)
3. HUD 런타임 API (voltex_runtime → voltex_editor 의존 추가)
4. audio_sync_system (voltex_runtime → voltex_audio 의존 추가)
5. 스킨드 애니메이션 (voltex_renderer/image/capture — 가장 큰 덩어리, 마지막)

각 단계는 독립 commit. 1~4 완료 시점에 엔진은 "게임 제작 가능" 상태. 5는 시각 검증까지 완료 시 캐릭터 애니메이션 데모 확보.

## 성공 기준

- [ ] `cargo build --workspace` 그린
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 0 워닝
- [ ] `cargo test --workspace` 그린 (+ 각 기능 회귀 테스트 신규)
- [ ] HUD: RuntimeApp에서 `set_hud_fn`으로 화면에 텍스트 렌더 (예제 또는 캡처 검증)
- [ ] nav: 에이전트가 경로 따라 이동, 도달 시 reached
- [ ] Lua: 스크립트에서 `input.is_key_pressed(...)` 동작
- [ ] audio: AudioSource playing 토글이 3D 재생/정지로 반영 (결정 로직 테스트)
- [ ] skinned: 시간에 따라 포즈 변화하는 PNG 두 장 픽셀 차이 검증
- [ ] 변경된 crate의 CODEMAP.md 갱신
- [ ] STATUS.md / DEFERRED.md 갱신 (해당 항목 완료 표시)
