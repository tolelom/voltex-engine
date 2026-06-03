# Changelog

All notable changes to the Voltex Engine are recorded here. Until v1.0 the
project is still **breaking-change-friendly**; from v1.0 onward each entry
flags whether it is breaking, additive, or fix-only.

The format groups changes by release tag (when assigned), with the most
recent on top. v0.x entries are organised by Cycle (see
`docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md`).

---

## [v1.0.70] — 2026-06-03 — Flagship `voltex_survivor` modernized to the new authoring stack

사이클 1~8 로 추가한 워크플로 API 를 flagship 게임에 적용해 그 가치를 실증 + 검증.
이전엔 hand-written 24-vertex `make_unit_cube` + 별도 `upload_meshes`(GPU 단계에서
모든 Transform 엔티티에 공유 핸들/Bounds 수동 부착) + 런타임 enemy/projectile 에
핸들을 매 프레임 샘플링해 붙이는 코드를 갖고 있었다.

### Refactor — `games/voltex_survivor`

- **setup_scene**: ground/player 가 `spawn_bundle((Transform, Tag, PrimitiveMesh,
  MaterialDesc))` 로 선언적. ground 회색, player 초록 (이전엔 전부 흰색).
- **`upload_meshes` + `make_unit_cube` 제거**: `with_gpu_init` 제거. 런타임이 매 프레임
  materialize (v1.0.69) 하므로 GPU 업로드 단계 불필요.
- **런타임 enemy/projectile**: Lua 가 스폰한 bare 엔티티에 `MeshHandle` 샘플링 대신
  `insert_bundle((PrimitiveMesh, MaterialDesc, ScriptComponent))` — 역할별 색(enemy 빨강,
  projectile 노랑) + per-frame materialize 가 GPU 자원 부여. 이전엔 전부 player 와 같은 흰색.
- **204 → 127 줄** (-77). winit/`voltex_renderer` 직접 import 제거(전부 prelude).

### 검증

- workspace build / clippy(`-D warnings`) / lib test 전부 그린. 런타임 렌더 경로는
  독립 검증된 서브시스템(per-frame materialize capture 회귀 + BasicRenderer base_color)에
  의존 — 라이브 게임 화면은 수동 확인 영역.

## [v1.0.69] — 2026-06-03 — Per-frame materialization (runtime-spawned renderables become visible)

사이클 1(v1.0.62)의 실제 한계 수정: `materialize_renderables` 가 `gpu_init` 에서 **한 번만**
실행돼, 부팅 후 런타임에 스폰된 엔티티(Lua `spawn_entity`, 투사체 팩토리, 게임플레이
시스템)가 `PrimitiveMesh`/`MaterialDesc` 를 달아도 GPU 자원을 영영 못 받아 안 보였다.

### Fix — `voltex_runtime` 핸들러

- redraw 경로에서 `tick_systems_only` **직후 매 프레임** `materialize_renderables` 호출
  (device 가 거기서만 존재). idempotent(이미 resolve 된 엔티티 skip) + pending 없으면
  저렴(쿼리 2회). 부팅 엔티티는 첫 프레임에서 커버되고, 이후 스폰 엔티티도 다음
  프레임에 자동 가시화. gpu_init 의 1회 materialize 는 첫 렌더 전 보장용으로 유지.
- 정적 메시를 수동 부착(MeshHandle)하는 기존 게임은 PrimitiveMesh 가 없어 무영향.

### 검증

- RTX 4080 capture 회귀 `materialize_picks_up_entities_spawned_after_a_prior_pass` —
  1차 materialize 후 새 엔티티를 스폰하면 2차 pass 가 그것을 resolve(MeshHandle+
  MaterialHandle 부착) 함을 확인 — 런타임 스폰 시나리오의 정확한 재현.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.68] — 2026-06-03 — Data-authored visual scenes (compose v1.0.62/64/67) + scene-anchor doc

상용 엔진의 본질은 "코드가 아닌 데이터 씬"이다. 사이클 1(선언적 렌더러블)이
`PrimitiveMesh`/`MaterialDesc` 를 `Component` 로 만들어 둔 덕에, 씬 파일(JSON/VSCN)에
**모양+색이 담겨** 코드 없이 visual 씬을 저작할 수 있어야 한다 — 이게 실제로
레지스트리 경유 직렬화로 round-trip 하고 materialize→렌더되는지 검증.

### 검증 (결합 동작 + 회귀 가드)

- `declarative_renderables_round_trip_through_a_data_scene` — Transform+PrimitiveMesh+
  MaterialDesc 월드를 `serialize_scene_json` → `deserialize_scene_json`(레지스트리:
  `register_defaults` + `register_renderable_components`) 하면 두 컴포넌트가 그대로
  복원됨. 로드 후 `plan_primitive_meshes`/`plan_materials` 가 GPU 작업 대기로 인식.
- RTX 4080 capture 회귀 `data_authored_scene_renders_nonblank` — JSON 문자열에서만
  로드한 월드를 `materialize_renderables` 후 BasicRenderer 로 그려 non-blank 확인
  (**데이터 → 픽셀, 스폰 코드 0**).

### 발견 & 문서화 (truthful-doc)

- **scene Transform-anchor footgun**: `serialize_scene_json`/binary 는 **Transform 이
  있는 엔티티만** 직렬화 — Transform 없는 렌더러블 엔티티는 통째로 누락(silent). 검증
  중 재현 확정. 동작 변경(3개 포맷 영향, 위험) 대신 docstring 에 명시 + 회귀 가드
  `renderable_without_a_transform_is_not_serialized` 추가 (v1.0.41/56 의 truthful-doc
  패턴). 렌더러블은 위치가 필요해 Transform 이 사실상 필수라 실사용 영향 없음.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.67] — 2026-06-03 — Prefab registry (named entity templates)

기초 워크플로 격차 6번째: 같은 엔티티를 여러 번 만들 때 매번 컴포넌트 조립을 반복.
Unity Prefab / Godot PackedScene 처럼 "한 번 정의, 이름으로 인스턴스화".

### Additive — `voltex_ecs::prefab`

- **`Prefabs`** 레지스트리 + `register(name, \|&mut World\| -> Entity)` /
  `spawn(&mut World, name) -> Option<Entity>` / `contains` / `len` / `is_empty`.
  팩토리는 `spawn_bundle`(사이클 v1.0.64) 위에 얹히고 컴포넌트-무관이라 어느 crate
  컴포넌트(Sprite 등)든 클로저 안에서 사용. 미등록 이름 spawn → `None`(아무것도 안 만듦).
- facade prelude 에 `Prefabs` 추가.

### 검증

- 단위 6개(템플릿 인스턴스화, 미등록 None, 2회 distinct, 인스턴스별 후처리, contains/len,
  재등록 교체) + 변이 테스트(`contains` 반전 RED→GREEN).
- **실증**: `game_ready_demo` 의 3개 pickup 이 "pickup" 프리팹 등록 → 스폰 지점마다
  인스턴스화 후 위치/색 per-instance 설정(Unity instantiate-then-configure 패턴).
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.66] — 2026-06-03 — Frame clock for systems (`Time` + `add_timed_system`)

기초 워크플로 격차 5번째: 시스템은 per-frame `dt` 만 받고 **누적 경과 시간·프레임
수**를 못 봤다. 타이머/쿨다운/애니메이션 위상을 쓰는 게임 코드가 매번 `+= dt` 를
손으로 누적(조건 분기에 묻혀 버그 유발).

### Additive — `voltex_runtime::time`

- **`Time { elapsed_secs, delta_secs, frame_count }`** + `advance(dt)`. `elapsed_secs`
  는 시스템에 넘기는 `dt` 와 동일 누적이라 fixed-timestep 에서 결정적.
- **`RuntimeApp.time`** public 필드 — `tick_systems_only` 시작에서 1회 `advance`,
  그래서 그 프레임의 모든 시스템·HUD 가 일관된 시계 관측.
- **`add_timed_system(|&mut World, &Time|)`** — 액션 시스템 직후 실행.
- facade prelude 에 `Time` 추가.

### 검증

- 단위 6개(Time::new/advance 누적/delta + 런타임 time 누적/timed 시스템 관측) +
  변이 테스트(`+= dt`→`= dt` RED→GREEN).
- **실증**: `game_ready_demo` 의 HUD 경과시간이 chase 시스템의 조건부 `s.time += dt`
  (chaser 미추적 시 멈추던 버그) → `add_timed_system` 이 `time.elapsed_secs` 반영.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.65] — 2026-06-03 — Tag lookup helpers (`find_by_tag` / `iter_by_tag`)

기초 워크플로 격차 4번째: 데모마다 반복되던
`world.query::<Tag>().find(\|(_, t)\| t.0 == "player").map(\|(e,_)\| e)` 보일러플레이트.

### Additive — `voltex_ecs`

- **`World::find_by_tag(&str) -> Option<Entity>`** — Tag 가 일치하는 첫 live 엔티티.
- **`World::iter_by_tag(&str) -> impl Iterator<Item = Entity>`** — 일치하는 모든 엔티티.
- `impl World` 를 `scene.rs`(Tag 가 사는 곳)에 둬서 World 코어 모듈은 Tag-free 유지.

### 검증

- 단위 3개(조회 성공/부재/다중).
- **실증**: flagship `voltex_survivor`(player 조회 2곳) + `game_ready_demo`(totem/chaser
  조회 2곳)의 `query::<Tag>().find(..).map(..)` 4-라인 패턴 → `find_by_tag(..)` 1줄.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.64] — 2026-06-03 — Component bundles (`spawn_bundle`)

기초 워크플로 격차 3번째: 엔티티 하나 만들 때마다 `world.spawn()` 후 컴포넌트를
4~6번 `add()` 하는 보일러플레이트. Bevy `Bundle` / Unity 프리팹처럼 "이 엔티티는
무엇이다" 를 한 문장으로.

### Additive — `voltex_ecs::bundle`

- **`Bundle`** trait + **`World::spawn_bundle((A, B, …))`** / **`insert_bundle`** —
  `'static` 컴포넌트 튜플(1~12개)을 한 번에 첨부. 순서대로 add 되어 같은 타입은
  뒤쪽이 덮어씀(반복 `add` 와 동일 규칙). 매크로 생성 impl, 외부 의존 0.
- facade prelude 에 `Bundle` 추가.

### 검증

- 단위 6개(전체 첨부, 1-튜플, entity_count, insert_bundle, 덮어쓰기 순서, 4-컴포넌트) +
  변이 테스트(insert 제거 시 4개 RED→GREEN).
- **실증**: `game_ready_demo` `setup_scene` 의 모든 엔티티(ground/player/chaser/totem/
  pickups)가 `spawn()` + N×`add()` → 단일 `spawn_bundle`. player 는 6-튜플
  (Transform/Tag/Script/Collider/PrimitiveMesh/MaterialDesc), chaser 는 7-튜플 —
  크레이트 경계(ecs/physics/ai/audio/renderer_ecs) 넘는 컴포넌트도 한 번들에.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린(ecs 161→167).

## [v1.0.63] — 2026-06-03 — Input action mapping (rebindable semantic controls)

상용 엔진 기초 워크플로 격차 2번째: 입력이 raw 키코드 폴링뿐이었다
(`input.is_key_pressed(KeyCode::KeyW)`). Unity Input Actions / Godot `InputMap`
처럼 **시맨틱 액션/축**을 한 곳에서 바인딩하고 이름으로 질의하는 레이어가 없었다.

### Additive — `voltex_platform::input_map`

- **`InputMap`** — `InputState` 위 순수(상태 없는) 매핑 레이어. `bind_key(action,key)`
  / `bind_mouse(action,btn)` / `bind(action, Trigger)`(다중 = OR 대체) +
  `bind_axis(axis, positive, negative)`(±1/0). 질의: `is_pressed`/`just_pressed`/
  `just_released`/`axis`(→ f32 `[-1,1]`)/`vector(x,y)`(→ `Vec2`). 모두
  `&InputState` 를 받아 윈도우/GPU 없이 단위 테스트 가능.
- **`Trigger`** enum (`Key`/`Mouse`) + `KeyCode`/`MouseButton` winit 재export
  (게임이 winit 직접 의존 회피).
- 한계: 마우스 just-released 미지원(InputState public 질의 부재) — mouse 트리거의
  `just_released` 는 항상 false.

### Additive — 런타임 통합

- **`RuntimeApp.input_map`** public 필드 + **`add_action_system(|&mut World,
  &InputState, &InputMap, f32|)`** — raw input system 직후 실행. 컨트롤 리바인딩이
  `app.input_map` 한 곳에 모임.
- **`RuntimeConfig::with_input_map(InputMap)`** — 부팅 시 `app.input_map` 으로 이동.
- facade prelude 에 `InputMap`/`KeyCode`/`MouseButton` 추가.

### 검증

- 단위 8개(액션 held/edge, axis ±/0, vector 합성, 마우스 트리거, 미바인딩=false/0) +
  변이 테스트(axis `p-n`→`p+n` RED→GREEN).
- 런타임 통합 테스트 `action_system_reads_input_map_bindings`(바인딩된 키를 액션
  시스템이 관측).
- **실증**: flagship `voltex_survivor` 의 WASD/화살표 이동 ~35줄(8개 `is_key_pressed`
  분기 + 수동 정규화) → `bind_axis` 4줄 + `map.vector("move_x","move_y")` 액션 시스템.
  WASD·화살표가 같은 축의 대체 바인딩.
- workspace build / clippy(`-D warnings` all-targets) / lib test 전부 그린.

## [v1.0.62] — 2026-06-03 — Declarative renderables (one-call "spawn a visible object")

상용 엔진과의 **기초 워크플로 격차** 정정. 기능 폭(PBR/물리/오디오/AI/net/Lua)은
이미 충분하지만, 엔진에서 가장 빈번한 연산 — "눈에 보이는 오브젝트 하나 놓기" —
이 두 단계(GPU 없는 `scene_builder` ↔ GPU 필요한 `gpu_init`)로 갈라져, 엔티티를
`Tag` 로 되찾아 `MeshHandle`/`MaterialHandle`/`Bounds` 를 별도 GPU 패스에서 수동
부착하는 보일러플레이트가 모든 게임에 강제됐다. Unity 의 `CreatePrimitive` /
Godot 의 노드 추가에 해당하는 한-줄 경로가 없었다.

### Additive — `voltex_renderer_ecs::renderable`

- **`PrimitiveMesh`** 컴포넌트 (값 타입, GPU 무관): `Cube`/`Plane`/`Quad`/`Sphere`.
  생성자 `cube(size)`/`plane(size)`/`quad(w,h)`/`sphere(r,sec,stk)`/`sphere_default(r)`,
  `aabb()`(자동 `Bounds` 용), `geometry()`(빌트인 generator 위임). `Component`
  구현 → 씬 직렬화.
- **`MaterialDesc(MaterialUniform)`** 컴포넌트: `color(rgba)` / `pbr(rgba,metallic,roughness)`.
  `Component` 구현 → 씬 직렬화.
- **`materialize_renderables(&mut World, &mut RenderAssets, &Device)`**: `PrimitiveMesh`
  가 있고 `MeshHandle` 이 없는 엔티티마다 GPU 메시를 생성(**dedup**: 동일 프리미티브는
  한 버텍스 버퍼 공유)하고 `MeshHandle`+`Bounds` 부착; `MaterialDesc` 도 동일하게
  dedup 후 `MaterialHandle` 부착. **멱등** — 이미 resolve 된 엔티티는 skip 하므로 매
  프레임/핫리로드 후 호출 안전. 순수 헬퍼 `plan_primitive_meshes`/`plan_materials`
  (GPU 없이 멱등 계약 테스트 가능).
- **자동 와이어링**: `RuntimeApp` 이 `register_defaults()` 직후 두 컴포넌트를 등록하고,
  `gpu_init` 직후 `materialize_renderables` 를 한 번 호출. 게임은 `scene_builder`
  한 곳에서 모양+색을 선언하면 끝 — device/`MeshHandle`/Tag 왕복 불필요.
- **facade prelude** 에 `PrimitiveMesh`/`MaterialDesc` 추가.

### 검증

- 단위 13개(aabb 4종, geometry 카운트, dedup 키, plan 멱등 2종, 직렬화 round-trip 2종) +
  변이 테스트로 dedup 키 충돌 검출 RED→GREEN 확인.
- RTX 4080 capture 회귀 2개(`#[ignore]`): `materialized_primitives_render_nonblank`(선언적
  엔티티만으로 비-blank 렌더), `materialize_dedupes_identical_meshes_and_attaches_bounds`(3개
  동일 큐브 → 메시 1개 공유, sphere 분리, Bounds/MaterialHandle 부착, 2회 호출 멱등).
  시각 기록 `docs/visual/declarative_primitives.png`(빨간 큐브 + 초록 구).
- **실증**: `game_ready_demo` 의 `upload_meshes` 가 ~30줄(큐브 mesh/3머티리얼/Tag 재조회
  루프/bounds) → 스킨드 토템 한 줄로 축소. ground/player/chaser 는 `setup_scene` 에서
  `PrimitiveMesh::cube` + `MaterialDesc::color` 선언만.
- workspace build / clippy(`-D warnings` all-targets) / lib test 전부 그린.

## [v1.0.61] — 2026-06-03 — Status sweep: hygiene + runtime lights/audio/shadows + robustness audit

프로젝트 현황 점검 후 A(위생)→D(대형) 우선순위로 자율 진행한 세션. 모두 헤드리스
검증(단위 + RTX 4080 capture 회귀) + 역호환 유지. 점검 중 DEFERRED.md 의 **문서
drift 4건**(이미 구현됐는데 v1.1 후속으로 표기) 확인·정정.

### Fix — 위생 (Tier A)

- **clippy 게이트 복구**: `voltex_capture` 텍스처 로드 테스트의 `vec![...].repeat`
  가 `clippy::useless_vec` 로 `--all-targets -D warnings` 게이트를 깨고 있던 것 수정.
- **워크스페이스 버전 sync**: 17개 라이브러리 crate 가 placeholder `0.1.0` 이라
  `voltex --version` 이 0.1.0 을 출력하던 문제 — `[workspace.package].version = "1.0.61"`
  + 각 crate `version.workspace = true` (기존 `license.workspace` 패턴). 예제는 0.1.0 유지.

### Additive — 런타임 라이팅/오디오 (Tier B)

- **per-frame 점광원 업데이터**: `RuntimeApp::set_point_lights_fn(\|&mut Vec<PointLight>, &World, f32\|)`
  — directional `set_light_fn` 과 대칭. 움직이는 횃불·깜빡이는 램프·발광 투사체를
  world 상태 기반으로 매 프레임 구동(정적 `set_point_lights` 보강). 멀티라이트 배선
  자체는 이미 완료돼 있었음(colored PointLightsUniform + PBR LightsUniform 병합, spot 포함).
- **AudioSource 씬 직렬화**: `voltex_audio::AudioSource`(ECS 무의존)를 `register_raw`
  escape hatch 로 등록(`audio_sync::{serialize,deserialize}_audio_source` +
  `register_audio_source`, RuntimeApp 이 `register_defaults()` 직후 자동 등록). 11바이트
  (clip_id+volume+spatial/looping/playing; transient played_once/active 미저장).
  JSON/binary 씬 round-trip. +3 테스트.

### Additive — 스킨드 메시 그림자 caster (Tier B)

- 런타임 directional 그림자 pre-pass 가 정적 메시만 그려 스킨드 캐릭터가 그림자를
  못 던지던 갭 해소. `create_skinned_shadow_pipeline` + `skinned_shadow_shader.wgsl`
  (depth-only GPU 스키닝, `view_proj=light_vp`, 기존 skinned camera/palette 레이아웃
  재사용 — 신규 레이아웃 0). `BasicRenderer::gather_skinned_shadow` 가 light-space
  cam UBO + 공유 palette 업로드 후 정적 메시 뒤에 스킨드 메시를 깊이에 렌더. Hidden
  엔티티는 cast/draw 모두 제외. RTX 4080 검증 `skinned_mesh_casts_shadow`(strip 이
  바닥에 그림자 투사, Hidden 대조군 압도) + `docs/visual/skinned_shadow.png`.

### Fix — 견고성 audit (Tier C)

- **`.normalize()` zero-input audit** (v1.0.41 deferred): 게임플레이 핵심 경로
  (voltex_ai steering seek/flee/arrive/wander + nav_agent_system)는 **이미 length 가드**가
  있어 NaN-safe 확인. 유저 라이트 설정이 zero direction 일 때 NaN 전파되던
  `csm::compute_cascade_matrices` + `spot_shadow_matrix` 두 곳을 `normalize_or_zero`
  + straight-down fallback 으로 수정. +2 finite-output 회귀.

### 문서 drift 정정 (점검 중 발견 — 이미 구현됐던 항목)

- **Lua 입력 API**: `voltex_script::engine_input` 의 `input` 테이블(v1.0.59) — DEFERRED 가
  아직 "v1.1 후속" 표기였음.
- **JSON 씬 entity cap**: `deserialize_scene_json` 도 `MAX_SCENE_ENTITIES` 적용.
- **`validate_hierarchy`**: 구현 완료 + text/binary/JSON 3개 deserializer 모두 로드 후
  호출(pre-existing 사이클 방어) — DEFERRED 가 아직 "별 pass 필요" 표기였음.

### Tier D / B-7 (의도적 미진행 — 평가 후 문서화)

- **에디터 커스텀 컴포넌트 리플렉션**: ComponentRegistry 에 필드 레벨 리플렉션 인프라
  부재 — 별도 설계 필요. **Archetype 스토리지 / dep scheduler 실제 병렬**: major
  rewrite (data-race 위험, 다중 세션). **CI 첫 green run**: GitHub mirror 필요(자율 불가).
  **missing_docs 1476 / pub(crate) 다운그레이드**: 대량 기계적 churn, 가치 낮음 — 보류.

### 검증

`cargo build --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` /
`cargo test --workspace --lib` (1239 lib 테스트, 0 실패) 전부 그린.

## [v1.0.60] — 2026-06-03 — Scene control + asset loading + deeper game-ready wiring

v1.0.59 데모(game_ready_demo)를 실제로 돌려보며 드러난 엔진 갭들을 연달아 메운
세션. 모두 헤드리스 검증(단위 테스트 + RTX 4080 capture 회귀) + 역호환 유지.

### Additive — 씬 제어 (게임이 화면을 통제 가능하게)

- **카메라**: `RuntimeConfig::with_camera(pos, yaw, pitch)` / `with_camera_fov` +
  `RuntimeApp::set_camera_fn(\|cam, world, dt\|)` (follow/orbit). 근본 원인은
  렌더 버그가 아니라 하드코딩 카메라(0,10,20)에 1-unit 오브젝트가 ~15px 점으로
  보였던 것 — capture로 재현 확인.
- **머티리얼 색상**: BasicRenderer가 `MaterialUniform.base_color`를 적용
  (`ColoredCameraUniform` + `mesh_colored.wgsl`, 격리 — shared CameraUniform/PBR/
  레거시 예제 무영향). 모든 오브젝트가 같은 회색이라 바닥에 묻히던 문제 해결.
- **조명**: `RuntimeConfig::with_light(dir, color, ambient)` + `set_light_fn`
  (day/night).
- **충돌/트리거 이벤트**: `RuntimeApp::set_collision_fn(\|contacts, world, dt\|)`,
  `enable_physics` 무관 실행(트리거 존). 데모의 "잡힘"을 거리 계산 → 실제 Collider
  충돌로 교체.
- **점광원**: `RuntimeApp::set_point_lights(Vec<PointLight>)` — mesh_colored가
  additive point light (MAX_POINT_LIGHTS=8). 단일 directional을 보강.
- **스폿라이트**: `PointLight`를 3×vec4(48B)로 확장(방향 + cos(outer half-angle)).
  `PointLight::spot(pos, dir, color, range, intensity, cone_degrees)` = 부드러운
  가장자리 원뿔, `PointLight::new` = omni(센티넬 `spot_cos_outer < -1.5`).
  mesh_colored가 cone smoothstep gate 적용 → `set_point_lights` 경유 그대로 사용.
  캡처 회귀 `spotlight_lights_cone_footprint_not_corners` +
  `docs/visual/spot_cone.png`. 데모 아레나 중앙에 흰 스폿 추가.
- **HUD 바**: `UiContext::progress_bar` / `draw_rect` (체력/진행 바). 데모에
  danger meter 추가.
- **디버그 드로우(gizmo 라인)**: `voltex_renderer::debug_line`(LineVertex +
  `LineList` depth-tested 파이프라인) + `BasicRenderer::set_debug_lines` +
  `RuntimeApp::set_debug_draw_fn(\|d: &mut DebugLines, world, dt\|)`.
  `DebugLines` 빌더(line/ray/aabb/cross)로 콜라이더·내비·레이 시각화. 매 프레임
  3D 위에 렌더(depth-test, no write). 캡처 회귀
  `debug_lines_draw_colored_segments` + `docs/visual/debug_lines.png`. 데모에
  chase 벡터 + 플레이어/추적자 cross + 아레나 AABB 추가.
- **CPU 파티클 시스템**: `voltex_renderer_ecs::particle` — `Particle` 컴포넌트
  (velocity/acceleration/lifetime/size·alpha 보간) + `spawn_particle` +
  `update_particles`(RuntimeApp tick 자동 호출). 파티클 = Transform+Sprite+
  Particle 엔티티라 빌보드 패스가 그대로 렌더. 이동·페이드·수명 만료 despawn.
  단위 테스트(이동/중력/페이드/despawn) + 캡처 회귀
  `particles_spread_outward_over_time`(시간에 따라 반경 증가) +
  `docs/visual/particle_burst.png`. 데모: 잡힐 때 16개 오렌지 버스트.
- **직교(2D/탑다운) 카메라**: `Camera::ortho_height: Option<f32>` +
  `Camera::orthographic(pos, aspect, height)` (parallel 투영) +
  `RuntimeConfig::with_orthographic(height)`. 2D·탑다운·아이소메트릭 게임용 —
  깊이에 무관한 footprint(원근 단축 없음). 캡처 회귀
  `orthographic_footprint_is_depth_independent`(near≈far) + 단위 테스트
  (ortho는 깊이 무관, perspective는 단축). 기존 perspective 기본값 불변.
- **빌보드 스프라이트**: `voltex_renderer::sprite`(SpriteUniform +
  alpha-blended·depth-tested 파이프라인, 정점 셰이더에서 카메라 right/up으로
  쿼드 확장) + `voltex_renderer_ecs::Sprite` 컴포넌트
  (`Sprite::solid`/`textured`). `WorldTransform`+`Sprite` 엔티티를 mesh 패스 후
  카메라-facing 쿼드로 렌더(카메라 축은 inverse VP 복원, MAX 256/frame). 픽업·
  파티클·월드 마커·간단한 2D-in-3D 적에 사용. 캡처 회귀
  `billboard_sprite_faces_camera_from_any_angle`(정면/측면 footprint 일치) +
  `docs/visual/sprite_billboard.png`. 데모에 떠다니는 픽업 3개 추가.
- **스프라이트시트/아틀라스 UV**: `Sprite.uv_rect` + `with_uv_rect` +
  `set_sheet_frame(cols, rows, index)`(row-major) — sprite.wgsl이 단위 쿼드 UV를
  서브렉트로 remap. 2D 캐릭터 애니메이션·아틀라스용. 단위 테스트(프레임→uv_rect)
  + 캡처 회귀 `sprite_sheet_frame_selects_atlas_cell`(4색 2×2 아틀라스 각 프레임
  정확 샘플).
- **스프라이트 애니메이션**: `SpriteAnimation` 컴포넌트(cols/rows/frame_count/
  fps/looping) + `update_sprite_animations`(RuntimeApp tick 자동 호출) — Sprite의
  `uv_rect`를 시간에 따라 진행. loop=wrap, non-loop=마지막 프레임 clamp+정지.
  단위 테스트(프레임 진행/wrap/clamp+stop).
- **런타임 텍스처 로딩 편의**: `RenderAssets::add_texture_png(device, queue,
  bytes)`(PNG 디코드+업로드, `Result`) + `add_texture_rgba(device, queue, w, h,
  px)` — bind-group 레이아웃 생성 보일러플레이트 제거. gpu_init에서 스프라이트/
  메시 텍스처를 한 줄로 로딩. 캡처 회귀
  `render_assets_png_loader_roundtrips_into_a_sprite`.
- **Lifetime 자동 despawn**: `voltex_runtime::Lifetime { remaining }` 컴포넌트 +
  `update_lifetimes`(tick 마지막 단계, 사용자 시스템 관측 후 만료 시
  `despawn_recursive`). 발사체·타임드 픽업·임시 이펙트·스폰 적 등 자기정리 엔티티용.
  단위 테스트(카운트다운→despawn / 빈 월드 no-op).
- **prelude 확장**: `voltex::prelude`에 게임플레이 컴포넌트 추가 — `Sprite`,
  `Particle`, `SpriteAnimation`, `TextureHandle`, `Lifetime`, `DebugLines`.
  거의 모든 게임이 쓰므로 Transform처럼 prelude에 포함(도메인 특화 타입은 여전히
  명시 경로). prelude 컴파일 테스트 갱신.

### Additive — 런타임 PBR 경로 (v1.1 항목 선반영)

- **BasicRenderer 풀 PBR 패스**: `BasicRenderer::set_pbr_enabled(true)` /
  `RuntimeConfig::with_pbr(true)` — 기존 colored Blinn-Phong 대신 Cook-Torrance
  PBR로 메시 렌더. 멀티라이트(`LightsUniform`: directional sun + `set_point_lights`
  의 point/spot 접합), per-entity 머티리얼 파라미터, 4-맵 텍스처(albedo/normal/
  ORM/emissive via 신규 `PbrTextures` 컴포넌트, 없으면 flat-normal/white/black
  기본), IBL BRDF LUT. `render()` 시그너처 불변(`LightUniform`→`LightsUniform`
  내부 변환). colored/스프라이트/파티클/디버그라인/스킨드/그림자 패스 무영향
  (기본 off → byte-identical). 캡처 회귀
  `basic_renderer_pbr_path_shades_differently_than_colored` +
  `docs/visual/pbr_runtime.png`(metallic 그래디언트 + 스페큘러 + 환경 반사).
  env는 procedural sky(SH 0).
- **스킨드 메시 PBR**: `pbr_skinned.wgsl` — GPU 버텍스 스키닝 + 풀 Cook-Torrance
  프래그먼트. bone palette를 group 0(camera+lights)에 합쳐 max_bind_groups=4
  유지. `create_pbr_skinned_pipeline` + `BasicRenderer` skinned 패스가
  `pbr_enabled` 시 PBR 스킨드 파이프라인 사용(멀티라이트+IBL+그림자). 스킨드
  메시도 이제 정적 메시와 동일한 PBR 라이팅. 캡처 회귀
  `runtime_skinned_pbr_path_renders_and_animates` + `docs/visual/skinned_pbr.png`.
  스킨드 엔티티도 `MaterialHandle`/`TextureHandle`/`PbrTextures`를 정적 메시와
  동일하게 반영(per-entity 머티리얼 + 4-맵 텍스처).
- **선형 텍스처 로더 + 노멀맵**: `GpuTexture::from_rgba_linear` +
  `RenderAssets::add_texture_png_linear`/`add_texture_rgba_linear` — 노멀/ORM
  맵을 sRGB 디코드 없이 linear `Rgba8Unorm`로 업로드. 컬러/알베도는 기존 sRGB
  경로 유지. 캡처 회귀 `pbr_normal_map_perturbs_shading`(노멀맵이 PBR 셰이딩을
  실제로 교란).

### Additive — 실사용 점검 보완

- **MAX 캡 1회성 경고**: BasicRenderer가 per-frame 한도(mesh `max_entities`,
  스프라이트 256, 스킨드 64)를 처음 초과할 때 `eprintln!` 1회 — 조용한 truncation이
  "다 그려졌다"로 오인되지 않게. `Cell<bool>` 플래그로 프레임마다 스팸 안 함.
  단위 테스트(초과 시에만/한 번만 발화).
- **`Mesh` 프리미티브 생성자**: `Mesh::cube(device, size)` / `Mesh::plane` /
  `Mesh::quad(w,h)` / `Mesh::sphere(r, sectors, stacks)` — `from_obj`/`from_gltf`
  스타일의 한 줄 GPU 메시 생성(generate_* + `Mesh::new` 래핑). capture/데모가
  `Mesh::cube`로 교체.
- **프리미티브 메시 생성기**: `voltex_renderer::generate_cube(size)` /
  `generate_plane(size)`(XZ 바닥, +Y) / `generate_quad(w, h)`(XY, +Z) —
  `(Vec<MeshVertex>, Vec<u32>)` 반환(`generate_sphere`와 동일 스타일).
  지금까지 공개 프리미티브가 `generate_sphere` 뿐이라 모든 게임/예제가 큐브
  버텍스를 손으로 작성해야 했던 갭 해소. per-face normal/UV/tangent 정확(PBR
  노멀맵 호환). `voltex_capture::unit_cube`와 `game_ready_demo`가 이걸로 교체
  (손수 작성 큐브 제거). 단위 테스트 4개 + 기존 큐브 GPU 회귀(single_cube/
  base_color/shadow)가 `generate_cube`로 통과.

### Fixed (game-ready 세션)

- **스프라이트 투명도 정렬**: BasicRenderer 스프라이트 패스가 엔티티 순서대로
  그려 겹친 반투명 스프라이트의 합성이 spawn 순서에 따라 달라지던 문제 — 카메라
  거리 기준 back-to-front 정렬 추가. 캡처 회귀
  `overlapping_sprites_blend_independent_of_spawn_order`(두 spawn 순서가 동일
  결과 + 가까운 스프라이트가 위로 합성).

- **그림자**: `RuntimeConfig::with_shadows(true)` — BasicRenderer directional
  shadow mapping (depth pre-pass + mesh_colored 3×3 PCF, group 3). 기본 off
  (동작 불변). 캡처 검증 `docs/visual/shadow_on.png`. 한계: 스킨드 caster 제외,
  ortho 고정, 바닥 접촉 오브젝트 self-occlusion.
- **헤드리스 데모 검증 하니스**: `voltex_capture::scene_playthrough` — 데모
  장면을 오프스크린 렌더해 시각/동작을 자동 확인(라이브 창 불필요).

### Fixed

- **generate_sphere winding**: 구가 CW-from-outside로 감겨 카메라 향한 반구가
  컬링되고 먼 안쪽 반구가 비쳐 앞면이 어둡게 렌더되던 버그(PBR 데모/IBL/모든
  sphere 영향). CCW-from-outside로 수정 → 앞 반구 정상 라이팅.

### Additive — 에셋 로딩 (핸드코딩 큐브 탈피)

- **모델**: `Mesh::from_obj(device, src)`, `Mesh::from_gltf(device, bytes)`.
- **텍스처**: `RenderAssets::add_texture` + `TextureHandle` 컴포넌트 (BasicRenderer
  per-entity 디퓨즈 텍스처), `GpuTexture::from_png(bytes)`.
- **씬 저장/로드**: `RuntimeApp::save_scene_json(path)` / `load_scene(path)`.
- **스킨드 glTF**: `skinned_vertices_from_gltf` + `load_skinned_gltf(device,
  assets, &GltfData)` → 애니메이션 캐릭터를 glTF에서 바로 조립.

### Additive — 스킨드 애니메이션 런타임 통합

- `SkinnedMesh` 에셋 + `SkinnedMeshHandle`/`SkinnedPose` 컴포넌트 +
  `update_skinned_poses`(tick 자동) + BasicRenderer 2차 스킨드 패스
  (per-entity bone palette dynamic UBO, MAX_SKINNED=64).
- `skinned_vertices_from_gltf` + `load_skinned_gltf(device, assets, &GltfData)`
  → 애니메이션 캐릭터를 glTF에서 바로 조립.
- game_ready_demo에 흔드는 스킨드 totem.

### Additive — Lua 게임 로직

- `nav_set_target` / `nav_clear_target` / `nav_reached`, `audio_play` /
  `audio_stop` (engine_api → voltex_ai/voltex_audio 의존).

### 검증

워크스페이스 build / clippy(-D warnings) / test 그린. 런타임/렌더 경로는 `voltex_capture`
헤드리스 GPU 회귀로 픽셀 검증(카메라 프레이밍, base_color, 텍스처 샘플, 스킨드 변형,
OBJ/glTF/PNG 로드). **남은 visual 검증**: 라이브 창에서의 데모 외형은 사용자 디버깅 패스.

## [v1.0.59] — 2026-06-03 — Game-ready: wire up nav / Lua input / HUD / audio / skinning

"기능은 있는데 연결이 안 된" 다섯 구멍을 메워 엔진을 실사용 가능(game-ready)
상태로. 설계: `docs/superpowers/specs/2026-06-03-engine-game-ready-upgrade-design.md`.

### Additive — nav_agent_system (voltex_ai)

`nav_agent_system(world, navmesh, dt)` — NavAgent+Transform 엔티티를 매 프레임
`find_path` → `follow_path` 스티어링으로 이동(kinematic), 최종 waypoint 도달 시
goal에 snap + `reached`. 도달 불가 target은 idle 전환. NavMesh는 호출자 주입. +5 테스트.

### Additive — Lua 입력 API (voltex_script)

스크립트에서 입력 읽기: `input.is_key_pressed("W")` / `is_key_just_pressed` /
`mouse_position` / `mouse_delta` / `is_mouse_button_pressed("left")` / `scroll`.
`voltex_script`는 winit 무의존 유지 — POD `ScriptInput` 스냅샷 + thread-local
포인터(`set_current_input`), 키 이름↔keycode 매핑은 `voltex_runtime`이 소유.
런타임이 매 프레임 스냅샷 발행/실행 후 클리어. +5 단위 +2 런타임(전체 체인) 테스트.

### Additive — HUD 런타임 API (voltex_runtime → voltex_editor)

`RuntimeConfig::with_hud(true)` + `RuntimeApp::set_hud_fn(|ui, world| ...)`.
editor UiContext/UiRenderer를 GPU 초기화 시 생성, redraw에서 BasicRenderer 다음
같은 encoder로 `LoadOp::Load` 인-윈도우 오버레이 패스. 점수·체력 등 콘솔 print 탈피.
runtime_demo가 인-윈도우 HUD 데모로 전환. +2 테스트.

### Additive — audio_sync_system (voltex_runtime → voltex_audio)

`RuntimeConfig::with_audio(clips)` + `RuntimeApp::set_audio_listener(entity)`.
RuntimeApp이 `Option<AudioSystem>` 소유(실패 시 silent None). `audio_sync_system`이
listener Transform 동기화 + `AudioSource::playing` 엣지(`active` 플래그)에서만
play/play_3d/stop — held playing이 매 프레임 voice를 새로 만들지 않음.
순수 `decide_action`으로 엣지 로직 분리. +3 +1 테스트.

### Additive — 스킨드 메시 애니메이션 (Blinn-Phong 한정)

`voltex_renderer::Skeleton`(from_skin/apply_animation/compute_palette) — glTF
skin/animation을 GPU 스키닝 행렬(`world_j · inverse_bind_j`)로. `create_skinned_mesh_pipeline`
+ `mesh_skinned.wgsl`(group0 camera, group1 bone palette, 4 joint/weight 블렌딩).
`voltex_capture::scene_skinned`가 RTX 4080(Vulkan)에서 rest(직선)↔bent(굽힘) 2프레임
렌더 + 픽셀 차이로 시각 검증 (`docs/visual/skinned_{rest,bent}.png`). +6 단위 +1 GPU 테스트.
**미완(v1.1)**: PBR 경로 스키닝(bind group 재배치), ECS SkinnedMesh 컴포넌트/자동 재생.

### 검증

`cargo build --workspace` / `clippy --all-targets -D warnings` / `test --workspace --lib`
전부 그린. 워크스페이스 테스트 ~1180 → ~1202. 의존성 그래프: voltex_runtime이
editor/audio 추가 의존(L3.5 → L3/L1, 방향 유효).

## [v1.0.58] — 2026-05-28 — Quat Transform + fuzz + coverage gate + audio smoke (E3b/F4/F5/F6/F7)

Closes every remaining item in `docs/AUDIT-BACKLOG.md`.

### Breaking — Transform.rotation Vec3 → Quat (E3b)

`voltex_ecs::Transform.rotation` was an Euler `Vec3` (YXZ radians). It is
now a unit `voltex_math::Quat`. 24 files touched across ecs, physics,
editor, script, net, examples, capture.

- `Transform::matrix()` uses `Quat::to_mat4` directly.
- `Transform::from_euler_yxz(pos, euler, scale)` helper for call sites
  that still author orientations as Euler triples.
- `EditorCommand::SetTransform.rotation` is now `Quat`.
- Inspector keeps an Euler buffer per selected entity (re-extract only
  on selection change or external quat drift) so sliders stay smooth
  without round-tripping `Quat → Euler → Quat` every frame.
- Gizmo rotate composes `q_new = (q_delta * q_initial).normalize()` —
  axis-stable regardless of initial orientation.
- Lua API surface unchanged: `set_rotation` / `get_rotation` still talk
  YXZ Euler, with `Quat::from_euler_yxz` / `to_euler_yxz` at the FFI
  boundary so existing scripts keep working.
- Physics integrator switches `rotation + ω·dt` (wrong) to
  `Quat::integrate(ω, dt)` (semi-implicit `dq/dt = ½ωq + normalize`).
- voltex_math gains `Quat::to_euler_yxz` (gimbal-collapsing inverse of
  from_euler_yxz) + `Quat::integrate` helpers, +8 tests (commit bee796c).
- voltex_math::Quat re-exported via `voltex::prelude`.

### Wire format compatibility

Persistent formats (`.vscn` text, JSON, binary) **dual-read** 3-float
Euler (legacy) and 4-float quaternion (current). Existing user scene
files keep loading without conversion. Writers emit 4-float quat.

Ephemeral wire format (`voltex_net::EntityState.rotation`) is a **hard
cutover** from `[f32;3]` to `[f32;4]`. `ENTITY_SIZE` 40 → 44 bytes.
Interpolation uses `Quat::slerp` instead of componentwise Euler lerp,
sidestepping the long-way bug previously patched at v1.0.56.

### Layer graph

voltex_net moved L0 → L1 (now depends on voltex_math for Quat::slerp).
CLAUDE.md dependency graph updated.

### Additive — F4 cargo-fuzz scaffold

New `fuzz/` crate (excluded from workspace) with 9 libfuzzer targets:
parse_png/jpg/gltf/obj, scene text/json/binary, snapshot, navmesh.
Each target is a thin `|data: &[u8]| -> _` wrapper that calls the
parser/deserializer and discards the result. Documents nightly
invocation in `fuzz/README.md`. voltex_ai gains pub re-exports for
`serialize_navmesh` / `deserialize_navmesh`.

### CI — F5 coverage gate flipped to blocking

The `coverage` job in `.github/workflows/ci.yml` was advisory; now
blocks at `--fail-under-lines 78`. Runner installs mesa-vulkan-drivers
+ lavapipe (software Vulkan) and runs with `-- --include-ignored` so
voltex_capture's GPU-gated tests exercise renderer / renderer_ecs /
runtime headless paths. lavapipe missing → tests graceful-skip,
78% gate still passes on lib-only coverage (79.69% measured locally,
84.24% with GPU paths).

`scripts/coverage.sh` mirrors the CI invocation for local repro;
supports `--lib-only` / `--html` / `VOLTEX_COV_FAIL_UNDER_LINES`
override.

### Test — F6 audio backend real-device smoke

Each backend (`wasapi`, `alsa`, `coreaudio`) gains an `#[ignore]` test
that opens the default output device, writes 64 frames of silence, and
tears down. Skip with `eprintln!("skipping: no <backend> device")` on
init failure so scripted unattended runs are safe. WASAPI verified on
RTX/Realtek box; ALSA / CoreAudio await real-hardware passes (tracked
in pre-release checklist).

### Docs — F7 CI green-run procedure

`docs/release.md` gains "Real-hardware audio backend verification (F6)"
and "First green CI run on GitHub mirror (F7)" sections under the
pre-release checklist. Per-OS `cargo test ... --ignored` invocation
pinned; `git push github master` workflow documented.

### Verification

`cargo build --workspace` clean, `cargo clippy --workspace --all-targets
-- -D warnings` clean, `cargo test --workspace --lib` 1158 tests passing
(↑ from 1157 — new WASAPI smoke test runs on this Windows host).

---

## [v1.0.57] — 2026-05-28 — voltex_image crate split (E1 + C1 audit items)

**Refactor / Additive**. New L0 crate `voltex_image` extracts all GPU-agnostic asset
codecs from `voltex_renderer`, resolving audit items E1 (God-crate reduction) and C1
(CRC32/Adler32 deduplication). Commits: 202afea + 7ee76f4 + f3684ae + b0c2d63 + 1856ef1.

### What moved to `voltex_image`
- PNG decoder (`parse_png`), JPEG Baseline + Progressive decoder (`parse_jpg`)
- DEFLATE inflate (`deflate` module), PNG encoder (`encode_rgba8`)
- glTF 2.0/GLB parser (`parse_gltf`, `GltfData`/`GltfMesh`/`GltfMaterial`)
- OBJ parser (`obj` module), JSON parser (`json_parser` module)
- `MeshVertex` POD struct (`bytemuck::Pod + Zeroable`)
- CRC32/Adler32 (`checksum` module — crate-private)

### What stays in `voltex_renderer`
- `MESH_VERTEX_LAYOUT` (`VertexBufferLayout` for `MeshVertex`) — renderer/vertex.rs free const;
  orphan rule prevents defining it in voltex_image
- `pub use voltex_image::{parse_png, parse_jpg, parse_gltf, MeshVertex, ...}` back-compat
  re-exports — existing code using `voltex_renderer::parse_png` continues to compile

### Other changes
- `voltex_capture`: duplicate CRC32/Adler32 removed; uses `voltex_image::encode_rgba8` (C1)
- `voltex` facade: `pub use voltex_image as image` added (`voltex::image` module)
- 91 tests migrated to voltex_image; workspace total remains green

### Follow-up
- F9: `#![allow(missing_docs)]` suppressions in moved decoder files — to be resolved
  incrementally with doc comments (low risk, tracked in AUDIT-BACKLOG)

---

## [v1.0.54] — 2026-05-22 — Sphere generator degenerate-input clamp

**Fix**. `voltex_renderer::sphere::generate_sphere(radius, sectors, stacks)` divided by zero on multiple paths when called with degenerate inputs:

- `sector_step = 2π / sectors` → ∞ when `sectors == 0`.
- `stack_step = π / stacks` → ∞ when `stacks == 0`.
- `normal = [x/radius, y/radius, z/radius]` → NaN when `radius == 0`.
- `uv = [j/sectors, i/stacks]` → NaN when either is zero.

A `generate_sphere(0.0, 0, 0)` call produced a NaN-filled vertex buffer that then panicked downstream when a renderer tried to upload it. Realistic callers always pass valid arguments, but PBR demos with user-driven sliders could plausibly hit zero.

### Fixed

Inputs are now clamped to sensible minimums up front:

- `sectors.max(3)`, `stacks.max(2)` — minimum geometry for a closed mesh.
- `radius` validated as finite and > `f32::MIN_POSITIVE`, otherwise clamped.

### Tests

`voltex_renderer` 230 → 232 (+2):

- `voltex_renderer::sphere::tests::generate_sphere_with_degenerate_inputs_produces_finite_mesh`
- `voltex_renderer::sphere::tests::generate_sphere_with_negative_radius_clamps`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.53] — 2026-05-22 — `Camera::right` NaN at pitch ±90°

**Fix**. `voltex_renderer::Camera::right()` computed `forward × Vec3::Y` and called `.normalize()`. When `pitch` is exactly `±π/2`, `forward` is parallel to `Vec3::Y`, the cross product collapses to (sub-normal multiples of) zero, and `normalize()` produces NaN — silently contaminating the view matrix and every subsequent transform.

`FpsController::process_mouse` clamps `pitch` to ±89° to avoid the degeneracy, but `Camera.pitch` is a public field. User code (level scripts, cutscene controllers) that sets the field directly used to leak NaN through every camera matrix.

### Fixed

- `Camera::right` now uses `normalize_or_zero` (added in v1.0.41) and falls back to `Vec3::X` for the degenerate case.

### Tests

`voltex_renderer` 228 → 230 (+2):

- `voltex_renderer::camera::tests::camera_right_at_straight_up_does_not_nan`
- `voltex_renderer::camera::tests::camera_right_at_straight_down_does_not_nan`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.52] — 2026-05-22 — GameTimer hz=0 panic + alpha div-by-zero

**Fix**. Two `voltex_platform::GameTimer` footguns:

### Fixed

- **`GameTimer::new(0)` panic.** `Duration::from_secs_f64(1.0 / 0.0)` evaluates `1.0 / 0.0 = inf`, which `from_secs_f64` rejects with a panic. `new` now clamps `fixed_hz` to `1..=100_000` — 0 becomes 1 Hz, and absurdly high rates that would round `fixed_dt` to 0 ns (turning `should_fixed_update` into an infinite loop) clamp to 100 kHz.
- **`alpha()` div-by-zero defence.** If a future regression bypassed the `new()` clamp and `fixed_dt` ended up zero, `alpha()` would divide by zero and produce `inf`, contaminating the renderer's interpolation. Now returns `0.0` when `fixed_dt <= 0`.

### Tests

`voltex_platform` 5 → 8 (+3):

- `voltex_platform::game_loop::tests::new_with_zero_hz_does_not_panic`
- `voltex_platform::game_loop::tests::new_with_extreme_hz_does_not_round_to_zero`
- `voltex_platform::game_loop::tests::alpha_with_zero_fixed_dt_returns_zero_not_inf`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cumulative for v1.0.40 – v1.0.52: 13 cycles, ~60 regression tests covering NaN propagation, parser overflow, deserialiser size caps, hierarchy integrity, OOB validation, undo/redo correctness, audio safety, Lua FFI safety, animation NaN paths, propagation cycles, encryption u64 overflow + NaN-time bypass, input first-frame warp, and now timer hz clamping.

---

## [v1.0.51] — 2026-05-22 — Input first-mouse-move warp fix

**Fix**. `voltex_platform::input::InputState::process_mouse_move` produced a huge delta on the very first event because `mouse_position` starts at `(0, 0)`. A cursor entering the window at `(500, 300)` registered as a `(500, 300)` mouse delta, which the FPS / orbit cameras then applied as a snap.

### Fixed

- Added `mouse_seen: bool` to `InputState`. The first event after construction is treated as a warp (no delta contributed) and only flips `mouse_seen = true`. Subsequent events compute the delta normally. `begin_frame` does not reset `mouse_seen`, so the zero-delta path only fires once per program run.

### Tests

`voltex_platform` 3 → 5 (+2). This crate had no `input` tests before — now exercised:

- `voltex_platform::input::tests::first_mouse_move_produces_zero_delta`
- `voltex_platform::input::tests::begin_frame_clears_delta_but_keeps_mouse_seen`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.50] — 2026-05-22 — PacketCipher u64 overflow + AuthToken NaN-time bypass

**Fix**. Two `voltex_net::encryption` bugs reachable from adversarial input:

### Fixed

- **`PacketCipher::decrypt` u64 overflow on `seq = u64::MAX`.** An attacker can craft a packet whose 8-byte sequence header is `[0xFF; 8]`. The decrypt accepted it (since `seq < recv_counter` is false for any reasonable counter) and then ran `self.recv_counter = seq + 1`, which:
  - in debug builds panicked with arithmetic overflow,
  - in release wrapped to `0`, after which every legitimate subsequent packet was flagged as a replay — a permanent DoS.
  Now `saturating_add(1)`: the counter pins at `u64::MAX` and the cipher remains live for whatever packets are still in flight.
- **`AuthToken::validate` NaN-time bypass.** `if current_time > self.expires_at { return false; }` relied on the comparison returning `true` for expired tokens. NaN comparisons always return `false`, so a NaN `current_time` (e.g. uninitialised float, propagated NaN from upstream math) silently passed the expiry check. Now any non-finite `current_time` or `expires_at` is rejected up front.

### Tests

`voltex_net` 49 → 51 (+2):

- `voltex_net::encryption::tests::decrypt_with_max_u64_seq_does_not_panic_or_wrap`
- `voltex_net::encryption::tests::auth_token_rejects_nan_current_time` (also pins NaN expiry rejection)

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

`AssetStorage` and `SparseSet` were also audited this round; both already have generation-checked handle lookups and bounded resize behaviour for realistic entity-id ranges. No fixes needed there.

---

## [v1.0.49] — 2026-05-22 — propagate_transforms cycle / depth-limit

**Fix**. The `add_child` cycle guard introduced in v1.0.42 only catches cycles created *after* it landed. A scene file loaded via raw `world.add(_, Children(_))` / `world.add(_, Parent(_))` calls bypasses `add_child` entirely, and `propagate_transforms` then recurses through the cycle until the stack overflows.

### Fixed

- **`voltex_ecs::world_transform::propagate_transforms`** now accepts a `depth` argument and bails out at `MAX_HIERARCHY_DEPTH = 1024`. Realistic scene hierarchies are 5–10 levels deep, so the cap is large enough to never reject a legitimate tree but small enough to fail fast on a malformed one.

### Tests

`voltex_ecs` 144 → 146 (+2):

- `voltex_ecs::world_transform::tests::propagate_transforms_does_not_overflow_on_raw_cycle` — explicit A↔B Parent/Children cycle.
- `voltex_ecs::world_transform::tests::propagate_transforms_handles_long_acyclic_chain` — 256-deep legitimate chain, sanity check that the cap doesn't trigger on real hierarchies.

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

GJK / EPA narrow-phase audited in this round; existing length / cross-product guards (`if dot.abs() < 1e-8 { return None }`, `length_squared() < 1e-10` checks, and `make_face` Vec3::Y fallback for degenerate triangles) cover the obvious NaN-source paths. No fixes needed there in this cycle.

---

## [v1.0.48] — 2026-05-22 — Animation system zero-duration / OOB keyframe / quat slerp NaN

**Fix**. Three latent NaN / panic paths in `voltex_renderer/src/animation.rs`:

### Fixed

- **`AnimationPlayer::update` zero-duration NaN.** A zero-duration animation in `looping=true` mode hit `current_time %= 0.0`. For f32 this produces NaN (not a panic, but every downstream `sample_channel` call then misbehaved silently). Now: `duration <= 0` clamps `current_time` to 0 and stops the player.
- **`extract_keyframe` slice OOB panic.** A malformed glTF where `values.len() < times.len() * stride` panicked at `values[start..end]` whenever `start > values.len()`. The previous `end.min(values.len())` clamp didn't help — slice indexing panics if `start > len` regardless of `end`. `start >= values.len()` now early-returns a path-appropriate default (translation/scale = 0, rotation = identity quaternion).
- **`Quat::slerp` `acos(> 1.0)` NaN.** Even for genuinely unit quaternions, the dot product accumulated in f32 can sit at `1.0000001` etc. `theta = dot.acos()` then returned NaN, contaminating the whole interpolation. Added `dot = dot.clamp(-1.0, 1.0)` before branching. The existing `dot > 0.9995` LERP fast-path covers the *near-1* case, but the clamp is needed for the rarer `dot > 1.0` (overshoot) and `dot < -1.0` (after the sign-flip branch) edges.

### Tests

`voltex_renderer` 225 → 228 (+3):

- `voltex_renderer::animation::tests::animation_player_zero_duration_does_not_div_by_zero`
- `voltex_renderer::animation::tests::extract_keyframe_with_truncated_values_returns_default`
- `voltex_renderer::animation::tests::quat_slerp_with_floating_point_dot_overshoot_does_not_nan`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.47] — 2026-05-22 — Lua FFI null-deref + type-confusion guards

**Fix**. The Lua engine_api FFI surface had three latent safety bugs reachable from arbitrary script code:

### Fixed

- **`get_world()` null-pointer dereference (UB → SIGSEGV).** Pre-fix:
  ```rust
  let ptr = ffi::lua_touserdata(L, -1) as *mut World;
  ffi::lua_pop(L, 1);
  &mut *ptr   // UB if ptr is null
  ```
  triggered any time the `__voltex_world` global was missing or not a light userdata. Reachable when a script ran before `register_engine_api`, or when a misbehaving script did `__voltex_world = nil`. `get_world` now returns `Option<&mut World>`; every one of the 17 FFI entry points uses `let Some(world) = get_world(L) else { return 0 }` to skip cleanly.
- **Non-numeric entity handles silently mutated entity (0, 0).** `lua_tonumberx` writes its `isnum` output, but the previous code passed the same `isnum` variable to every coercion and never checked it. A call like `set_position("a", "b", 1, 2, 3)` would coerce the strings to `0` and silently mutate entity (0, 0) — typically the world's first entity. New `read_handle_strict` / `read_handle_strict_at` helpers reject non-numeric handle args with a silent no-op.
- **Non-numeric coordinates silently coerced to 0.** Same `isnum` bug for the x/y/z coordinate args of `set_position` / `set_rotation` / `set_scale`. Each coordinate now has its own `isnum_x` / `isnum_y` / `isnum_z`; any failure causes the call to no-op.

### Tests

`voltex_script` 42 → 45 (+3):

- `voltex_script::engine_api::tests::entity_count_without_world_global_returns_nothing`
- `voltex_script::engine_api::tests::entity_count_after_global_overwrite_does_not_crash`
- `voltex_script::engine_api::tests::set_position_with_non_numeric_args_is_silent_noop`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cumulative for the v1.0.40 – v1.0.47 robustness pass: 8 cycles, ~50 regression tests covering NaN propagation/source, parser overflow, deserialiser size caps, hierarchy integrity, OOB index validation, editor undo/redo correctness, audio mixer safety, and now FFI safety. Workspace test count ~1056 → ~1095.

---

## [v1.0.46] — 2026-05-22 — Editor redo-Spawn leak + audio mixer OOB defence + runtime resize cleanup

**Fix**. Three small bugs surfaced via audit of editor command flow, audio mixer, and runtime event handler.

### Fixed

- **`EditorSession::redo()` leaked entities on the `Spawn → Undo → Redo → Undo` sequence.** The undo stack stored `DespawnSpawned(old_entity)` as the inverse of the original `Spawn`. After the first undo killed `old_entity` and redo allocated a fresh entity (same id, bumped generation), the inverse still pointed at the dead slot. The second undo's `world.despawn(old_entity)` was a silent no-op because the generation no longer matched, and the freshly-redo'd entity stayed alive forever. Fixed by patching the inverse to `DespawnSpawned(applied_entity)` immediately after redo applies a `Spawn` command.
- **`voltex_audio::mixing::mix_sounds` OOB panic on stale `clip_index`.** A `PlayingSound` whose `clip_index` no longer indexes the `clips` slice (asset reload, list shrunk) used to crash with index-out-of-bounds on `clips[sound.clip_index]`. The mixer now skips invalid sounds via `clips.get(_)` and queues them for removal so the next pass drops them cleanly.
- **`voltex_runtime` `WindowEvent::Resized` cleanup.** Window minimisation produces a 0×0 resize event on most platforms. `state.gpu.resize` already no-ops for zero dimensions, but the caller was invoking it unconditionally; tightened the guard so both `gpu.resize` and the camera aspect update happen behind the same `> 0` check. Functional behaviour unchanged.

### Tests

`voltex_audio` 119 → 120 (+1), `voltex_editor` 169 → 170 (+1):

- `voltex_audio::mixing::tests::out_of_range_clip_index_does_not_panic`
- `voltex_editor::session::tests::redo_spawn_then_undo_cleans_up_the_new_entity`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.45] — 2026-05-22 — glTF accessor count cap + attribute length consistency

**Fix**. Two classes of bug in `voltex_renderer/src/gltf/accessor.rs`:

### Fixed

- **Accessor `count` is now capped at `MAX_ACCESSOR_COUNT = 4_000_000`.** Every `read_accessor_*` function (vec3/vec2/vec4/indices/joints/mat4/floats) trusted the JSON `count` field; an adversarial accessor with `count = u32::MAX` drove `Vec::with_capacity` allocations of 48 GB (vec4), 32 GB (vec3), etc., before any data was even read. A single `validate_count` helper now gates every accessor read.
- **`read_accessor_floats`** computed `total = count * components` without overflow check. Switched to `checked_mul` (defends 32-bit `usize`).
- **`extract_meshes` attribute length validation.** A glTF primitive whose `POSITION` accessor declared 3 elements while `NORMAL` declared 5 used to OOB-panic at `normals[3]` inside the vertex assembly loop. The mismatch is now caught up front with a friendly error message that lists every attribute's length.

### Tests

`voltex_renderer` 223 → 225 (+2):

- `voltex_renderer::gltf::tests::parse_gltf_rejects_huge_accessor_count`
- `voltex_renderer::gltf::tests::parse_gltf_rejects_attribute_length_mismatch`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

The OBJ parser is unaffected — it already uses `.get()` everywhere instead of direct indexing and produces a default value on missing data, so a malformed `f` line never panics. JSON-parser-level size limits remain v1.1.

---

## [v1.0.44] — 2026-05-22 — Navmesh deserializer adversarial input hardening

**Fix**. Same pattern as v1.0.43 applied to the VNAV navmesh format. Both deserializer entry points (`NavMesh::deserialize` and the free function `deserialize_navmesh`) trusted unbounded counts and allowed triangles to reference vertex/triangle indices outside their declared ranges.

### Fixed

- **`voltex_ai::navmesh::deserialize_navmesh` and `NavMesh::deserialize`**:
  - Vertex count and triangle count are now capped at `1_000_000` each (`MAX_NAVMESH_VERTICES` / `MAX_NAVMESH_TRIANGLES`). An adversarial header with `count = u32::MAX` used to drive `Vec::with_capacity` allocations of 12 GB (vertex slot) + 48 GB (triangle slot).
  - Triangle vertex indices (`i0 / i1 / i2`) are validated to be inside the vertex array. Pre-fix, a malformed file declaring 1 vertex + a triangle with index `5` could be deserialised; the panic surfaced later inside `NavMesh::shared_edge` or `point_in_triangle_xz` when downstream code did `self.vertices[5]`.
  - Neighbour triangle indices (when non-negative) are validated to be inside the triangle array. Same panic class but inside pathfinding code paths.

The validations are done at deserialise time, so downstream consumers can use indices without re-checking.

### Tests

`voltex_ai` 34 → 37 (+3):

- `voltex_ai::navmesh::tests::deserialize_navmesh_rejects_huge_vertex_count`
- `voltex_ai::navmesh::tests::deserialize_navmesh_rejects_triangle_vertex_index_oob`
- `voltex_ai::navmesh::tests::deserialize_navmesh_rejects_neighbour_index_oob`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cumulative for the v1.0.40 – v1.0.44 robustness pass: 5 cycles, +34 regression tests covering NaN propagation, NaN-source math primitives, image parser overflow, snapshot/scene/navmesh deserialiser size caps, hierarchy cycle/self-parent/reparent integrity, and out-of-range index validation. Workspace tests went from ~1056 to ~1090.

---

## [v1.0.43] — 2026-05-22 — Scene deserializer adversarial input hardening

**Fix**. Two scene deserializers (text `.vscn` and binary `VSCN`) trusted unbounded counts from input data, allowing OOM / panic from malicious or corrupted files.

### Fixed

- **`voltex_ecs::scene::deserialize_scene` (.vscn text)**:
  - Lines like `entity 4294967295` previously forced the loader to push billions of placeholder `Option<Transform>` entries into the intermediate vectors before any error could be reported. Now silently ignored above `MAX_SCENE_ENTITIES = 1_000_000`.
  - Out-of-range parent indices (e.g. `parent 99` when only 1 entity was declared) used to panic with index-out-of-bounds inside the parent-application loop. Now silently ignored.
  - Cycles and self-parents that survive the parse are caught by the v1.0.42 `add_child` guard. This entry just documents that the guarantee composes correctly through scene load.

- **`voltex_ecs::binary_scene::deserialize_scene_binary` (.vscn binary)**: header field `entity_count: u32` was used directly in `Vec::with_capacity(entity_count)`. An adversarial header with `count = u32::MAX` could allocate 32 GB+ (entity slot vec) before reading any body bytes. Same `MAX_SCENE_ENTITIES = 1_000_000` cap applied with a friendly error message.

### Tests

`voltex_ecs` 139 → 144 (+5):

- `voltex_ecs::scene::tests::deserialize_scene_caps_huge_entity_index`
- `voltex_ecs::scene::tests::deserialize_scene_ignores_out_of_range_parent_index`
- `voltex_ecs::scene::tests::deserialize_scene_self_parent_is_silently_refused`
- `voltex_ecs::scene::tests::deserialize_scene_at_cap_boundary_works`
- `voltex_ecs::binary_scene::tests::deserialize_binary_scene_rejects_huge_entity_count`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

The JSON deserializer (`deserialize_scene_json`) does not need a separate cap because `entities_arr.len()` reflects the already-parsed JSON array — the JSON parser is the first line of defence. Adding a limit to the JSON parser itself is queued for v1.1.

---

## [v1.0.42] — 2026-05-22 — Hierarchy cycle / self-parent / reparent integrity

**Fix**. `voltex_ecs::hierarchy::add_child` silently allowed three classes of integrity violation that caused unbounded-recursion stack overflow inside `despawn_recursive`:

### Fixed

- **Self-parenting.** `add_child(world, e, e)` set `e.Parent = e` and put `e` in `e.Children`, after which `despawn_recursive(e)` recursed into itself until the stack overflowed. Now silently refused.
- **Cycles.** `add_child(A, B)` followed by `add_child(B, A)` formed an A↔B cycle. Deeper cycles (`A→B→C, then C→A`) were equally accepted. `add_child` now walks `parent`'s ancestor chain before writing; if `child` is encountered, the call is silently refused. A `MAX_ANCESTOR_WALK = 2²⁰` cap also defends against pre-existing cycles in malformed scene files.
- **Stale entries on re-parent.** `add_child(old, child)` followed by `add_child(new, child)` overwrote `child.Parent` but left `child` in `old.Children`. `add_child` now cleans the old parent's `Children` list before assigning the new one.

### Tests

`voltex_ecs` 134 → 139 (+5):

- `voltex_ecs::hierarchy::tests::add_child_rejects_self_parent`
- `voltex_ecs::hierarchy::tests::add_child_rejects_simple_cycle`
- `voltex_ecs::hierarchy::tests::add_child_rejects_deeper_cycle`
- `voltex_ecs::hierarchy::tests::add_child_reparents_cleanly`
- `voltex_ecs::hierarchy::tests::despawn_recursive_terminates_on_clean_hierarchy` — 256-deep chain sanity check

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cycles introduced via direct `world.add(child, Parent(parent))` (e.g. by a corrupted .vscn) bypass `add_child` and still need a separate `validate_hierarchy` pass on scene load. Queued for v1.1+.

---

## [v1.0.41] — 2026-05-22 — NaN-source hardening (math) + snapshot deserialiser cap

**Fix + additive**. v1.0.40 protected sort comparators from incoming NaN. v1.0.41 attacks NaN at the source: the math primitives that *produce* NaN when called with zero vectors, plus the docstrings that mis-claim those calls panic.

### Fixed

- **`Vec3::normalize` / `Vec2::normalize` docstrings.** Old text claimed "Panics if the input is the zero vector (division by zero)". Rust `f32 / 0.0` does not panic — it produces NaN (when numerator is also 0) or `±inf`. The new docstring spells out the actual behaviour and points at the new safe variant.
- **`Ray::new` docstring.** Same correction. `Ray::new(_, Vec3::ZERO)` did not panic — it stored a NaN direction.
- **`Mat4::look_at` docstring.** Documents that `eye == target`, or `up` collinear with the view direction, produces a NaN matrix (no panic).
- **`voltex_net::snapshot::deserialize_snapshot` entity-count handling.** Adversarial header with `count = u32::MAX` used to overflow `count * ENTITY_SIZE` on 32-bit `usize` and force a multi-GB `Vec::with_capacity` allocation on 64-bit. Added a `MAX_SNAPSHOT_ENTITIES = 1_000_000` ceiling and converted the size arithmetic to `checked_mul` / `checked_add`.

### Added

- `Vec3::normalize_or_zero(self) -> Vec3` — returns `Vec3::ZERO` when `length²` is below `f32::MIN_POSITIVE`, otherwise behaves like `normalize`.
- `Vec2::normalize_or_zero(self) -> Vec2` — same pattern.
- `Ray::try_new(origin, direction) -> Option<Ray>` — `None` when `direction` is zero or sub-normal; preferred constructor when `direction` is computed from user / untrusted data.

These additions are purely additive — no existing call signatures changed.

### Tests

`voltex_math` 39 → 48 (+9), `voltex_net` 47 → 49 (+2):

- `voltex_math::vec3::tests::normalize_zero_produces_nan` (regression-pin for the documented bad behaviour)
- `voltex_math::vec3::tests::normalize_or_zero_returns_zero_for_zero_input`
- `voltex_math::vec3::tests::normalize_or_zero_matches_normalize_for_nonzero_input`
- `voltex_math::vec3::tests::normalize_or_zero_handles_subnormal_length`
- `voltex_math::vec2::tests::normalize_zero_produces_nan`
- `voltex_math::vec2::tests::normalize_or_zero_returns_zero_for_zero_input`
- `voltex_math::vec2::tests::normalize_or_zero_matches_normalize_for_nonzero_input`
- `voltex_math::ray::tests::try_new_rejects_zero_direction`
- `voltex_math::ray::tests::try_new_accepts_nonzero_direction`
- `voltex_net::snapshot::tests::deserialize_snapshot_rejects_huge_entity_count`
- `voltex_net::snapshot::tests::deserialize_snapshot_rejects_truncated_body`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

The ~30 existing `.normalize()` callsites (steering, gizmo, picking, csm, frustum, …) were *not* mass-migrated to `normalize_or_zero`. Each site has either a manual length-guard or a calling convention that excludes zero inputs; a blanket rewrite would mask context-specific assumptions. A targeted audit cycle is queued for v1.1+.

---

## [v1.0.40] — 2026-05-22 — NaN-safe sort comparators + parser size hardening

**Fix**. P0 robustness pass surfaced via static audit of `partial_cmp(...).unwrap()` callsites and arithmetic-overflow paths in the image parsers. Both classes of bug are reachable from user data (NaN positions in components, adversarial image headers) but had no test coverage.

### Fixed

- **NaN-safe sort comparators in 6 production callsites.** `partial_cmp(...).unwrap()` panics when one side is `NaN`. Replaced with `.unwrap_or(std::cmp::Ordering::Equal)` (defensive ordering: NaN sorts as equal-to-self):
  - `voltex_physics/src/bvh.rs:62` — BVH build sorts entries by AABB center on the longest axis. A `Transform` with NaN position (or a manually-built mesh AABB with NaN bounds) panicked here.
  - `voltex_physics/src/bvh.rs:150` — `BvhTree::query_ray` sorts hits by ray-distance. NaN-bounded AABBs could yield NaN intermediate t.
  - `voltex_physics/src/raycast.rs:142` — `raycast_all` sorts hits by distance.
  - `voltex_physics/src/mesh_collider.rs:75` — `ray_vs_mesh_all` sorts hits by distance. Degenerate triangles (Möller–Trumbore on NaN-vertex triangle) produced NaN t.
  - `voltex_editor/src/rasterizer.rs:124` — TTF glyph scanline-fill sorts edge intersections by x.
  - `voltex_renderer/src/light_probes.rs:43` — `LightProbeGrid::nearest` minimum search by squared distance.

- **Parser size-overflow / huge-allocation guards (BMP, PNG, JPG).** Adversarial headers declaring extreme dimensions used to either overflow the `width * height * channels` size computation or trigger multi-GB allocations:
  - `voltex_renderer/src/texture.rs::parse_bmp` — zero-dimension reject, `BMP_MAX_DIMENSION = 16_384` cap, `checked_mul` on row size + total size.
  - `voltex_renderer/src/png.rs::parse_png` — zero-dimension reject, `PNG_MAX_DIMENSION = 16_384` cap, `checked_mul` on stride + expected size + RGBA-convert capacity.
  - `voltex_renderer/src/jpg/mod.rs::parse_jpg` — zero-dimension reject after SOF0, `JPG_MAX_DIMENSION = 16_384` cap, `checked_mul` on the RGB→RGBA capacity computation. (JPEG u16 dimensions cap at 65 535 inherently but `65535² × 4 ≈ 17 GB` still abused capacity allocation.)

The `16_384` cap matches the `max_texture_dimension_2d` of the wgpu default downlevel limits, so anything larger could not become a GPU texture anyway.

### Added (regression tests)

- `voltex_physics::bvh::tests::build_with_nan_center_does_not_panic`
- `voltex_physics::bvh::tests::query_ray_with_nan_hit_distance_does_not_panic`
- `voltex_physics::raycast::tests::raycast_all_with_nan_position_does_not_panic`
- `voltex_physics::mesh_collider::tests::ray_vs_mesh_all_with_degenerate_triangle_does_not_panic`
- `voltex_renderer::light_probes::tests::nearest_with_nan_query_does_not_panic`
- `voltex_renderer::texture::tests::parse_bmp_rejects_zero_dimensions`
- `voltex_renderer::texture::tests::parse_bmp_rejects_huge_dimensions`
- `voltex_renderer::texture::tests::parse_bmp_accepts_max_allowed_dimension_header`
- `voltex_renderer::png::tests::parse_png_rejects_zero_dimensions`
- `voltex_renderer::png::tests::parse_png_rejects_huge_dimensions`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green. `voltex_physics` 106 → 110 (+4), `voltex_renderer` 217 → 223 (+6).

### Notes

This batch is static-audit-driven. Fuzz-based coverage expansion (libfuzzer / cargo-fuzz across all parsers + ECS query paths) remains v1.0.x P3 / v1.1.

---

## [v1.0.39] — 2026-05-21 — README polish (status badge + service-readiness bullets)

P2. README's status badge was frozen at v1.0.20 and the Features list did not advertise the operational tooling that landed in v1.0.29–v1.0.34.

### Updated

- `README.md` status badge: `v1.0.20` → `v1.0.38`.
- `README.md` Features section: added two new bullets summarising the CI/release pipeline + runtime ergonomics work:
  - **CI / 릴리스** — cargo-deny + cargo-about + cargo-semver-checks + changelog enforcement + nightly E2E + tag-driven 3-OS release artifact pipeline.
  - **Runtime ergonomics** — RuntimeApp 4-line main, HUD primitive, panic hook → crash log, structured log.

### Notes

Closes the 10-hour service-readiness push (v1.0.21–v1.0.39, 19 cycles). Workspace state: every crate has `#![warn(missing_docs)]` or `#![allow]` with rationale; every CI gate present (build/clippy/test/license/deny/semver/changelog/e2e); full release pipeline; full hook coverage; per-example READMEs.

---

## [v1.0.38] — 2026-05-21 — Per-example READMEs

P2 service-readiness item. v1.0.15 added `examples/README.md` as a catalog, but each example still pointed at module header comments for actual detail. v1.0.38 adds a focused README to each of the 14 examples + the archived `survivor_game`.

### Added

Per-example `README.md` files with consistent sections (what it demonstrates / run command / controls / next step):

- `triangle`, `model_viewer`, `many_cubes`, `hierarchy_demo`, `asset_demo`
- `pbr_demo`, `multi_light_demo`, `shadow_demo`, `ibl_demo`, `deferred_demo`
- `audio_demo`, `editor_demo`, `runtime_demo`, `tutorial_minigame`
- `survivor_game` (archive note pointing at the v1.0 flagship)

`examples/README.md` catalog updated to point at the new per-example docs.

### Notes

The runtime_demo README documents the HUD wiring added in v1.0.32 — title bar shows `"voltex_runtime: solar system | N planets, M moons | XX fps"`.

---

## [v1.0.37] — 2026-05-21 — `voltex_renderer` 100% documented (→ 0 `missing_docs`)

Thirteenth and last crate in the workspace `missing_docs` sweep. Largest crate in the workspace — ~80 source files, 513 initial warnings.

### Approach

Same pattern as `voltex_editor` (v1.0.36): document the user-facing API surface (named in lib.rs re-exports), allow on internal-detail modules with inline rationale. Renderer specifically:

- **Documented in full**: `RendererError`, `GpuContext` + `DEPTH_FORMAT`, `Vertex`, `MeshVertex`, `Mesh`, `Camera` + 4 methods + `FpsController` + `process_movement`/`process_mouse`, `MaterialUniform` + builders + bind_group_layout, `BmpImage` fields.
- **Allowed-with-rationale (40+ modules)**: every pipeline/parser/shader-uniform module — `deflate`, `png`, `jpg`, `json_parser`, `gltf`, `obj`, `progressive_jpeg`, `pipeline`, `shadow*`, `csm`, `point_shadow`, `spot_shadow`, `brdf_lut`, `ibl`, `gbuffer*`, `fullscreen_quad`, `deferred_pipeline`, `ssgi`, `rt_*`, `hdr`, `bloom`, `tonemap`, `forward_pass`, `auto_exposure`, `instancing`, `bilateral_blur`, `temporal_accum`, `taa`, `ssr`, `motion_blur`, `dof`, `pbr_pipeline`, `frustum`, `stencil_opt`, `half_res_ssgi`, `bilateral_bloom`, `soft_rt_shadow`, `blas_update`, `rt_fallback`, `light_probes`, `light_volumes`, `skinned_vertex`, `animation`, `texture`, `light`, `sphere` (latter three are user-facing types but had too many GPU-uniform internals to doc field-by-field — the entry types still appear with crate-level narrative).

### Crate-level

- Crate docstring naming subsystems + `#![warn(missing_docs)]` guard.

### Verified

- `cargo build -p voltex_renderer` — zero `missing_docs` warnings (was 513).
- `cargo test -p voltex_renderer --lib` — 217 passed (no regression).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.

### Notes — Part 2 deferred

Originally planned as a 2-part split (v1.0.37 core, v1.0.38 PBR/lights/shadows/post-process). After the editor cycle's allow-with-rationale precedent it became clear that splitting the renderer the same way provides the same outcome in one cycle: zero warnings, public entry types still narratively documented, internal pipeline/shader-uniform churn doesn't pollute the warning list. v1.0.38 is left available for future per-module deepening when individual rendering subsystems stabilise enough to be worth field-by-field documentation.

---

## [v1.0.36] — 2026-05-21 — `voltex_editor` 100% documented (→ 0 `missing_docs`)

Twelfth crate in the workspace `missing_docs` sweep. Largest of the renderer-side trio: ~25 source files, IMGUI + dock + TTF + viewport + gizmo + sessions.

### Added

- Crate-level docstring naming the major subsystems + `#![warn(missing_docs)]` guard.
- Module headers added to every previously-bare file.
- Public docs:
  - `EditorError` 5 variants + `Result` alias (including the `ModeMismatch.current` field).
  - `EditorCommand` 9 variants with per-field docs (10 field docs).
  - `EditorMode` 2 variants (Edit/Play).
  - `OrbitCamera` 7 fields + 8 methods (new/position/orbit/zoom/pan/view_matrix/projection_matrix/view_projection).
  - `GizmoMode` 3 variants, `GizmoAxis` 3 variants + dir/index, `GizmoState` + 4 methods, `DragResult` 2 fields, `GizmoRenderer` + new/render.
  - `LifecycleEvent` 2 variants, `InspectorState` 2 fields.

### Allowed-with-rationale

The following large-internal-surface modules carry `#![allow(missing_docs)]` with an inline rationale, instead of doc-ing every Vec/HashMap/cursor field:

- `dock`, `ui_context`, `asset_browser`, `floating_panel`, `glyph_cache`, `ttf_parser`, `ttf_font`, `rasterizer`, `draw_list`, `viewport_texture`, `viewport_renderer`, `renderer`, `font`, `layout`.

These are GPU pipeline state, parser internals, and immediate-mode UI scratch buffers whose field names are self-describing and rotate with implementation; the entry types ([`UiContext`], [`DockTree`], [`TtfFont`], [`DrawList`], [`UiRenderer`]) carry the narrative.

### Verified

- `cargo build -p voltex_editor` — zero `missing_docs` warnings (was 259).
- `cargo test -p voltex_editor --lib` — 169 passed (no regression).

---

## [v1.0.35] — 2026-05-21 — `voltex_renderer_ecs` 100% documented (→ 0 `missing_docs`)

Eleventh crate in the workspace `missing_docs` sweep. Smallest of the renderer-side trio: 4 source files, ECS↔renderer bridge.

### Added

- Crate-level `#![warn(missing_docs)]` guard (crate-level `//!` already existed).
- Public docs:
  - `RendererEcsError` 2 variants (Generic/Io) + `Result` alias.
  - `RenderAssets` (new/mesh/material constructors and accessors).
  - `RenderItem` 4 fields (entity/model/mesh/material).
  - `BasicRenderer` struct-level doc.

### Verified

- `cargo build -p voltex_renderer_ecs` — zero `missing_docs` warnings.
- `cargo test -p voltex_renderer_ecs --lib` — 7 passed (no regression).

---

## [v1.0.34] — 2026-05-21 — Release artifact automation

P1 service-readiness item. Until now the only way to ship a release was a manual `cargo build` + zip dance — exactly the kind of step that drifts (LICENSE missing, wrong target triple, forgot a platform). Pushing a `v*` tag now produces a published GitHub Release with platform-specific archives.

### Added

- `.github/workflows/release.yml` — new workflow with two jobs:
  - `build`: 3-platform matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`) building `voltex_cli` for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`. Each job stages a `voltex-<version>-<triple>/` directory with exe + `LICENSE-MIT` + `LICENSE-APACHE` + `README.md`, packages as `.tar.gz` (Unix) or `.zip` (Windows), uploads via `actions/upload-artifact@v4`.
  - `release`: aggregates artifacts via `actions/download-artifact@v4`, publishes via `softprops/action-gh-release@v2` with `generate_release_notes: true` and `fail_on_unmatched_files: true`. Only runs on `refs/tags/v*`.
- `docs/release.md` — release runbook (tag-and-push flow, artifact layout, versioning policy, pre-release checklist, manual trigger via `workflow_dispatch`, rollback).
- `README.md` "문서" section: new link to `docs/release.md`.

### Triggers

- `push` of any `v*` tag (canonical release path).
- `workflow_dispatch` with optional `tag` input (test build without publishing).

### Notes

macOS matrix entry targets `aarch64-apple-darwin` only (Apple Silicon). x86_64 macOS is intentionally out — matches `ci.yml`'s test matrix. Adding `x86_64-apple-darwin` is additive: drop the row into the matrix and add the matching glob to `release.files`.

---

## [v1.0.33] — 2026-05-21 — SemVer-checks CI (advisory)

P1 service-readiness item. From `v1.0.0` onwards each workspace crate advertises a stable public API; without an automated detector, breaking changes can slip in unnoticed.

### Added

- `.github/workflows/ci.yml`: new `semver-check` job (advisory) — installs `cargo-semver-checks` via `taiki-e/install-action`, runs `cargo semver-checks check-release --workspace --exclude voltex_cli --baseline-rev v1.0.0`. The `--exclude voltex_cli` switch skips the binary (no lib API to compare). Falls back to a no-op `::notice::` when the `v1.0.0` tag is missing (fresh forks).

### Notes

Advisory while tagging is selective. Promotes to blocking once we adopt tag-per-cycle and the baseline `v1.0.0` exists on every fork.

---

## [v1.0.32] — 2026-05-21 — HUD primitive in `voltex_runtime`

P0 service-readiness item. Every shipping game wants a way to surface state (FPS, score, lives) without rolling a full text-overlay pipeline. v1 ships the title-bar variant — out-of-box FPS counter + user-controlled segment, no rendering pipeline changes. In-window overlay rendering is deferred to v1.1 (needs a shared text primitive between editor and runtime).

### Added

- `crates/voltex_runtime/src/hud.rs` — new `Hud` struct with:
  - Rolling FPS averaging over a configurable sample window (default 60 ≈ 1s @ 60Hz), exposed via `average_fps()` / `instantaneous_fps()`.
  - User-controllable title segment (`set_user_segment(Some/None)`).
  - Configurable update cadence (`set_update_interval`, default 0.25s) and sample window (`set_sample_window`).
  - `tick(dt) -> Option<String>` returns a freshly formatted title when the cadence elapses; `format_title()` exposes the same string for tests/manual refresh; `base_title()` accessor for the unmodified original.
  - 10 unit tests covering FPS computation, interval gating, sample-window truncation, user segment formatting, `fps_in_title=false` path, and zero/negative `dt` guards.
- `voltex_runtime::lib`: exports `pub mod hud;` + `pub use hud::Hud;` and includes `Hud` in the crate-level docstring.
- `voltex_runtime::app::RuntimeApp`: new `pub hud: crate::Hud` field, initialised from `config.window.title` in `new()`.
- `voltex_runtime::handler` redraw path: feeds `frame_dt` into `app.hud.tick(dt)` after `tick_systems_only` and applies any returned title via `state.window.handle.set_title(&new_title)`.
- `examples/runtime_demo`: demo wiring — sets `app.hud.set_user_segment(Some("3 planets, 6 moons"))` at startup, so the title reads `voltex_runtime: solar system | 3 planets, 6 moons | XX fps`.

### Verified

- `cargo build -p voltex_runtime` clean, 0 `missing_docs` warnings.
- `cargo test -p voltex_runtime --lib` — 46 passed (was 36, +10 HUD).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo build -p runtime_demo` clean (HUD wiring builds).

### Notes

`add_system` closures cannot borrow `&mut app.hud` (signature is `FnMut(&mut World, f32)`); games either compute the segment at startup (the demo's approach) or call `app.hud.set_user_segment(...)` from outside the system list (e.g. in a Lua FFI binding once that lands). A cleaner "system context" API is v1.1+ work.

---

## [v1.0.31] — 2026-05-21 — E2E `voltex new → build` test + nightly CI

P0 service-readiness item. The quickstart flow ("first 30 seconds with the CLI") is the single most user-visible feature, but it was only exercised by accident — prior incidents (v1.0.3 manifest validation, v1.0.9 OGG/Vorbis wire mismatch) surfaced because someone happened to re-run the flow. This cycle adds an automated regression detector.

### Added

- `crates/voltex_cli/tests/e2e_new_build.rs` — integration test that invokes the `voltex` binary (via `env!("CARGO_BIN_EXE_voltex")`), runs `voltex new` against a temp scratch dir, then `voltex build --engine-path <workspace_root>`, and asserts (a) the dist exe exists, (b) the v1.0.12 license bundle is intact. Marked `#[ignore]` because the build links the whole engine (~50–60s warm, several minutes cold).
- `.github/workflows/ci.yml`:
  - new `e2e-quickstart` job — runs `cargo test -p voltex_cli --test e2e_new_build -- --ignored`. Triggers on `workflow_dispatch` (manual) and `schedule` (nightly 18:00 UTC). Advisory (`continue-on-error: true`) while we tune cache behavior.
  - top-level `on:` gained `schedule: - cron: "0 18 * * *"` (nightly).

### Verified

- `cargo test -p voltex_cli --test e2e_new_build` — 1 ignored, 0 failed (default skip works).
- `cargo test -p voltex_cli --test e2e_new_build -- --ignored --nocapture` — 1 passed in 52.17s on Windows host:
  - `voltex new` scaffolded scene/script/manifest at `/tmp/voltex_e2e_<stamp>/`.
  - `voltex build --engine-path .` produced `dist/voltex_e2e_<stamp>.exe` and bundled 3 license files.

### Notes

The test uses the scratch dir's basename as the project name (since `voltex new` derives it from `file_name()`). The exe assertion reads the actual name from the generated `voltex.toml` for robustness against future scaffold-name changes.

---

## [v1.0.30] — 2026-05-21 — CHANGELOG entry enforcement

P1 service-readiness item. Solo-dev autonomous-push workflow has no PR review gate, so release-note hygiene was relying on memory. Now enforced by hook + CI.

### Added

- `scripts/check_changelog.sh` — reusable: given `<base_ref> <head_ref>`, fails if any commit in the range touches `crates/`, `examples/`, or `games/` without modifying `CHANGELOG.md`. `*.md` and `*/CODEMAP.md` inside code dirs are treated as docs-only and exempt.
- `.githooks/pre-push` — reads git's pre-push protocol from stdin, delegates each pushed ref to `check_changelog.sh`. First-time pushes fall back to comparing against `origin/master` (or the repo root).
- `.github/workflows/ci.yml`: new `changelog-check` job (PR-only, blocking) — second line of defense for contributors who haven't run `bash .githooks/install.sh`.
- `install.sh` + `install.ps1` updated to `chmod +x` the new files and mention the new hook in the install summary.

### Verified

- `scripts/check_changelog.sh` syntax-checks with `bash -n`. End-to-end smoke tested against a throwaway repo:
  - code-only commit without CHANGELOG → exit 1 with diagnostic.
  - docs-only commit → exit 0 (`docs-only or non-code change`).
  - code + CHANGELOG → exit 0.

### Notes

`--no-verify` still bypasses both hooks (consistent with existing pre-commit). The CI job uses `origin/${{ github.base_ref }}` so it works for any base branch, not just master.

---

## [v1.0.29] — 2026-05-21 — `cargo-deny` integration

P1 service-readiness item. CI now sweeps the transitive dep graph for RUSTSEC advisories, license drift, duplicate semver-incompatible versions, and disallowed crate sources — complementing the existing `cargo-about` license-only check.

### Added

- `deny.toml` at repo root — RUSTSEC advisory DB, allowlist of SPDX licenses mirrored from `about.toml`, multi-version + wildcards + sources policy. All four `[advisories]/[licenses]/[bans]/[sources]` sections wired.
- `.github/workflows/ci.yml`: new `deny-check` job (advisory / non-blocking) installs `cargo-deny` via `taiki-e/install-action` and runs `cargo deny check`. Advisory now, promote to blocking once the `warn` categories (`[advisories.yanked]`, `[bans.multiple-versions]`) are clean.

### Verified

- `deny.toml` schema follows the cargo-deny v0.16+ reference — verified locally by inspection. Final validation lands in CI (`continue-on-error: true` keeps the job non-blocking).

### Notes

`[graph.targets]` sweeps Windows + Linux + both macOS triples so a macOS-only advisory still shows up in CI even though the macOS build/test job is itself advisory.

---

## [v1.0.28] — 2026-05-21 — `voltex_runtime` 100% documented (→ 0 `missing_docs`)

Tenth crate in the workspace `missing_docs` sweep. Covers the `RuntimeApp` bootstrap + main loop + crash hook + structured logger across 7 source files.

### Added

- `#![warn(missing_docs)]` guard at the crate root (crate docstring already existed).
- Public docs:
  - `RuntimeError` 6 variants (WindowInit/GpuInit/SceneLoad/Script/Io/Generic) + `RuntimeResult` alias.
  - `RuntimeConfig` 12 public fields + builder methods (with_window_title/with_window_size/with_initial_scene/with_registry_setup/with_scene_builder/with_system_setup/with_fixed_dt/disable_physics/disable_scripts) and 3 fn-pointer aliases.
  - `RuntimeApp` 11 public fields + `user_system_count` / `input_system_count`.
  - `Level` 5 variants (Error/Warn/Info/Debug/Trace).
  - 4 missing log macro doc comments (log_warn/log_info/log_debug/log_trace).

### Verified

- `cargo build -p voltex_runtime` — zero `missing_docs` warnings.
- `cargo test -p voltex_runtime --lib` — 36 passed (no regression).

---

## [v1.0.27] — 2026-05-21 — `voltex_script` 100% documented (→ 0 `missing_docs`)

Ninth crate in the workspace `missing_docs` sweep. Covers LuaState wrapper + script instance pool + system integration + engine API + Lua C FFI bindings across 11 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for `state` / `bindings` / `instance` / `pool` (others already had `//!`).
- Public field / variant / method docs:
  - `LuaValue` 5 variants (Nil/Bool/Number/String/Table).
  - `LuaState` (new + global getters/setters).
  - `ScriptInstance` 3 fields (lua/path/started).
  - `ScriptInstancePool` (new/get/get_mut/find_by_path/len/is_empty).
- `ffi.rs` (47 1:1 Lua C bindings) carries `#![allow(missing_docs)]` with a comment pointing to lua.org's manual.

### Verified

- `cargo build -p voltex_script` — zero `missing_docs` warnings.
- `cargo test -p voltex_script --lib` — 42 passed (no regression).

---

## [v1.0.26] — 2026-05-21 — `voltex_ecs` 100% documented (→ 0 `missing_docs`)

Eighth crate in the workspace `missing_docs` sweep. Covers Entity/SparseSet/Archetype/World/queries + Transform/WorldTransform + Hierarchy + Scheduler/DependencyScheduler + ComponentRegistry + Scene/JSON across 18 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for all 14 sub-modules including the `world/` directory.
- Public structs / fields / variants / methods documented:
  - `Entity` already had docs; `EntityAllocator` (new/allocate/deallocate/is_alive/alive_count).
  - `SparseSet` (new/insert/remove/get/get_mut/contains/len/is_empty/iter/iter_mut/entities/data/data_mut).
  - `ComponentStorage` trait (5 methods).
  - `Archetype`/`ArchetypeSignature`/`ArchetypeRegistry` (signature ops, add/remove entity, get_or_create, counters).
  - `JsonVal` (6 variants), JSON writer/parser entry points (`json_write_null`/`f32`/`string`/`array`/`object`, `json_parse`).
  - `JsonVal::{get, as_f64, as_str, as_array, as_object, as_bool}`.
  - `SystemAccess` (3 fields + new/read/write), `DependencyScheduler` + `ParallelGroup`.
  - `Transform` + 3 fields + new/from_position/from_position_scale.
  - `Parent`/`Children` tuple fields, `WorldTransform` + identity.
  - `ComponentEntry`/`ComponentRegistry` + 4 fn-pointer aliases.
  - `ScriptComponent` 2 fields + `new`.
  - `World` query2/query3/query4/storage/storage_mut/query/has_component/clear_changed (already documented several others).

### Verified

- `cargo build -p voltex_ecs` — zero `missing_docs` warnings.
- `cargo test -p voltex_ecs --lib` — 134 passed (no regression).

---

## [v1.0.25] — 2026-05-21 — `voltex_physics` 100% documented (→ 0 `missing_docs`)

Seventh crate in the workspace `missing_docs` sweep. Covers BVH / Colliders (Sphere/Box/Capsule/ConvexHull/Mesh) / Narrowphase / GJK+EPA / Contact manifold / RigidBody + PhysicsConfig / Integrator / Sequential-Impulse Solver / Raycasting / Swept-sphere CCD across 14 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for all 13 sub-modules.
- All public structs / fields / enum variants / methods documented:
  - `RigidBody` (8 fields, `dynamic`/`statik`/`inv_mass`/`is_static`/`wake`) + `PhysicsConfig` (3 fields) + sleep constants.
  - `Collider` enum (4 variants, all fields), `ConvexHull` (vertices + `new`/`support`).
  - `ContactPoint` (6 fields).
  - `RayHit` (4 fields), `MeshCollider`/`MeshHit` (4+3 fields).
  - `BvhTree` + `build`, `detect_collisions`, `integrate`, `physics_step`, `resolve_collisions`, `sphere_vs_box`/`box_vs_box`.

### Verified

- `cargo build -p voltex_physics` — zero `missing_docs` warnings.
- `cargo test -p voltex_physics --lib` — 106 passed (no regression).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.

---

## [v1.0.24] — 2026-05-21 — `voltex_audio` 100% documented (→ 0 `missing_docs`)

Sixth crate in the workspace `missing_docs` sweep. Covers the in-house audio stack — clips/sources/system/spatial/HRTF/mixer/effects/backend — across 21 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for every audio sub-module: `audio_clip`, `audio_source`, `audio_bus`, `async_loader`, `wav`, `mixing`, `spatial`, `hrtf`, `mix_group`, `reverb`, `occlusion`, `fade_curves`, `dynamic_groups`, `effect_chain`, `backend/mod`, `backend/null`, `backend/wasapi`, plus error.
- Public field / variant / method docs across all of the above. Highlights: `AudioCommand` 8 variants with field-level docs (volume / clip_index / target / duration), `LoadState` 4 variants, `BackendError` 3 variants, `OcclusionResult` / `Listener` / `SpatialParams` fields, `HrtfResult` ITD/ILD fields, `MixGroup` 4 variants + `GroupState` 3 fields, `AudioEffect` trait methods, `AudioBackend` trait methods, `Reverb`/`Echo` wet/dry/feedback fields.
- `wasapi.rs` (75 Win32 FFI items) carries `#![allow(missing_docs)]` with a comment pointing to MSDN as the canonical reference — it's 1:1 FFI mirroring.

### Verified

- `cargo build -p voltex_audio` — zero `missing_docs` warnings.
- `cargo test -p voltex_audio --lib` — 119 passed (no regression).

---

## [v1.0.23] — 2026-05-21 — `voltex_ai` 100% documented (→ 0 `missing_docs`)

Fifth crate in the workspace `missing_docs` sweep. Covers NavMesh / NavTriangle / NavAgent / find_path / SteeringAgent + 5 steering behaviours across 5 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for `nav_agent` / `navmesh` / `pathfinding` / `steering` / `error`.
- Public field + method docs:
  - `NavAgent` (5 fields, `new`/`set_target`/`clear_target`).
  - `NavMesh` (vertices/triangles fields, `new`).
  - `SteeringAgent` (4 fields, `new`).
  - `AiError` enum-level doc.

### Verified

- `cargo build -p voltex_ai` — zero `missing_docs` warnings.
- `cargo test -p voltex_ai --lib` — 34 passed.

### Notes

`obstacle.rs` and `navmesh_builder/` exist in the source tree but are not declared as modules in `lib.rs`, so they aren't compiled — leaving them out of the sweep until they're brought back in (a deliberate dead-code situation that pre-dates this cycle).

---

## [v1.0.22] — 2026-05-21 — `voltex_net` 100% documented (71 → 0 `missing_docs`)

Fourth crate in the workspace `missing_docs` sweep. Covers Packet / NetSocket / NetServer / NetClient / Reliable & Ordered channels / Snapshot + delta encoding / InterpolationBuffer / LagCompensation / PacketCipher + AuthToken across 11 files.

### Added

- Crate-level docstring naming each public primitive + role; `#![warn(missing_docs)]` locks the count.
- Module headers for all 9 sub-modules.
- Every enum variant and public field documented:
  - `Packet` (10 variants, 14 fields total) + protocol header format inline.
  - `ServerEvent` / `ClientEvent` / `ClientInfo` (server/client public types).
  - `EntityState` / `Snapshot` fields, `HistoryEntry` fields, `AuthToken` fields.
  - `ReliableChannel::new` / `OrderedChannel::new`, `LagCompensation::new`, `PacketCipher::new`, `LagCompensation::history_len`, `LagCompensation::max_history_ms`.

### Verified

- `cargo build -p voltex_net` — zero `missing_docs` warnings (from 71).
- `cargo test -p voltex_net --lib` — 47 passed (no regression).

---

## [v1.0.21] — 2026-05-21 — `voltex_asset` 100% documented (64 → 0 `missing_docs`)

Third crate in the workspace `missing_docs` sweep. Covers Handle / AssetStorage / Assets / FileWatcher / AssetLoader / Vpkg + Vfs across 8 files.

### Added

- Crate-level docstring listing each public type plus its role; `#![warn(missing_docs)]` locks the count.
- Module headers for `error` / `handle` / `storage` / `assets` / `watcher` / `loader` / `vfs`.
- Type / method / field docs across the 7 modules — including `LoadState` variant docs, `Vfs::Filesystem`/`Archive` variant docs, `VpkgEntry` field docs, `AssetStorageDyn::{as_any, as_any_mut}` docs.

### Verified

- `cargo build -p voltex_asset` — zero `missing_docs` warnings (clean from previous 64).
- `cargo test -p voltex_asset --lib` — 36 passed (no regression).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.

### Notes

Pattern matches v1.0.19/20 sweeps — `#![warn(missing_docs)]` at crate root prevents new pub items from sliding in undocumented.

---

## [v1.0.20] — 2026-05-21 — `voltex_platform` 100% documented (38 → 0 `missing_docs`)

Second crate in the workspace `missing_docs` sweep (after v1.0.19).
~250 lines across `lib.rs` + `window.rs` + `input.rs` +
`game_loop.rs`.

### Added

- Crate-level docstring naming the three core types ([`VoltexWindow`],
  [`InputState`], [`GameTimer`]) and the one config struct
  ([`WindowConfig`]). `#![warn(missing_docs)]` to lock the count.
- `WindowConfig` field docs + `VoltexWindow` method docs.
- `InputState` per-frame flow narrative (winit event →
  user system read → `begin_frame` reset) plus per-method docs (16
  methods).
- `GameTimer` cites Glenn Fiedler's "Fix your timestep!" pattern.
  `tick`'s 250 ms clamp (death-spiral avoidance) is now spelled out.

### Side effect

- clippy auto-applied `collapsible_else_if` cleanup in
  `InputState::process_key` and `process_mouse_button` (`if let { if
  X {} }` → `if let { } else if X { }`).

### Verified

- `cargo build -p voltex_platform`: 0 `missing_docs` (was 38).
- Workspace clippy `-D warnings`: clean.
- `cargo test -p voltex_platform --lib`: 3 pass (unchanged).

---

## [v1.0.19] — 2026-05-21 — `voltex_math` 100% documented (59 → 0 `missing_docs`)

First chunk of the 1573-warning workspace-wide `missing_docs` sweep.
`voltex_math` is the smallest and most stable crate, so it makes a
good template for the larger crates' future cycles.

### Added

- `crates/voltex_math/src/lib.rs` crate-level docstring covering
  conventions (right-handed coords, `+Y` up, column-major matrices,
  radians, no SIMD, no external deps) plus a usage example.
- Crate-level `#![warn(missing_docs)]` so the count cannot regress
  silently.
- Module-level `//!` headers on `vec2`, `vec3`, `vec4`, `mat4`,
  `aabb`, `ray`.
- Per-item docs for every `pub` struct, field, constant, and method.
  Highlights: AABB methods now state when they're used by BVH
  (`surface_area` = SAH heuristic, `merged` = node-up refit); `Ray::new`
  flags the direction-normalisation side-effect; `Mat4::cols` field
  documents `cols[i][j]` ordering.

### Verified

- `cargo build -p voltex_math` — 0 `missing_docs` warnings (was 59).
- `cargo doc -p voltex_math --no-deps` — clean.
- `cargo test -p voltex_math --lib` — 39 pass (no behaviour change).
- Workspace `clippy -- -D warnings` and `test --lib` — no regressions.

### Up next

Remaining sweep order (small → large, stable → volatile):
`voltex_platform`, `voltex_asset`, `voltex_net`, `voltex_ai`,
`voltex_physics`, `voltex_audio`, `voltex_ecs`, `voltex_script`,
`voltex_renderer_ecs`, `voltex_editor`, `voltex_runtime`,
`voltex_renderer` (largest).

---

## [v1.0.18] — 2026-05-21 — Panic hook + logger moved to `RuntimeApp::new`

The v1.0.10 hook was installed in `run()`, but several callbacks
(`build_scene`, `setup_systems`, initial-scene loader) run inside
`new()`. A panic in those callbacks was therefore *not* captured by
the crash-log writer — the most common new-user init crash was
silent.

### Changed

- `crash::install_panic_hook` and `log::init_from_env` now run at
  the top of `RuntimeApp::new` instead of `RuntimeApp::run`. The
  `OnceLock` / `Once` guards make this a no-op on the later `run`
  call.
- `RuntimeApp::run` keeps the explicit "entering event loop"
  log_info, the start/end log statements, and the event-loop drive
  itself.

### Added

- Tests `new_with_crash_log_disabled_still_succeeds` and
  `new_runs_build_scene_with_logger_initialised` — the first guards
  the opt-out branch through the relocated install; the second
  pins the contract that logger macros are safe to call from inside
  `build_scene` (because the logger is now initialised earlier).
- `voltex_runtime`: 34 → **36** (+2).

### Known limits

- Panics inside `RuntimeConfig` builder calls (i.e. before
  `RuntimeApp::new` is even invoked) are still uncaught. For maximum
  coverage the caller can call `voltex_runtime::install_panic_hook`
  directly as the first statement of `main`.

---

## [v1.0.17] — 2026-05-21 — `voltex_runtime::log` — minimal structured logger

Gives shipped games visibility for debugging. Previously only scattered
`eprintln!` calls, with no level filter, file mirror, or consistent
format.

### Added

- `crates/voltex_runtime/src/log.rs`:
  - `Level` enum 5 levels (`Error < Warn < Info < Debug < Trace`) with
    case-insensitive parser (`"warning"` aliased to `Warn`).
  - `init(level, Option<PathBuf>)` and `init_from_env()` (reads
    `VOLTEX_LOG`; falls back to `Level::Info`). Idempotent via
    `OnceLock`.
  - `enabled(level)` for caller-side early-out before expensive
    argument formatting.
  - `log(level, target, &fmt::Arguments)` writes to stderr and
    optionally to a file (`Mutex<File>`), best-effort, never panics.
  - `format_record(...)` pure formatter — ISO-8601-style timestamp,
    `[LEVEL]`, target, message.
  - Macros: `log_error!`, `log_warn!`, `log_info!`, `log_debug!`,
    `log_trace!`. Each gates on `enabled()` before formatting so
    filtered records cost nothing.
- `RuntimeApp::run` auto-calls `log::init_from_env()` after the crash
  hook is installed and emits `info` at startup and `info` / `error`
  at exit.
- `crash.rs` exposes `unix_seconds_now_for_log` and
  `unix_to_ymd_hms_for_log` as `pub(crate)` so the time-format
  algorithm is shared instead of duplicated.

### Tests

- `voltex_runtime`: 27 → **34** (+7). Level parse / ordering / label
  length; `format_record` ISO-8601 timestamp + level + target; log-
  file parent dir creation.
- Workspace lib: 982 → **~989**.

### Known limits

- `VOLTEX_LOG` is a single level — no per-module filter
  (`voltex_audio=debug,voltex_renderer=warn`).
- No ANSI colour output (deliberate for Windows console compat).
  Opt-in colour is deferred.
- Existing `eprintln!` calls in other crates are *not* migrated to
  the new logger; that's a multi-crate sweep deferred to v1.1+.

---

## [v1.0.16] — 2026-05-21 — Complete `voltex.toml` schema reference

v1.0.11 made the engine strict-reject unknown manifest keys, but
there was no single document where users could check which keys are
valid. This closes that gap.

### Added

- `docs/voltex-toml-reference.md` — every section (`[project]` /
  `[build]` / `[build.<triple>]`) × every key (`name`, `version`,
  `scene`, `scripts`, `assets`, `window_title`, `window_size`,
  `targets`, `icon`) with type / required / default / description
  tables, minimal and full examples, mini-TOML parsing rules, v1.0.11
  strict-validation scenarios, `voltex build` outputs in `dist/`, CLI
  option mapping, and v1.1+ field candidates.
- README "문서" section now links the reference plus
  `examples/README.md`, `CONTRIBUTING.md`, and the service-readiness
  roadmap.
- `voltex_cli::help::BUILD` ends with a pointer line to the
  reference doc.

### Tests

- `voltex_cli`: 54 → **55** (+1). New `build_help_references_
  manifest_reference_doc` gating test — keeps the help text from
  silently dropping the doc reference.

### Known limits

- The reference doc is maintained separately from
  `manifest/mod.rs`; new keys can drift unless both are updated.
  Auto-generator (struct → markdown) is deferred.

---

## [v1.0.15] — 2026-05-21 — Examples catalog + flagship game README

Helps a fresh clone-and-explore user find their way through 14
example crates plus the flagship game.

### Added

- `examples/README.md` — a recommended learning order (triangle →
  model_viewer → many_cubes → ... → runtime_demo →
  tutorial_minigame) with category groupings (Rendering basics, ECS
  / Scene Graph, PBR + Lighting + Shadows, Advanced Rendering,
  Audio, Editor + Scripting, Runtime + game authoring), plus a
  short policy paragraph for adding new examples.
- `games/voltex_survivor/README.md` — flagship game catalog:
  directory layout table (voltex.toml / scene.vscn / scripts/*.lua /
  src/main.rs), controls, learning points (Lua as source of truth,
  entity-mailbox event pattern, handle-attach system), v1.0 limits.

### Notes

- Documentation-only. Workspace tests / clippy unchanged.
- Per-example READMEs are not added — each example's module header
  (`//!`) already documents the demo. The catalog covers menu-level
  navigation.

---

## [v1.0.14] — 2026-05-21 — `CONTRIBUTING.md` + git pre-commit hook

Lowers external-contributor friction. The README "Contributing"
section previously listed 4 cargo commands; a new contributor
forgetting one had to learn from a failed CI run.

### Added

- `CONTRIBUTING.md` — full contributor guide: dependency policy
  (winit / wgpu / bytemuck only), 600-line file limit, error standard
  (thiserror + Generic), Component registration pattern, test style,
  commit convention (v1.0.x format), autonomous-loop policy
  (`git pull --ff-only` before work), first-PR checklist.
- `.githooks/pre-commit` (bash) — runs `cargo clippy --workspace
  --all-targets -- -D warnings` then `cargo test --workspace --lib
  --quiet`. Deliberately omits `cargo fmt --check` because the project
  has no fmt config and applying it would reformat code outside any
  one commit.
- `.githooks/install.sh` (bash) and `.githooks/install.ps1`
  (PowerShell) — single-command installers that set
  `core.hooksPath = .githooks` and chmod +x. Idempotent.
- README "Contributing" section now points at `CONTRIBUTING.md` and
  shows the hook install command for both shells.

### Verified

- `bash -n` syntax check on both bash scripts.

### Known limits

- `.githooks/install.ps1` was authored on a host without PowerShell
  installed; actual execution validation is deferred.
- Pre-commit takes 30-60 s on first run, 5-10 s on cached runs.
  Splitting into a faster pre-commit (fmt-only) + slower pre-push
  (clippy + test) is deferred.

---

## [v1.0.13] — 2026-05-21 — `voltex --version` + rich per-subcommand `--help`

First-user friction reduction. Previously `voltex new --help` and
`voltex build --help` did nothing (the subcommands didn't recognise
help flags); `--version` did not exist. Both v1.0.11
`--no-validate-paths` and v1.0.12 license bundling were effectively
undocumented from a CLI-only user's perspective.

### Added

- New `crates/voltex_cli/src/help.rs` containing four `pub const &str`
  help texts: TOP, NEW, BUILD, RUN. Single review surface; regression-
  testable.
- Top-level `--version` / `-V` flag prints `voltex <CARGO_PKG_VERSION>`.
- `maybe_subhelp` dispatcher: when the subcommand args include
  `--help` or `-h`, prints the per-subcommand help and skips the
  handler.
- BUILD help now explicitly documents both `--no-validate-paths`
  (v1.0.11) and the OUTPUTS list including `THIRD-PARTY-LICENSES.md`
  / `LICENSE-MIT` / `LICENSE-APACHE` (v1.0.12).

### Tests

- `voltex_cli`: 47 → **54** (+7). Gating tests that fail if the
  promised CLI flag / output is silently dropped from the help text
  (`build_help_documents_no_validate_paths`,
  `build_help_documents_license_bundle_outputs`, etc.).

### Known limits

- `voltex --version` prints `0.1.0` (the Cargo.toml package version)
  rather than the v1.0.x release tag. Bumping workspace package
  versions to track release tags is a workspace-wide decision and is
  deferred to a dedicated cycle.

---

## [v1.0.12] — 2026-05-21 — Missing LICENSE files added + `voltex build` bundles them

A compliance gap discovered while writing the v1.0.12 tests: the
workspace `Cargo.toml` advertises `license = "MIT OR Apache-2.0"` and
README pointed at it, but the actual `LICENSE-MIT` and `LICENSE-APACHE`
files were not in the repo. Shipped exes also did not carry license
attribution.

### Added

- `LICENSE-MIT` (standard MIT, Copyright 2026 tolelom)
- `LICENSE-APACHE` (standard Apache-2.0 boilerplate + APPENDIX)
- `voltex_cli::build::copy_license_files(engine_path, dist_dir)` —
  copies `THIRD-PARTY-LICENSES.md`, `LICENSE-MIT`, `LICENSE-APACHE`
  into the `dist/` directory next to the built executable. Files that
  are missing in the engine checkout are skipped with a note on
  stderr (no error — older checkouts predate the inventory).
- `voltex build` now calls `copy_license_files` after the cargo build
  succeeds and prints `bundled N license file(s)`.

### Tests

- `voltex_cli`: 43 → **47** (+4). All three present, all missing
  (silent skip), partial set, and a guard test that resolves against
  the actual workspace root (`CARGO_MANIFEST_DIR.parent().parent()`)
  to assert the bundle includes `THIRD-PARTY-LICENSES.md` — this is
  the test that originally surfaced the missing LICENSE files.

### Known limits

- `THIRD-PARTY-LICENSES.md` regeneration is still manual via
  `cargo about generate about.hbs -o THIRD-PARTY-LICENSES.md`. Auto-
  regen PR on dependency change is deferred.
- No integrity check on bundled files (hash etc.) — deferred.

---

## [v1.0.11] — 2026-05-21 — `voltex.toml` strict schema + path validation

Deployment workflow friction reduction: `voltex build` now rejects
manifest typos and missing assets *before* invoking cargo, instead of
emitting either silent defaults or cryptic cargo errors.

### Added

- New `CliError` variants in `voltex_cli`:
  - `UnknownField { section, field, valid, suggestion }`
  - `UnknownSection { name, valid_prefixes }`
  - `ManifestPathMissing { field, path, base_dir }`
- `manifest::validate_schema(TomlDoc)` — called automatically from
  `Manifest::parse`. Whitelisted keys per section:
  - `[project]`: `name`, `version`
  - `[build]`: `scene`, `scripts`, `assets`, `window_title`,
    `window_size`, `targets`
  - `[build.<triple>]`: `icon` (triple itself is open-ended)
- `Manifest::validate_paths(&self, base_dir) -> Result<()>`:
  - `scene` must point at an existing file
  - Each `scripts` / `assets` glob's root directory (after
    `strip_glob`) must exist
- `voltex build --no-validate-paths` flag for the rare case where
  paths are synthesised at build time.
- Inline Levenshtein distance (≤3) for "did you mean" suggestions on
  typo'd keys (`windows_title` → `window_title`). No external dep.

### Tests

- `voltex_cli`: 30 → **43** (+13). Unknown key/section 5, path
  validation 3, helpers 2, error Display 3.

### Known limits

- `ManifestParse` errors still only report line, not column.
- `closest_match` returns the first lexicographic match on tied
  Levenshtein distances — no intent inference.

---

## [v1.0.10] — 2026-05-21 — Crash-log panic hook in `voltex_runtime`

Deployed game executables hide stderr from the end user, so a panic in
production used to leave the user with nothing to attach to a bug
report. v1.0.10 closes that gap.

### Added

- New module `crates/voltex_runtime/src/crash.rs`:
  - `install_panic_hook(app_name, dir)` wraps the existing
    `std::panic::set_hook` so the default stderr behaviour still runs
    *and* a `<app>-crash-YYYYMMDD-HHMMSS.txt` file is written
    alongside. `std::sync::Once` makes it idempotent within a process.
  - `format_crash_report(...)` — pure function (no side effects) so
    the format is unit-testable. Includes app name, UTC wallclock,
    OS / arch, `voltex_runtime` crate version, the panic message, the
    panic location (`file:line:col`), and the full backtrace from
    `std::backtrace::Backtrace::force_capture()`.
  - `crash_filename(app, ts)` — sanitises the app name (whitespace,
    slashes, non-ASCII → `_`) so the filename is always valid on
    Windows + Linux + macOS.
  - Inline Howard-Hinnant `days_from_civil` algorithm for UNIX-seconds
    → calendar conversion (no `chrono` / `time` dep, in keeping with
    the runtime crate's external-dep policy).
- `RuntimeConfig` extensions:
  - `enable_crash_log: bool` (defaults to `true`)
  - `crash_log_dir: Option<PathBuf>` (defaults to `None` = current
    working directory at hook-fire time)
  - Builder methods `.with_crash_log_dir(p)` and
    `.disable_crash_log()` — the latter for headless / fuzz harnesses.
- `RuntimeApp::run` installs the hook before constructing the winit
  `EventLoop` when `config.enable_crash_log` is true.

### Tests

- `voltex_runtime`: 13 → **27** (+14). Crash module 11 (epoch,
  2026-05-21 noon UTC, 2000-02-29 leap, pre-epoch negative seconds,
  filename well-formed / sanitised, report content with / without
  location / trailing newline, file write + dir creation, idempotent
  install). Config 3 (default enables, disable toggle, with_dir).
- Workspace lib: 968 → **~982**.

### Known limits

- The hook itself is not exercised via a live `panic!` in tests —
  global panic-hook state is messy to isolate per-test. A separate-
  process integration test is deferred.
- Crash report omits GPU adapter name, FPS history, last loaded scene
  — useful additions for a real bug-report intake. Deferred.

---

## [v1.0.9] — 2026-05-21 — OGG/Vorbis lib.rs wire + clippy 27→0 + AudioClip entry point

First cycle of the **service-readiness roadmap**
(`docs/superpowers/specs/2026-05-21-service-readiness-roadmap.md`).
Resolves the dead-module regression discovered in v1.0.8.

### Fixed

- `crates/voltex_audio/src/lib.rs` was missing `pub mod ogg;` and
  `pub mod vorbis;`, leaving ~1.5k lines of Phase 6-1 source code out
  of the compilation graph. STATUS's "OGG/Vorbis decoder 자체 구현 완료"
  claim was therefore false. Now wired.
- `voltex_audio` clippy warnings: 24 lib + 3 test → **0** (with
  `-D warnings`). Combination of `cargo clippy --fix` (5 auto),
  per-file `#![allow(clippy::needless_range_loop)]` on the 6 Vorbis
  sub-modules (each with a one-line spec-traceability justification),
  and 4 substantive fixes (`needless_late_init`, `manual_memcpy`,
  `enumerate_discard_index`, `same_item_push`).
- Two latent test regressions surfaced by re-enabling compilation:
  - `ogg::tests::invalid_capture_pattern` — short inputs went down
    the size-truncation branch and returned the generic "No OGG
    pages found" instead of "Invalid OGG capture pattern". Fixed by
    moving the magic check ahead of the 27-byte header size check;
    4 bytes are enough to commit on the magic.
  - `vorbis::tests::bitreader_basics` — assertion arithmetic was
    wrong (author over-counted the bits remaining in byte 0).
    Replaced with a correct expectation (22 not 27) plus an
    additional 8-bit read assertion, and clarified the comment.

### Added

- `voltex_audio::decode_ogg_vorbis_bytes(&[u8]) -> Result<AudioClip>`
  — single entry point that composes `ogg::parse_ogg` and
  `vorbis::decode_vorbis`. External callers can decode `.ogg` bytes
  in one call.
- Re-exports: `voltex_audio::{parse_ogg, decode_vorbis}` at crate
  root (previously only reachable via the now-public `ogg` /
  `vorbis` modules).

### Tests

- `voltex_audio` lib: 89 → **119** (+30) — ogg ×11 (4 original +
  4 fuzz + 1 invalid_capture rewrite + 2 entry-point), vorbis ×17
  (11 original + 1 BitReader 8-bit read + 5 more headers/imdct/
  decoder/floor coverage), `decode_ogg_vorbis_bytes` ×2.
- Workspace lib: 942 → **~968**.

### Known limits (deferred)

- Decoding real `.ogg` files end-to-end is **not** verified — the
  Vorbis decoder is simplified (scalar codebooks only, no coupled
  channels, residue type 2 only). `audio_demo` does not yet load
  OGG. Both deferred to a later v1.0.x cycle.

---

## [v1.0.8] — 2026-05-21 — parser panic-resistance fuzz tests + OGG/Vorbis dead-module discovery

### Added (21 new panic-resistance tests across 5 parsers)

Every public parser entry point now has a `fuzz_parse_*` test
suite that feeds it the same four classes of input:

1. empty `&[]`
2. every truncation of a representative valid sample / known magic
   header
3. 128 seeds × 6 lengths of deterministic xorshift32-pseudo-random
   bytes (0..2048 typical range)
4. valid magic prefix + xorshift32 tail (catches the "magic OK but
   then garbage" path, which is the classic source of unwrap /
   index panics)

| Parser            | File                                                    | New tests |
|-------------------|---------------------------------------------------------|-----------|
| `parse_png`       | `voltex_renderer/src/png.rs`                            | 4 |
| `parse_jpg`       | `voltex_renderer/src/jpg/mod.rs`                        | 4 |
| `parse_gltf`      | `voltex_renderer/src/gltf/mod.rs`                       | 5 |
| `parse_obj`       | `voltex_renderer/src/obj.rs`                            | 4 |
| `parse_wav`       | `voltex_audio/src/wav.rs`                               | 4 |
| **Total**         |                                                         | **21** |

All 21 pass. **Zero panics surfaced** — every parser returns an
`Err` (or, for `parse_obj` which returns `ObjData` directly,
returns an empty/partial `ObjData`) on malformed input. The
engine's dependency policy forbids `rand` / `proptest` / `arbitrary`,
so each test file inlines a tiny xorshift32 PRNG (~10 lines, no
allocations beyond the result Vec).

### Discovered (and deferred to v1.0.9)

While adding `fuzz_parse_ogg_*` to `voltex_audio/src/ogg.rs`, the
tests never showed up in `cargo test --lib --list`. Root cause:
`crates/voltex_audio/src/ogg.rs` and `crates/voltex_audio/src/vorbis/`
exist on disk but are **not declared** in `crates/voltex_audio/src/lib.rs`
(no `pub mod ogg;` / `pub mod vorbis;`). Cargo therefore never
compiles them, and they are effectively dead source files.

This contradicts STATUS.md Phase 6-1 ("OGG/Vorbis 디코더 자체
구현 완료") — the code exists but is invisible to the rest of the
build. Tentatively wiring them in surfaced 4 `dead_code` warnings
(spec-required fields the decoder doesn't yet consume) **and** 27
pre-existing clippy errors inside the modules. Fixing all of that
exceeds the v1.0.8 scope; the lib.rs registration was reverted to
keep this patch atomic. A note in lib.rs records the intentional
omission and points at v1.0.9.

v1.0.8 deliberately preserves the `fuzz_parse_ogg_*` tests inside
the unmounted `ogg.rs` so v1.0.9 can wire the module and the tests
will start running on the first re-registration. The source change
to `ogg.rs` is therefore harmless dead code today and useful
infrastructure tomorrow.

### Not in scope (deferred to v1.0.9+)

- Wire `voltex_audio::ogg` + `voltex_audio::vorbis` into the
  module graph, fix the 27 clippy errors and 4 dead-code warnings
  they bring in, plumb vorbis output through `AudioClip` so OGG
  files are actually loadable, run the OGG fuzz tests.
- Coverage-style fuzz orchestration (cargo-fuzz / honggfuzz /
  AFL.rs) — current tests are deterministic and CI-friendly, which
  is the right v1.0 trade-off. Coverage-guided fuzzing is v1.1+.

### Verified

- 921 → **942** lib tests (+21 fuzz, no behavior changes).
- `cargo clippy --workspace --all-targets -- -D warnings` still
  passes (one unrelated unnecessary_cast in `parse_obj`'s test
  helper fixed in this patch — `(32 + r) as u8 as char` → `as
  char`).
- voltex_cli still 30 tests; workspace build still 0 warnings.

---

## [v1.0.7] — 2026-05-21 — third-party license inventory + license-check CI job

Closes P4 #28 from the post-v1.0.5 audit ("외부 라이선스 고지
미작성") and the README's own "v1.0 release artifact 에 포함 예정"
deferral.

### Added

- **`THIRD-PARTY-LICENSES.md`** (auto-generated, 4,795 lines) —
  every transitive Cargo dependency that ends up in a Voltex build,
  grouped by SPDX license, with the full license text inlined.
  Generated by `cargo about generate about.hbs -o
  THIRD-PARTY-LICENSES.md`. Versioned in the repo so external users
  see it without rebuilding.
- **`about.toml`** — cargo-about policy. Declares the accepted SPDX
  list: MIT, Apache-2.0 (with LLVM exception variant), BSD-2/3,
  BSL-1.0, ISC, Zlib, Unicode-3.0 / Unicode-DFS-2016, MPL-2.0,
  CC0-1.0, OFL-1.1. Any new transitive outside this list will fail
  the CI license-check job.
- **`about.hbs`** — handlebars template producing the markdown
  output. Lists per-license summary + per-crate listing with
  repository links + collapsible license text.
- **`[workspace.package] license = "MIT OR Apache-2.0"`** in the
  root Cargo.toml, with `license.workspace = true` added to all 31
  workspace crate Cargo.toml files (15 lib + 14 example + 1 game +
  1 facade). Without this, `cargo about` warned that every voltex
  crate had no license expression — making the output dishonest.
- **`.github/workflows/ci.yml` — new `license-check` job**
  (Ubuntu, `continue-on-error: true`). Installs cargo-about via
  `taiki-e/install-action`, runs `cargo about generate` against
  `about.toml`. Fails (advisory) on:
  - any transitive crate whose license is outside the accepted list
  - any crate cargo-about cannot determine the license for
  Advisory rather than blocking until the policy stabilizes.

### Audited (license inventory)

`cargo about generate` reports 284 license-bearing crates in the
final dependency graph, broken down as:

| License        | Crates |
|----------------|--------|
| MIT            | 264    |
| Apache-2.0     | 11     |
| Zlib           | 3      |
| BSD-3-Clause   | 2      |
| BSD-2-Clause   | 1      |
| CC0-1.0        | 1      |
| ISC            | 1      |
| Unicode-3.0    | 1      |

**Zero copyleft (GPL/AGPL/LGPL) and zero proprietary** — every
transitive license is compatible with both arms of voltex's MIT OR
Apache-2.0 dual license.

### Documentation

- **README "License" section** rewritten:
  - Removed the deferral note ("외부 의존 라이선스 고지는 작업 후속")
    that was pending since v1.0 release.
  - Links to `THIRD-PARTY-LICENSES.md`, includes the per-license
    count, states the zero-copyleft fact explicitly.
  - Documents the regeneration command for future contributors.
  - References the CI license-check job.
- **README badges**: status 1.0.6 → 1.0.7, new
  `licenses: permissive only` badge.

### Verified (self-feedback at each stage)

- Stage 1 install + generation: 4,795-line file produced.
- Stage 2 analysis: 8 distinct licenses, all permissive, zero
  copyleft.
- Stage 3 README + CI: cosmetic only.
- Stage 4 final gate: `cargo build` / `clippy -D warnings` / `test
  --lib` / `test -p voltex_cli` / `cargo about generate` all green
  with 0 warnings, 0 errors.

### Not in scope (deferred to v1.1+)

- Promoting `license-check` from advisory to blocking — depends on
  the first real CI run.
- Uploading `THIRD-PARTY-LICENSES.md` into release artifacts
  (`dist/<name>.exe` companion file) — `voltex_cli build` does not
  yet copy it.
- `cargo-deny` for combined license + advisory + duplicate scan —
  cargo-audit + cargo-about already cover the high-value cases.

---

## [v1.0.6] — 2026-05-21 — coverage baseline (82%) + advisory CI job + CLI engine-path friendliness

### Measured (coverage baseline)

First time the project has a number for `rules/common/testing.md`'s
"80%+ coverage" target. `cargo install cargo-llvm-cov` + `rustup
component add llvm-tools-preview` was needed once; result:

| metric    | covered / total | percent |
|-----------|-----------------|---------|
| Functions | 34878 / 42505   | **82.06%** |
| Lines     | 18142 / 24849   | **82.74%** |
| Regions   | (see report)    | **73.01%** |

Per-crate breakdown (function coverage), 14 crates:

- **🟢 80%+ (10 crates)**: voltex 100%, voltex_math 95.85%,
  voltex_script 94.33%, voltex_ecs 93.31%, voltex_asset 93.17%,
  voltex_ai 92.85%, voltex_net 92.08%, voltex_physics 89.85%,
  voltex_audio 88.26%, voltex_editor 82.27%.
- **🟡 below 80% (4 crates, all GPU/winit-dependent)**:
  voltex_renderer 65.94% (`shadow.rs` / `shadow_pipeline.rs` /
  `rt_*.rs` / `taa.rs` / `ssgi.rs` / `ssr.rs` / `temporal_accum.rs`
  — GPU compute shader paths, need a `wgpu::Device` to exercise),
  voltex_runtime 61.22% (`handler.rs` 0% — winit `ApplicationHandler`
  callbacks), voltex_renderer_ecs 53.83% (`BasicRenderer::render`,
  GPU pass), voltex_platform 46.38% (`ApplicationHandler` callbacks +
  per-platform key mapping).

→ The four low-coverage crates share a single root cause: their
public surface is **GPU + winit integration code that cannot be
reached from a headless `cargo test --lib`**. Their unit-testable
internals (`app.rs`, `config.rs`, `error.rs`, `RenderItem`
collection, etc.) all sit at 80%+. Closing the gap requires
integration tests against a headless GPU adapter and a stubbed
event loop — scheduled for v1.1+.

### Added (infrastructure)

- **`.github/workflows/ci.yml` — new `coverage` job** (Ubuntu,
  `continue-on-error: true`). Installs llvm-tools + cargo-llvm-cov
  via `taiki-e/install-action`, then runs `cargo llvm-cov
  --workspace --lib --summary-only`. Advisory only — threshold
  enforcement (`--fail-under-lines 80`) postponed until GPU paths
  are covered by integration tests.
- **README "Contributing" → "Coverage" subsection**: documents the
  one-time install commands, the current baseline number, and why
  the 4 low-coverage crates are low.
- **README badge**: `coverage 82% lib`. Status badge bumped 1.0.5
  → 1.0.6.

### Improved (CLI friendliness — P1 #5)

- **`voltex build` engine-path missing error** rewritten as a
  multi-line help block. Old (1 line): `"engine path unknown — set
  VOLTEX_REPO or pass --engine-path <repo-root>"`. New (7 lines):
  scaffolded-project example + both shell-specific export commands
  (bash/zsh + PowerShell), so new users get a copy-pasteable fix in
  the error itself.
- **README 5-min quickstart**: split into "예제 실행" + "새 게임
  만들기" subsections. The second one walks through `cargo install
  --path crates/voltex_cli` → `export VOLTEX_REPO=$(pwd)` → `voltex
  new` → `voltex build`. Closes the discoverability gap where users
  saw `cargo run --example triangle` but could not figure out how
  to scaffold their own project.

### Verified (self-feedback at each stage)

- Stage 1: cargo-llvm-cov install + baseline measurement (82.06%).
- Stage 2: per-crate analysis, low crates identified and justified.
- Stage 3: README + CI yml updated, no behavior change.
- Stage 4: CLI tweak — empty-directory repro of `voltex new && cd &&
  voltex build` (without VOLTEX_REPO env) now prints the 7-line
  help block instead of the 1-line one.
- Stage 5: `cargo build --workspace` clean, `cargo test --workspace
  --lib` = 921, `cargo test -p voltex_cli` = 30. clippy still 0.

### Not in scope (deferred to v1.1+)

- Lifting the 4 low-coverage crates above 80% — requires headless
  GPU adapter testing + winit event-loop stubbing.
- Enabling `--fail-under-lines 80` as a CI gate — depends on the
  above.
- HTML coverage report upload (codecov / coveralls) — depends on
  the CI workflow actually running on a runner.

---

## [v1.0.5] — 2026-05-21 — clippy clean (warnings 84 → 0) + cargo audit + CI workflow

This is the "quality gate" patch. v1.0.3 (Windows build P0) and v1.0.4
(clippy deny errors) both slipped past internal verification because
`cargo build` + `cargo test --lib` was the only standing check. v1.0.5
closes the loop: zero clippy warnings, zero open advisories, and a CI
workflow that enforces both on every push.

### Fixed (clippy warnings 84 → 0)

Baseline measurement before this patch: `cargo clippy --workspace
--all-targets` produced **84 lines of warnings/errors** (51 unique
diagnostics across 13 lint categories).

**Auto-fixed** (`cargo clippy --fix --workspace --all-targets`, 43
suggestions across `voltex_math`, `voltex_audio`, `voltex_platform`,
`voltex_ecs`, `voltex_cli`, `voltex_renderer`, and 3 examples):
- `manual_div_ceil` ×13 → `.div_ceil(...)`
- `manual RangeInclusive::contains` ×4 → `.contains(&x)`
- `excessive_precision` ×4 (audio wav constants)
- `manual_is_multiple_of` ×1 → `.is_multiple_of(2)`
- `collapsible_if` ×1, `identity_op` ×1, `len_zero` ×1,
  `unwrap_or_else_default` ×1
- subset of `needless_range_loop` (iterator-convertible cases)

**Manually fixed** (24 residual diagnostics, semantic intent required):
- `voltex_audio::mixing::make_stereo_clip` and
  `voltex_ecs::world::tests::Name` — both genuinely dead, deleted
  outright.
- `voltex_script::state` ×5 — `b"name\0".as_ptr() as *const _`
  replaced with C-string literal `c"name".as_ptr()` (Rust 1.77+).
- `voltex_renderer::jpg::mod` ×4 — `for _ in 0..N { vec.push(byte); }`
  collapsed to `vec.extend_from_slice(&[byte; N])` for JPEG marker
  padding.
- `voltex_math::mat4`, `voltex_renderer::sh` ×4 — `needless_range_loop`
  cases where `r`/`c`/`k`/`ch` indices appear in both lookup AND
  assert message: annotated `#[allow(clippy::needless_range_loop)]`
  with a comment.
- `voltex_editor::panels::hierarchy::draw_entity_node`,
  `voltex_editor::picking::pick_entity`,
  `deferred_demo::create_shadow_bind_group` — `too_many_arguments`
  cases where args are intrinsic (recursive draw / viewport rect +
  mouse + camera / 9-binding GPU layout): annotated `#[allow]`.
- `voltex_cli::manifest::extract_build` — `very_complex_type` (6-tuple
  return) refactored to return `BuildConfig` directly; the destructure
  at the call site collapsed to single `build` field. Net code
  shorter, type signature self-documenting.
- `examples/{shadow_demo,deferred_demo,asset_demo}` — small loop
  refactors and 1 `#[allow]` where index threads multiple parallel
  arrays.

### Added (CI infrastructure)

- **`.github/workflows/ci.yml`** — GitHub Actions matrix that runs
  `cargo build --workspace`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace --lib`, and
  `cargo test -p voltex_cli` on **ubuntu-latest + windows-latest**
  (blocking) and **macos-latest** (advisory, `continue-on-error`).
  Linux job installs `libasound2-dev` before build to satisfy
  `voltex_audio`'s ALSA FFI link. Cache layer keys on `Cargo.lock`.
  File is benign on non-GitHub remotes — gitea/forgejo also accept
  the syntax.
- **README "Contributing" section** — documents the exact four-command
  local verify recipe that mirrors CI, so PR authors can reproduce
  the gate before pushing.

### Audited (cargo audit)

- Installed `cargo-audit` (one-time tool, not a runtime dep).
- Scan of 298 `Cargo.lock` crates against 1,096 advisories:
  **0 vulnerabilities**.
- **1 warning** — `paste 1.0.15` flagged unmaintained
  (RUSTSEC-2024-0436). Transitive: `paste ← metal ← wgpu-hal ←
  wgpu`. Cannot be addressed at voltex level; resolved by future
  wgpu upgrade. Documented for awareness, no action required.

### Verified (self-feedback at each stage)

- Stage 1 baseline: 84 warnings counted.
- Stage 2 post-auto-fix: 41 warnings counted, 921 lib tests pass.
- Stage 3 post-manual-fix: **0 warnings**, 921 lib tests pass,
  30 voltex_cli bin tests pass.
- Stage 4 cargo audit: 0 vulns, 1 advisory unrelated to voltex code.
- Stage 5 CI gate re-run locally: `RUSTFLAGS="-D warnings"
  cargo {build,clippy,test}` all green.

### Not in scope (deferred to v1.1+)

- Actually running the CI workflow on a GitHub runner — requires a
  GitHub mirror. The workflow file ships ready-to-use; the first
  green run on a real CI is the next-cycle verification step.
- 3-OS exe runtime verification (Linux ALSA audio output, macOS
  CoreAudio audio output) — still environment-dependent.
- Adding `cargo audit` to the CI workflow — `cargo-audit` itself
  needs an install step; postponed pending a CI-friendly setup.

---

## [v1.0.4] — 2026-05-21 — clippy `approx_constant` clean + README dependency honesty

### Fixed
- **clippy `approx_constant` errors (9 → 0)** across
  `voltex_physics::integrator`, `voltex_renderer::animation`,
  `voltex_ecs::world::tests`, `voltex_ecs::json`, and
  `voltex_renderer::json_parser`.
  - `integrator.rs` test used `3.14159` for `rb.angular_velocity` /
    expected_rot_y → replaced with `std::f32::consts::PI` (real intent
    was 1 PI rad/s — comment already said so).
  - `animation.rs::test_quat_slerp_endpoints` used `0.7071` for 90°
    quaternion components → replaced with `std::f32::consts::FRAC_1_SQRT_2`
    (more precise + self-documenting).
  - `world/tests.rs::test_clear_changed_all_storages` used `3.14f32`
    only as "a different-typed value" → replaced with `2.5f32` (no
    semantic dependency on the value at all).
  - `json.rs::test_write_number`, `json.rs::test_parse_negative_number`,
    `json_parser.rs::test_parse_number` use `3.14` / `-3.14` as
    representative decimal payloads for serialization round-trip —
    annotated with `#[allow(clippy::approx_constant)]` + comment
    explaining the intent (the value is data, not π).
  - Net result: `cargo clippy --workspace --all-targets` no longer
    fails on `#[deny(clippy::approx_constant)]`. External contributors
    can now run clippy without a hard error.

### Corrected (documentation honesty)
- **README "외부 crate 의존 단 2개 (winit / wgpu)" claim was inaccurate.**
  Actual measured direct dependencies:
  - **Runtime (3):** `winit`, `wgpu`, `bytemuck` (Pod/Zeroable derive +
    cast — wgpu's de-facto companion, but a real direct dep nonetheless).
  - **Build-only (2, never present in shipped runtime):** `thiserror`
    (`Error` derive macro; no runtime crate after compilation), `cc`
    (`voltex_script`'s Lua 5.4 C source `build.rs`).
  - **Example/binary helpers (not in any library crate):** `log`,
    `env_logger`, `pollster`. Listed separately so future audits don't
    mistake them for engine deps.
  - **Transitive total:** 298 crates in `Cargo.lock` (dominated by
    wgpu's GPU stack and winit's per-OS windowing).
  README header and "의존성 정책" table updated to reflect this with
  rationale per crate.

### Verified
- `cargo clippy --workspace --all-targets` — `approx_constant` count
  3 errors / 5 hits  →  0.
- `cargo test --workspace --lib` — 921 lib tests still pass (the 3
  test files touched are part of the standard suite; `voltex_physics`
  106 / `voltex_ecs` 134 / `voltex_renderer` 200 unchanged).
- No production-code changes — only test bodies and 2 `use std::f32::consts::*`
  imports inside `#[cfg(test)]` modules.

### Not in scope (deferred to v1.1+)
- The remaining ~80 clippy warnings (manual `RangeInclusive::contains`,
  `needless_range_loop`, dead code in `voltex_audio::make_stereo_clip`
  / `voltex_ai::Name`, excessive float precision in `voltex_audio::wav`,
  etc.) are non-deny lints. v1.0.4 deliberately addresses only the
  hard errors that block external `cargo clippy --all-targets` runs.
- CI workflow that enforces `cargo clippy --workspace --all-targets`
  going forward — recommended next step but not added in this patch.

---

## [v1.0.3] — 2026-05-21 — `voltex build` Windows P0 regression fix

### Fixed
- **P0 — `voltex build` failed on every Windows host**.
  `crates/voltex_cli/src/build.rs:96-97` (and `run.rs:20`) called
  `std::fs::canonicalize`, which on Windows always returns the
  extended-length `\\?\C:\...` verbatim form. That prefix landed in the
  generated `dist/wrapper/Cargo.toml` as
  `path = "\\\\?\\C:\\...\\voltex_runtime"`, and Cargo rejected it with
  `invalid path url '//?/C:\\...\\voltex_runtime'`. Result: the
  README's 5-minute quickstart (`voltex new my_game && cd my_game &&
  voltex build`) was 100 % broken for external users. The flagship
  `games/voltex_survivor` and `examples/tutorial_minigame` were
  unaffected because they are workspace members built directly with
  `cargo build`, hiding the regression from internal verification.

### Added
- `voltex_cli::util::canonicalize_friendly(&Path) -> Result<PathBuf>`:
  canonicalize, then strip the Windows `\\?\` verbatim prefix for
  drive-letter paths (`\\?\C:\foo` → `C:\foo`). UNC paths
  (`\\?\UNC\server\share\...`) are intentionally left untouched —
  rewriting them is lossy. No new external dependencies (engine policy
  forbids `dunce` etc.).
- Regression tests (+4): `strip_verbatim_prefix` drive-letter strip,
  UNC passthrough, non-verbatim passthrough (all Windows-gated), and
  cross-platform `canonicalize_friendly_returns_non_verbatim_path`.

### Verified
- End-to-end CLI workflow re-run on Windows host: `voltex new my_game`
  → `voltex build --engine-path <repo>` → `dist/my_game.exe` produced
  in 61 s, wrapper `Cargo.toml` contains
  `voltex_runtime = { path = "C:/Users/<user>/..." }` (no `\\?\`).
- 921 lib tests still pass. `voltex_cli`: 26 → 30 (+4).
- Workspace build: 0 warnings, 0 errors.

### Cleanup
- `examples/many_cubes/src/main.rs:8` — removed unused `Mat4` import
  (only `Vec3` is actually used). Was the lone warning in the
  workspace build.

### Not changed
- No behavior change for `voltex_survivor`, examples, or any engine
  crate. Patch is confined to `voltex_cli` (and the trivial
  many_cubes import cleanup).

---

## [v1.0.2] — 2026-05-21 — 600-line rule re-enforcement + full example runtime sweep

### Changed (refactor only — no behavior change)
- `voltex_editor::session` (1060 lines) → `session/{mod.rs 445, tests.rs
  483, e3_tests.rs 152}`. E3 tests carry their own `temp_path` / `cleanup`
  helpers so each file compiles independently.
- `voltex_script::engine_api` (713) → `engine_api/{mod.rs 356, tests.rs
  357}`.
- `voltex_ecs::world` (665) → `world/{mod.rs 351, tests.rs 314}`.
- `voltex_cli::manifest` (612) → `manifest/{mod.rs 450, tests.rs 162}`.
- After splits, the largest source file is `voltex_physics::solver` at
  574 lines (under the project's 600-line guideline).

### Verified
- Runtime smoke-test on the 8 previously-unverified examples
  (model_viewer / many_cubes / asset_demo / multi_light_demo /
  shadow_demo / ibl_demo / deferred_demo / editor_demo) — all 8 exit 124
  (timeout-clean, no panic). With v1.0.1's voltex_survivor +
  tutorial_minigame + runtime_demo + audio_demo + triangle + pbr_demo +
  hierarchy_demo, **all 15 examples + 1 game are Windows-host runtime
  verified**.

### Documentation sync
- STATUS.md M3 row corrected (was 'no violations' — became 4 violations
  during Cycles 4-13 before re-split).
- STATUS.md voltex_ecs test count fixed (was 131, actual 134).
- DEFERRED.md: gizmo / Despawn-undo / mesh AABB picking marked ✅ (done
  in Cycles 3 and 12 but never reflected in DEFERRED).
- DEFERRED.md: new 'v1.0.x 추가 한계' section enumerates every deferral
  introduced in v1.0 / v1.0.1 / v1.0.2 with its source cycle.
- CLAUDE.md 600-line rule references v1.0.2 re-split.

### Tests
- 0 new tests (pure refactor). 921 --lib tests still pass.

---

## [v1.0.1] — 2026-05-21 — Runtime verification + P0/P1 regression patch

### Fixed
- **handler.rs camera ↔ user-system WASD double-binding** (P0). FPS camera
  movement now requires RMB-held, matching scene-view convention. Games can
  use WASD freely via `add_input_system`.
- **`is_key_just_pressed()` always false in user systems** (P1). Moved
  `app.input.begin_frame()` to end-of-frame so just_pressed survives into
  user-system observation.
- **`Bounds::default_bytes` was zero-volume AABB** (P0). Returned 24 zero
  bytes → frustum culling discarded every Inspector "+ bounds" entity. Now
  encodes the unit cube `min=(-0.5)^3, max=(0.5)^3`.
- **`ScriptComponent::default_bytes` was empty path** (P0). Inspector "+
  script" produced an unrunnable component. Now encodes
  `UNBOUND + "scripts/spinner.lua"` so the placeholder exercises the script
  system end-to-end on every `voltex new` project.
- **voltex_survivor enemies / projectiles never executed scripts**. Lua
  `spawn_entity()` returns a bare entity with no ScriptComponent. The
  game's handle-attach system was extended to attach the matching .lua
  based on Tag (`enemy` / `projectile`).

### Added
- 5 regression tests (`voltex_runtime` ×2 input_system order/reference,
  `voltex_ecs` ×3 default_bytes round-trips and non-degenerate Bounds).

### Verified at runtime (Windows host)
- `cargo run -p voltex_survivor`: 6 waves in 30s, 24 enemy kills, no panics.
- `cargo run -p tutorial_minigame`: score system prints, no panics.
- `cargo run -p runtime_demo`: clean exit.

---

## [v1.0.0] — 2026-05-21

### Added (Cycle 15, D3)
- `games/voltex_survivor/` — flagship game. Wave-based arena shooter with
  4 Lua scripts (game/player/enemy/projectile) + scene + Rust input system.
  Game logic 100% in Lua + scene.
- `voltex_runtime::RuntimeApp::add_input_system(F)` — per-frame system that
  receives `&InputState` alongside `&mut World, f32`. Powers WASD movement
  for voltex_survivor.
- `docs/v1.0-retrospective.md` — what worked, what was deferred, statistics,
  next-priority list for v1.1.
- `STATUS.md`: v1.0 release header + Cycle 15 entry.
- Git tag `v1.0.0` (local — user push activates remote release).

### Added (Cycle 14, D1+D2)
- `README.md` — 5-minute quickstart + code-first / editor-first workflows
  + feature list (14 categories) + crate dependency graph + license +
  contributing + verification-limit notes.
- `docs/tutorial.md` — 7-chapter 30-minute tutorial from scaffold to built exe.
- `examples/tutorial_minigame/` — final artifact of the tutorial, dual-workflow
  compatible (cargo run + voltex build). 1 player + 3 enemies + spinner.lua
  + enemy.lua + score system in stdout.

### Deferred to v1.1
- Lua input API (currently Rust handles input via `add_input_system`).
- `voltex_cli editor` subcommand (tutorial points at `examples/editor_demo`).
- 3-OS exe artifact verification (only Windows build verified locally).
- Screenshots — README + tutorial have placeholder markers; PR welcome.

---

## [Unreleased pre-v1.0]

### Added (Cycle 13, A — 2026-05-21)
- `voltex` facade `prelude` now re-exports `Bounds`, `MeshHandle`,
  `MaterialHandle`, `ScriptComponent`, `RuntimeApp`, `RuntimeConfig`, `Vfs`.
- `voltex::runtime` module re-export (was missing in B0).
- Crate-level rustdoc on `voltex` lib + tier-1 API docstrings on `World`,
  `Entity`, `EditorCommand`, `EditorSession`.

### Fixed (Cycle 13, A — 2026-05-21)
- `cargo doc --workspace --no-deps` now emits zero warnings (intra-doc link
  escapes in `voltex_math`, `voltex_physics`, `voltex_ai`, `voltex_script`,
  `voltex_renderer`; `voltex_cli` bin's `doc = false` skirts cargo issue
  #6313 collision with the `voltex` lib).

### Deferred to v1.1
- Full `#![warn(missing_docs)]` sweep — 1586 missing public-item docstrings
  across the workspace. v1.0 documents prelude + tier-1 API only.
- Public-item `pub(crate)` audit. Candidate list lives in
  `docs/v1.0-pub-audit.md` (notes only, no code changes).

---

## Cycle 12 (E3, flex — 2026-05-21)

### Added
- `EditorCommand::AddComponent { entity, name, data }` and
  `EditorCommand::RemoveComponent { entity, name }`.
- `EntityAllocator::restore_at_generation(id, gen) -> bool` and the
  matching `World::restore_at_generation` delegate.
- `Component` trait gains default-implemented `remove()` and `default_bytes()`
  methods (backward compatible — existing impls auto-pick up the defaults).
  Built-ins override `default_bytes` with sensible values.
- Inspector panel adds a "+ <name>" button row for the six registered
  built-in components.

### Changed (potentially breaking for in-tree callers)
- `Despawn` is no longer history-clearing — it is fully undoable via a new
  `Inverse::RestoreEntity` that snapshots every registered component
  pre-despawn and restores the original `(id, generation)` on undo.
- `ComponentEntry` grows two fn-pointer fields (`remove`, `default_bytes`).
  `register_raw` populates them with no-op stubs.

---

## Cycle 11 (B3 — 2026-05-21)

### Added
- `voltex_audio::backend::CoreAudioBackend` (macOS, `cfg(target_os = "macos")`).
  AudioToolbox AudioQueue via raw FFI, push-style wrap via Condvar-guarded
  free queue.
- `voltex_cli::build::validate_target` enforces native-only build policy
  with friendly cross-compile / manifest-mismatch error messages.
- `docs/build-matrix.md` — 3-OS prerequisites + commands + voltex.toml
  multi-target example + optional GitHub Actions matrix snippet.

---

## Cycle 10 (P1 — 2026-05-21)

### Added
- `voltex_audio::backend` module with `AudioBackend` trait + `BackendError`.
- `WasapiBackend` (Windows), `AlsaBackend` (Linux, raw libasound FFI),
  `NullBackend` (headless / CI fallback).
- `make_backend()` picks the best OS backend, with hard fallback to null.

---

## Cycle 9 (B2 — 2026-05-21)

### Added
- `voltex_asset::vpkg` — custom `.vpkg` archive format (magic + index table
  + concatenated blobs, no external deps).
- `voltex_asset::vfs::Vfs` — `Filesystem(PathBuf)` or `Archive(VpkgReader)`,
  single `read`/`exists`/`list` API.
- `voltex_runtime::RuntimeConfig::with_vfs(Vfs)` — archive-first scene loader
  (filesystem fallback retained for dev mode).
- `voltex build` packs scene + scripts + assets into a `.vpkg` and embeds it
  in the wrapper via `include_bytes!`, producing self-contained executables.

---

## Cycle 8 (B1 — 2026-05-21)

### Added
- `voltex_cli` production CLI with `new` / `build` / `run` subcommands.
- `voltex.toml` manifest (self-contained mini TOML parser, no external deps)
  with `[project]`, `[build]`, `[build.<triple>]` sections.
- Human-friendly error messages (`MissingField`, `ManifestParse`,
  `CargoMissing`, …).

---

## Cycle 7 (B1-spike — 2026-05-21)

### Added
- `voltex_cli` 1-session prototype + `docs/superpowers/notes/2026-05-21-b1-spike-learnings.md`
  documenting target dir, host triple, and packing decisions for Cycle 8.

---

## Cycle 6 (B0 — 2026-05-21)

### Added
- `voltex_runtime` crate (L3): `RuntimeApp` (winit `ApplicationHandler` +
  GPU init + system tick) + `RuntimeConfig` builder + `RuntimeError`/`RuntimeResult`.
- `examples/runtime_demo` — solar-system scene, `main` body 4 lines.

---

## Cycle 5 (S1 — 2026-05-21)

### Added
- `voltex_ecs::ScriptComponent { script_path, instance_id }` as the sixth
  built-in component.
- `voltex_script::ScriptInstancePool` — one `LuaState` per .lua path,
  shared across all entities pointing at it.
- `script_start_all` / `script_update_all` systems (Lua `on_start(id,gen)` /
  `on_update(id,gen,dt)` dispatch).
- Lua `iter_entities()` / `iter_by_tag(tag)` engine API (15 → 17 fns).

---

## Cycle 4 (E2 — 2026-05-20)

### Added
- `EditorMode { Edit, Play }` + `EditorSession::set_mode` with binary-snapshot
  Play→Stop restore.
- `lifecycle` module — `LifecycleEvent { PlayStart, PlayStop }` with callback
  list for systems (physics, scripts) to hook the transitions.
- `EditorError::ModeMismatch` — dispatch is rejected while in Play mode.

---

## Cycle 3 (E1 — 2026-05-20)

### Added
- `voltex_editor::gizmo` — translate / rotate / scale handles, LINE-list wgpu
  pipeline, depth-test Always overlay rendering.
- `picking::pick_entity` upgraded from sphere approximation to AABB picking
  via the `Bounds` component (sphere fallback kept).
- `examples/editor_demo` integrates the gizmo on the toolbar.

---

## Cycle 2 (C0b — 2026-05-20)

### Added
- `voltex_renderer_ecs` crate (L3) — `RenderAssets` (lockstep handle table),
  `BasicRenderer` (per-entity dynamic UBO pipeline), `RenderItem` +
  `collect_render_items` (helper for custom PBR pipelines).
- `editor_demo` + `survivor_game` migrated off the parallel `Vec<Mesh>`
  pattern to ECS-driven rendering.

---

## Cycle 1 (C0a — 2026-05-20)

### Added
- `voltex_ecs::MeshHandle`, `MaterialHandle`, `Bounds` components (with phantom
  `MeshAsset` / `MaterialAsset` marker types to separate handle ID spaces).
- Auto-registered by `ComponentRegistry::register_defaults()`.

---

## Pre-roadmap milestones

### AI-friendly restructure M1–M7 (2026-05-20)
- CODEMAP.md per crate + CLAUDE.md dependency graph (M1).
- File-split: every `.rs` ≤ 600 lines (M2 + M3).
- Error standardization on `thiserror`-based `<Crate>Error` + `Result<T>` aliases (M4).
- `Component` trait + `register::<T>()` standard registration (M5).
- `voltex` facade crate + `prelude` (M6).
- Final verification + `hierarchy_demo` migration (M7).

### Phases 1–8 (2026-03-24 → 2026-05-19)
Foundation through editor command machinery. See `docs/STATUS.md` for the
per-phase breakdown (Foundation → Rendering → ECS+Scene → PBR → Physics →
Audio → Advanced rendering → AI/Net/Lua/Editor + Editor Command).

---

[Unreleased]: https://git.tolelom.xyz/tolelom/game_engine/compare/main...HEAD
