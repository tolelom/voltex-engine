# Cycle 2 (C0b) — Renderer 브릿지 + 첫 ECS-driven 렌더링 설계

**작성일**: 2026-05-20
**상위 로드맵**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 2
**Sub-project**: C (Core bridge)
**세션 수**: 2
**의존**: Cycle 1 (C0a — `MeshHandle`/`MaterialHandle`/`Bounds` 컴포넌트가 voltex_ecs 에 존재)

---

## 1. 목적

ECS World 의 엔티티가 매 프레임 자동으로 렌더링되도록 voltex_ecs 와 voltex_renderer 를 잇는 신규 crate `voltex_renderer_ecs` 를 만든다. editor_demo / survivor_game 의 평행 `Vec<Mesh>` / `Vec<Mat4>` / `Vec<material>` 구조를 ECS-driven 으로 마이그레이션하여, **Transform 컴포넌트 변경 → 다음 프레임 뷰포트에 반영** 이라는 단방향 데이터 흐름을 확립한다.

## 2. Scope / Out-of-scope

### 2.1 In-scope

- 신규 crate `voltex_renderer_ecs` (L3):
  - `RenderAssets` — `voltex_renderer::Mesh` + `MaterialUniform` 을 보관하는 thin wrapper. 반환되는 `MeshHandle` / `MaterialHandle` (C0a 의 phantom-typed) 는 내부의 `Handle<Mesh>` / `Handle<MaterialUniform>` 과 id/generation 이 lockstep.
  - `BasicRenderer` — pipeline + per-entity dynamic UBO + bind groups 를 소유. `render(...)` 한 메서드로 World 를 그린다. mesh_shader.wgsl 기반 (Blinn-Phong, 2 bind group).
  - `collect_render_items(world, assets) -> Vec<RenderItem>` — pure helper. 자체 PBR pipeline 을 쓰는 호출자(survivor_game) 가 평행 Vec 대신 호출.
  - `RenderItem { entity, model, mesh, material }` — 한 엔티티의 한 프레임 렌더 데이터.
- `voltex` facade 에 `voltex::renderer_ecs` 모듈 re-export. prelude 는 변경 없음.
- editor_demo: `scene_meshes: Vec<Mesh>` 제거, `RenderAssets` + 단일 unit cube mesh 공유, 시드 엔티티에 MeshHandle/MaterialHandle 부착, 렌더 루프를 `BasicRenderer::render` 한 줄로 대체.
- survivor_game: arena/player/projectile/enemy 모두 ECS 엔티티화. `Game` 이 entity ID 를 보관하고 Transform.position 을 업데이트. 매 프레임 `collect_render_items` 로 dynamic UBO staging 데이터 생성.
- 단위 테스트 6개 (`voltex_renderer_ecs` lib).
- `crates/voltex_renderer_ecs/CODEMAP.md` 신규, `CLAUDE.md` L3 그래프 갱신, `STATUS.md` 갱신, 로드맵 cycle 2 진행 마커 갱신.

### 2.2 Out-of-scope (후속 cycle)

- Gizmo / picking 업그레이드 — Cycle 3 (E1) 에서 처리. C0b 의 시각 검증은 Inspector 슬라이더로 Transform.position 을 직접 수정하는 방식.
- Frustum culling — `Bounds` 컴포넌트는 등록만 되어 있고 render 시점에는 사용 안 함. v1+ 에서 도입.
- MeshHandle 변경 시 Bounds 자동 재계산.
- PBR 전용 `render_world` 통합 함수 — survivor_game 은 자체 PBR draw loop 를 유지하고 `collect_render_items` 만 호출.
- Editor 측 Mesh/Material 자원의 핫리로드 통합 (`FileWatcher` 와 `RenderAssets` 연동).

## 3. 아키텍처

### 3.1 의존성 그래프 갱신

```
이전 L3:
  voltex_editor → voltex_math, voltex_ecs, voltex_renderer

이후 L3:
  voltex_renderer_ecs (신규) → voltex_math, voltex_ecs, voltex_asset, voltex_renderer
  voltex_editor              → voltex_math, voltex_ecs, voltex_renderer
                                                       (voltex_renderer_ecs 는 의존 안 함, examples 만 사용)
```

`voltex_renderer_ecs` 는 voltex_renderer (L2) + voltex_ecs (L1) + voltex_asset (L0) + voltex_math (L0) 의 결합. L3 합당. facade 의 voltex crate (L4) 가 이를 모듈로 re-export.

### 3.2 ID 공간 매핑 — 핵심 결정

C0a 가 정의한 `MeshHandle(Handle<MeshAsset>)` 의 `MeshAsset` 은 **phantom marker** (unit struct, voltex_ecs 거주). voltex_asset 의 `Assets` 는 `TypeId<T>` 키 기반이라, `MeshAsset` 자체로는 `voltex_renderer::Mesh` 와 다른 저장소 슬롯에 들어간다.

해결: `RenderAssets` 가 두 Handle 의 raw 표현(id/generation)을 lockstep 으로 유지.

```rust
pub struct RenderAssets {
    assets: voltex_asset::Assets,  // 실제 voltex_renderer::Mesh + MaterialUniform 저장
}

impl RenderAssets {
    pub fn add_mesh(&mut self, mesh: voltex_renderer::Mesh) -> voltex_ecs::MeshHandle {
        let h: Handle<voltex_renderer::Mesh> = self.assets.insert(mesh);
        // id/generation 만 같고 phantom T 만 다른 핸들로 변환.
        voltex_ecs::MeshHandle(Handle::from_raw_parts(h.id(), h.generation()))
    }

    pub fn mesh(&self, h: voltex_ecs::MeshHandle) -> Option<&voltex_renderer::Mesh> {
        let raw: Handle<voltex_renderer::Mesh> = Handle::from_raw_parts(h.0.id(), h.0.generation());
        self.assets.get(raw)
    }

    pub fn add_material(&mut self, m: voltex_renderer::MaterialUniform) -> voltex_ecs::MaterialHandle { /* 동일 패턴 */ }
    pub fn material(&self, h: voltex_ecs::MaterialHandle) -> Option<&voltex_renderer::MaterialUniform> { /* 동일 패턴 */ }
}
```

**왜 이 패턴**: voltex_ecs 가 voltex_renderer 에 의존하지 못하는 L1 제약을 유지하면서, 외부 사용자에게는 단일 `RenderAssets` API 만 노출. 두 ID 공간 (ECS-side handle, renderer-side handle) 이 항상 동일 id/generation 을 갖는 invariant 는 `add_mesh` / `add_material` 가 유일한 진입점인 한 보장된다.

**Trade-off**: `Handle::from_raw_parts` 는 unsafe 표시 없이 임의 핸들 생성을 허용. 사용자가 직접 `MeshHandle(Handle::from_raw_parts(...))` 를 만들어 잘못된 ID 를 전달하면 `mesh()` 가 None 반환 (graceful). 동일 위험이 이미 scene 디시리얼라이즈에 존재 (C0a 의 deserialize 가 from_raw_parts 사용).

### 3.3 BasicRenderer — editor_demo 의 render path

`mesh_shader.wgsl` (Blinn-Phong) 을 그대로 사용. 2 bind group (group 0 = camera+light dynamic, group 1 = texture).

```rust
pub struct BasicRenderer {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,        // dynamic UBO, MAX_ENTITIES × aligned(CameraUniform)
    light_buffer: wgpu::Buffer,
    camera_light_bind_group: wgpu::BindGroup,
    default_texture: voltex_renderer::GpuTexture,  // 1×1 white
    aligned_size: u32,
    max_entities: usize,
}

impl BasicRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        max_entities: usize,
    ) -> Self { ... }

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        world: &mut World,                // &mut 필요 (propagate_transforms)
        assets: &RenderAssets,
        view_proj: voltex_math::Mat4,
        cam_pos: voltex_math::Vec3,
        light: voltex_renderer::LightUniform,
    );
}
```

내부 동작:
1. `voltex_ecs::propagate_transforms(world)` — WorldTransform 갱신
2. `let items = collect_render_items(world, assets)` — RenderItem 리스트
3. items.iter() → CameraUniform 으로 staging 빌드 (`{ view_proj, model = item.model.cols, camera_pos }`). MaterialUniform 은 BasicRenderer 가 무시 (mesh_shader.wgsl 이 머티리얼 안 씀).
4. `queue.write_buffer(camera_buffer, 0, staging)` + light_buffer 쓰기
5. render pass 시작 → set_pipeline → set_bind_group(1, default_texture) → 엔티티마다 set_bind_group(0, ..., &[dynamic_offset]) + set_vertex/index_buffer(item.mesh) + draw_indexed.

`render` 가 `&self` (mut 아님) 인 이유: GPU 자원 (pipeline, buffers) 은 frame 마다 mutate 안 됨. staging 작성은 staging Vec 을 stack/heap 에 만들고 queue.write_buffer 호출만 함.

`world: &mut World` 인 이유: `propagate_transforms` 가 `WorldTransform` 컴포넌트를 World 에 add 하기 때문.

### 3.4 collect_render_items — survivor_game helper

```rust
pub struct RenderItem {
    pub entity: voltex_ecs::Entity,
    pub model: voltex_math::Mat4,        // = WorldTransform
    pub mesh: voltex_ecs::MeshHandle,    // assets.mesh(item.mesh) 로 조회
    pub material: voltex_renderer::MaterialUniform,  // 없으면 default
}

pub fn collect_render_items(world: &mut World, assets: &RenderAssets) -> Vec<RenderItem>;
```

내부:
1. `propagate_transforms(world)`
2. `world.query2::<WorldTransform, MeshHandle>()` iterate
3. 각 엔티티에 대해 `assets.mesh(h)` 가 None 이면 skip (stale handle).
4. MaterialHandle 컴포넌트가 있고 `assets.material(h)` 가 Some 이면 그걸, 없으면 `MaterialUniform::new()` (white, default PBR params).
5. RenderItem 생성하여 push.

survivor_game 의 매 프레임:
```rust
let items = voltex::renderer_ecs::collect_render_items(&mut state.world, &state.render_assets);

// 기존: for i in 0..num_entities { /* parallel arrays */ }
// 변경: for (i, item) in items.iter().enumerate() {
//   cam_staging[i*aligned..] ← CameraUniform { view_proj, model: item.model.cols, cam_pos }
//   mat_staging[i*aligned..] ← item.material
// }

// draw loop: state.render_assets.mesh(item.mesh).expect("registered") 로 buffer 가져와 draw
```

### 3.5 Facade 통합

```rust
// crates/voltex/src/lib.rs
pub use voltex_renderer_ecs as renderer_ecs;
```

사용자 코드:
```rust
use voltex::renderer_ecs::{RenderAssets, BasicRenderer, RenderItem, collect_render_items};
```

prelude 에는 추가 안 함 (RenderAssets/BasicRenderer 는 도메인 특화, prelude 정책 일치).

## 4. 데이터 흐름

### 4.1 셋업 (한 번)

```
1. RenderAssets::new()
2. let cube_h = assets.add_mesh(make_unit_cube(device))
3. let white_h = assets.add_material(MaterialUniform::new())
4. BasicRenderer::new(device, queue, viewport_format, depth_format, MAX_ENTITIES)
5. spawn 엔티티 + Transform { position } + MeshHandle(cube_h) + MaterialHandle(white_h)
```

### 4.2 매 프레임 (editor_demo)

```
Inspector 슬라이더 → EditorCommand::SetTransform
  ↓
session.dispatch → World::add(entity, Transform { new_position })
  ↓
BasicRenderer::render(world, assets, ...)
  ↓ propagate_transforms — WorldTransform 갱신
  ↓ collect_render_items — RenderItem 리스트
  ↓ staging buffer 작성 (entities × aligned(CameraUniform))
  ↓ queue.write_buffer
  ↓ render pass: 엔티티별 dynamic offset draw
viewport_tex 에 갱신된 큐브 위치 렌더링됨
```

### 4.3 매 프레임 (survivor_game)

```
Game::update(dt) — 게임 로직, player_pos 등 갱신
  ↓
sync_to_ecs() — world.add(player_entity, Transform { new_pos }), 죽은 적 despawn 등
  ↓
let items = collect_render_items(&mut world, &assets)
  ↓
기존 PBR shadow + color pass 가 items iterate (parallel Vec 자리)
  ↓
draw
```

## 5. 모듈 구조 (voltex_renderer_ecs)

```
crates/voltex_renderer_ecs/
├── Cargo.toml
├── CODEMAP.md
└── src/
    ├── lib.rs                # re-export
    ├── error.rs              # RendererEcsError + Result
    ├── assets.rs             # RenderAssets
    ├── items.rs              # RenderItem + collect_render_items
    └── basic_renderer.rs     # BasicRenderer
```

각 파일 300줄 이하 (600줄 한도). lib.rs 는 `pub mod`/`pub use` 만.

### 5.1 에러 타입

`voltex_renderer_ecs::error::RendererEcsError` (`thiserror` 기반, M4 패턴 준수):
- `Generic(String)` (+ `From<String>`, `From<&str>`)
- `Io(#[from] std::io::Error)`

현재 시점에 강제로 발생하는 에러는 거의 없지만 (대부분 Option<...> 반환), M4 표준화 일관성을 위해 정의. 외부에 노출하는 fallible API 는 v1 에서 없음.

## 6. 테스트 (≥4 — 6개 작성)

| # | 테스트 | 위치 |
|---|---|---|
| 1 | `RenderAssets::add_mesh` → `mesh()` 라운드트립 (한 메시 등록 후 핸들로 조회) | `assets.rs` |
| 2 | `RenderAssets::add_material` → `material()` 라운드트립 | `assets.rs` |
| 3 | 빈 World → `collect_render_items()` == `[]` | `items.rs` |
| 4 | Transform+MeshHandle 있는 엔티티 1개 + Transform 만 있는 엔티티 1개 → 1개만 반환, model = Transform.matrix() | `items.rs` |
| 5 | MaterialHandle 누락 시 default MaterialUniform 반환 | `items.rs` |
| 6 | 3-level hierarchy (root → mid → leaf, 각각 translation) → leaf 의 RenderItem.model 이 누적 행렬 | `items.rs` |

`BasicRenderer::render` 는 GPU 호출이 필요해서 unit test 어려움 (workspace 기준). 대신 collect_render_items 와 RenderAssets 의 invariant 만 검증. BasicRenderer 동작 검증은 editor_demo 의 시각 검증으로 대체.

전체 workspace 테스트 카운트: 803 + 6 = **809+**.

## 7. 시각 검증 (수동)

editor_demo 의 done 기준:
- Editor 띄움 → cube 3개 + ground 보임
- Inspector 패널에서 Cube1 선택 → position.x slider 드래그 → 뷰포트 큐브가 실시간 따라 움직임
- Save → Load → 동일 상태 복원

스크린샷 (3장 권장):
1. 초기 상태 (3 cube + ground)
2. Cube1 을 (5, 0.5, 0) 으로 이동 후
3. Save → Load round-trip 후 동일 화면

## 8. 마이그레이션 디테일

### 8.1 editor_demo 변경 사항

| 제거 | 추가 |
|---|---|
| `scene_meshes: Vec<Mesh>` 필드 | `render_assets: RenderAssets`, `basic_renderer: BasicRenderer` |
| `make_cube(cx, cy, cz, size)` (위치 baked-in) | `make_unit_cube()` (원점 단위 큐브, 1개만 생성) |
| `scene_pipeline: wgpu::RenderPipeline` + 관련 buffer/bind_group | (BasicRenderer 가 owning) |
| `camera_buffer`, `light_buffer`, `camera_light_layout`, `texture_layout`, `dummy_texture_bg` | (BasicRenderer 가 owning) |
| `for mesh in &state.scene_meshes { rpass.draw_indexed(...) }` | `basic_renderer.render(queue, encoder, viewport_color, viewport_depth, &mut session.world, &render_assets, vp, cam_pos, light)` |

시드 엔티티는 그대로 4개 (ground + 3 cube), 각자 spawn + Transform + MeshHandle + MaterialHandle + Tag. ground 는 별도 ground mesh (`make_unit_ground`), cube 3개는 공유 `cube_h` 핸들.

### 8.2 survivor_game 변경 사항

| 제거 | 추가 |
|---|---|
| `entities: Vec<RenderEntity>` (arena) | `arena_entities: Vec<Entity>` (ECS) + RenderAssets 에 등록된 5개 메시 핸들 |
| `models: Vec<Mat4>` 필드 | (각 엔티티의 Transform → WorldTransform 으로 propagate) |
| `materials: Vec<([f32;4], f32, f32)>` 필드 | (RenderAssets 에 5개 MaterialUniform 등록, 각 엔티티에 MaterialHandle) |
| 플레이어/투사체/적의 ad-hoc model/material 계산 | World 에 entity 등록, Game 이 entity ID 보관, 매 프레임 Transform 갱신 + 죽으면 despawn |
| shadow/color pass 의 `for i in 0..num_entities` 루프 | `for item in &items { /* dynamic offset draw */ }` |

`Game` struct 에 World + RenderAssets 핸들 보관 추가:
```rust
struct Game {
    // ... 기존 필드
    pub world: World,
    pub player_entity: Entity,
    pub projectile_entities: Vec<Entity>,
    pub enemy_entities: Vec<Entity>,
    pub player_mesh_h: MeshHandle,
    pub projectile_mesh_h: MeshHandle,
    pub enemy_mesh_h: MeshHandle,
    pub player_mat_h: MaterialHandle,
    // ...
}
```

매 프레임 update 끝에 `sync_to_ecs()` 가 entity Transform 을 player_pos 등으로 갱신.

## 9. 세션 분할

### 세션 1 (이번 conversation 또는 다음)
- `voltex_renderer_ecs` crate 골격 (Cargo.toml, lib.rs, error.rs)
- `RenderAssets` 구현 + 테스트 1, 2
- `RenderItem` + `collect_render_items` + 테스트 3~6
- `BasicRenderer` 구현 (테스트는 GPU 필요해서 생략, editor_demo 시각 검증으로 대체)
- `voltex` facade 에 `pub use voltex_renderer_ecs as renderer_ecs;`
- `CODEMAP.md` 신규, `CLAUDE.md` 의존 그래프 갱신
- `cargo build --workspace` + `cargo test -p voltex_renderer_ecs --lib` 검증

### 세션 2
- editor_demo 리팩토링 (Section 8.1) + Editor 실행 + Inspector 슬라이더 검증 + 스크린샷
- survivor_game 리팩토링 (Section 8.2) + 실제 플레이 검증
- `cargo build --workspace` + `cargo test --workspace --lib` 검증 (809+ 통과)
- `STATUS.md` Phase 8-8 (또는 Cycle 2) 섹션 추가, 로드맵 cycle 2 마커 `[~ 2/2]` → `[✅]`

## 10. 위험 / 미지수

### 10.1 alignment + max_bind_groups

- `min_uniform_buffer_offset_alignment` 는 디바이스마다 다름 (보통 256). many_cubes 의 `align_up` 로직 그대로 차용.
- editor_demo 의 mesh_shader path 는 bind group 2개 (cam+light, texture) 만 → 4개 제약 여유.
- survivor_game 의 PBR path 는 bind group 4개 사용 중 (cam_dynamic, pbr_textures, mat_dynamic, shadow). `collect_render_items` 는 staging 만 만들고 bind group 추가하지 않음 → 충돌 없음.

### 10.2 propagate_transforms 의 비용

매 프레임 모든 root 부터 재귀. v1 엔티티 수 (수십~수백) 에서는 무시 가능. 성능 우려 시 dirty flag 도입은 future work.

### 10.3 RenderAssets 의 한 가지 invariant

`add_mesh` / `add_material` 외 경로로 `Handle<MeshAsset>` 을 만들면 id 불일치 가능. v1 에서는 사용자가 from_raw_parts 를 직접 호출하지 말도록 문서로 안내 + scene 디시리얼라이즈는 raw 그대로 보존하므로 round-trip 일관성 유지됨 (handle id/generation 이 voltex_renderer::Mesh 측의 그것과 같다는 약속).

### 10.4 survivor_game 의 dynamic entity 갱신 비용

매 프레임 적/투사체 추가/제거 → World::spawn / despawn 호출. SparseSet 이므로 amortized O(1). 100 entities 한도 내에서는 무시 가능.

### 10.5 editor_demo 의 Toolbar "Spawn" 버튼

현재 Toolbar 의 Spawn 은 `EditorCommand::Spawn` 만 → 새 엔티티에 MeshHandle 이 없어 BasicRenderer 가 render 안 함. 시각적으로는 "Spawn 눌렀는데 화면에 안 보임" 으로 보일 수 있음.
**처리**: 세션 2 의 editor_demo 변경에서 Spawn 후 자동으로 cube mesh + 흰색 material + Transform(0,0.5,0) 부착하는 helper 를 dispatch (예: `EditorCommand::Spawn` 직후 `SetTransform` 처럼 추가 dispatch). 이건 cycle 12 (E3) 의 AddComponent command 본격 도입 전 임시 처치.

## 11. 변경 영향도

- voltex_ecs: 변경 없음 (C0a 만으로 충분)
- voltex_renderer: 변경 없음 (이미 노출된 Mesh, MaterialUniform, CameraUniform, LightUniform, pipeline::create_mesh_pipeline 사용)
- voltex_asset: 변경 없음
- voltex: facade re-export 1줄 추가
- examples/editor_demo: 상당 변경 (Section 8.1)
- examples/survivor_game: 상당 변경 (Section 8.2)
- examples/many_cubes: 변경 없음 (자체 dynamic UBO 코드 유지). 다음 cycle 에서 BasicRenderer 로 마이그레이션 가능하지만 cycle 2 scope 밖.
- examples/triangle, model_viewer, asset_demo, pbr_demo, hierarchy_demo, ibl_demo, shadow_demo, multi_light_demo, audio_demo, deferred_demo: 변경 없음.

## 12. Done 기준 (로드맵 § Cycle 2 그대로 옮김)

- [ ] 두 example 의 평행 Vec 구조 완전 제거
- [ ] `voltex_renderer_ecs` 단위 테스트 +6 (요구 +4 이상)
- [ ] editor_demo Inspector position slider → 뷰포트 큐브 따라 움직임 (수동, 스크린샷)
- [ ] `cargo build --workspace` 깨끗 (warning 0 새로 추가 안 함)
- [ ] `cargo test --workspace --lib` 깨끗 (809+ 통과)
- [ ] 로드맵 cycle 2 마커 `[✅]` + done commit hash 기록
- [ ] STATUS.md 에 cycle 2 결과 섹션 추가
