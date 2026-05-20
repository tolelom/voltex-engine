# Cycle 3 (E1) — Gizmo + AABB Picking 설계

**작성일**: 2026-05-20
**상위 로드맵**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 3
**Sub-project**: E (Editor v1)
**세션 수**: 2
**의존**: Cycle 2 (C0b) ✓ — Bounds 컴포넌트 + RenderAssets + 시각 검증 가능 상태

---

## 1. 목적

선택된 엔티티를 시각적으로 조작 (translate/rotate/scale 3축 gizmo). picking 정확도를 sphere 근사에서 mesh AABB로 업그레이드. 모든 mutation은 `EditorCommand::SetTransform` dispatch로 흘러 undo/redo + scene 영속화의 혜택을 그대로 받는다.

## 2. Scope / Out-of-scope

### 2.1 In-scope

- `voltex_editor::picking::pick_entity` 업그레이드: Bounds 컴포넌트가 있으면 mesh-local AABB 의 8 corner 를 WorldTransform 으로 transform → world-space AABB 재구축 → ray_vs_aabb. Bounds 가 없으면 기존 sphere 경로 fallback (호환성).
- `voltex_editor::picking::ray_vs_aabb` 신규 헬퍼 (slab method, ~25줄). voltex_physics 에 동등 구현이 있지만 L2 cross-crate 의존성 회피를 위해 복제.
- 신규 모듈 `voltex_editor::gizmo` (5 파일):
  - `mod.rs` — `GizmoMode` enum (Translate/Rotate/Scale), `GizmoState` (drag state 머신)
  - `translate.rs` — translate hit detection + drag plane projection
  - `rotate.rs` — rotate hit detection + ring 평면 angle delta
  - `scale.rs` — scale hit detection + drag plane projection
  - `renderer.rs` — `GizmoRenderer` (전용 wgpu pipeline + procedural 메시 + depth-off forward pass)
- editor_demo 통합:
  - Toolbar 에 Translate/Rotate/Scale 모드 토글 버튼
  - 좌클릭 down → gizmo hit check (현재 모드의 hit detector) → hit 이면 drag state 활성화, 선택 picking 은 보류
  - 드래그 중 World::Transform 직접 mutate (실시간 시각 반영)
  - 드래그 release 시 final Transform 으로 `EditorCommand::SetTransform` 1회 dispatch (undo 1 entry)
  - 시드 엔티티 (ground + 3 cube) 에 Bounds 컴포넌트 부착
- voltex_editor 테스트 +15 이상

### 2.2 Out-of-scope (spec 따라)

- Snap-to-grid
- Multi-select
- OBB (회전된 박스) picking — world-space loose AABB 로 대체
- Constant screen-space gizmo size — world-space 고정 크기 (1.5 단위 길이 등)
- 키보드 W/E/R 모드 단축키 — 버튼만으로
- 화면 안 보이는 면 (back-face) gizmo 부분 dimming
- gizmo 자체의 hover highlight (드래그 중 active axis 만 강조 정도는 가능, hover 는 미구현)
- 회전 시 SetTransform 의 rotation 표현: 현재 `Transform.rotation: Vec3` 는 Euler. quaternion 으로 가지 않고 Euler 누적

## 3. 아키텍처

### 3.1 의존성 변경

```
voltex_editor (L3) — 기존 의존성 그대로 유지: voltex_math, voltex_ecs, voltex_renderer
```

voltex_renderer_ecs 와 voltex_physics 에는 의존 안 함. ray_vs_aabb 는 picking.rs 에 inline.

### 3.2 picking.rs 업그레이드

```rust
// 신규 헬퍼
pub fn ray_vs_aabb(ray: &Ray, aabb: &AABB) -> Option<f32> { /* slab method */ }

// 신규 헬퍼
fn world_aabb_from_local(local: &AABB, world: &Mat4) -> AABB {
    // 8 corner transform, rebuild axis-aligned
}

// pick_entity 변경: Bounds 우선, 없으면 sphere fallback
pub fn pick_entity(world, view_proj, cam_pos, rect, mouse, default_radius) -> Option<Entity> {
    // 1. propagate_transforms 필요 없음 (caller 가 BasicRenderer 호출 전후로 처리 가정)
    // 2. iterate entities with MeshHandle:
    //      if Bounds present + WorldTransform present:
    //          world_aabb = world_aabb_from_local(bounds, &WT)
    //          ray_vs_aabb(&ray, &world_aabb)
    //      else:
    //          기존 sphere 경로 (Transform + default_radius)
    // 3. nearest hit 반환
}
```

**WorldTransform 의존 주의**: pick_entity 는 caller 가 직전에 BasicRenderer::render 를 호출해 WorldTransform 이 갱신되었다는 가정. editor_demo 의 순서는 (mouse event → propagate via BasicRenderer.render is in the SAME frame, but picking happens BEFORE render in current loop). 그래서 pick_entity 내부에서 propagate_transforms 한 번 호출 (cost 무시 가능 — entity 수 적음).

### 3.3 gizmo 모듈 구조

```rust
// gizmo/mod.rs
pub enum GizmoMode { Translate, Rotate, Scale }
pub enum GizmoAxis { X, Y, Z }
pub struct GizmoState {
    pub mode: GizmoMode,
    drag: Option<DragState>,
}
struct DragState {
    axis: GizmoAxis,
    gizmo_origin: Vec3,            // entity world position at drag start
    initial_transform: Transform,  // entity transform at drag start (for delta computation)
    plane_origin: Vec3,            // drag plane origin = gizmo_origin
    plane_normal: Vec3,            // best-facing axis plane normal
    // for translate: anchor = ray-plane hit at drag start, axis projection used to compute delta
    initial_axis_projection: f32,
    // for rotate: anchor angle on the ring plane
    initial_angle: f32,
    // for scale: same as translate, but delta becomes scale factor
}

impl GizmoState {
    pub fn new() -> Self;
    pub fn set_mode(&mut self, mode: GizmoMode);
    pub fn is_dragging(&self) -> bool;

    /// 좌클릭 down 시 호출. hit 한 axis 가 있으면 drag state 활성화 후 true 반환.
    pub fn try_begin_drag(
        &mut self, world: &World, entity: Entity,
        view_proj: &Mat4, cam_pos: Vec3,
        rect: ViewportRect, mouse: (f32, f32),
    ) -> bool;

    /// 드래그 중 매 프레임 호출. World 의 Transform 을 mutate 한다.
    pub fn update_drag(
        &mut self, world: &mut World, entity: Entity,
        view_proj: &Mat4, cam_pos: Vec3,
        rect: ViewportRect, mouse: (f32, f32),
    );

    /// 좌클릭 release 시 호출. drag 중이었으면 final Transform 을 반환 (caller 가
    /// SetTransform dispatch 로 1회 undoable 변경 기록). 반환 후 drag state 해제.
    pub fn end_drag(&mut self, world: &World, entity: Entity) -> Option<Transform>;
}
```

### 3.4 hit detection 세부

#### translate.rs
- 각 axis 의 "thick line" 을 ray vs cylinder (또는 ray vs world-space AABB of thin box) 로 검사
- v1 단순화: axis 별로 thin world-space AABB 정의 (예: X axis = AABB { min: gizmo_origin + (0.2, -thickness, -thickness), max: gizmo_origin + (axis_length, +thickness, +thickness) })
- 3 axis 중 nearest hit 반환

#### rotate.rs
- 각 ring 의 평면 (X ring → YZ 평면, normal = X axis 등)
- ray vs plane → world-space hit point
- |hit - gizmo_origin| 가 [ring_radius - tolerance, ring_radius + tolerance] 범위면 hit
- 3 ring 중 nearest (가장 작은 ray t) hit 반환

#### scale.rs
- translate 와 동일 hit detection (axis-aligned thin box). 차이는 drag 시 의미 (position 가 아니라 scale).

### 3.5 drag math

#### Translate
- drag plane: axis A 에 수직이지 않은 면 중 카메라를 가장 잘 향하는 면 선택. 후보: (A, B) 평면 normal = C, (A, C) 평면 normal = B. dot(camera_forward, candidate_normal) 절댓값이 큰 쪽 선택.
- 매 프레임 ray vs plane → world hit point
- axis projection: t = dot(hit - gizmo_origin, A)
- delta = t_current - t_at_drag_start
- new_position = initial_transform.position + A * delta

#### Rotate
- ring plane normal = ring axis
- ray vs plane → world hit point
- angle = atan2(dot(hit - gizmo_origin, U), dot(hit - gizmo_origin, V)) where U, V are tangent basis in the plane
- delta_angle = angle_current - angle_at_drag_start
- new_rotation_euler[axis_idx] = initial_transform.rotation[axis_idx] + delta_angle

#### Scale
- translate 와 동일 hit + projection
- delta = t_current - t_at_drag_start
- scale_factor = 1.0 + delta / axis_length (drag 거리 비율)
- new_scale[axis_idx] = initial_transform.scale[axis_idx] * scale_factor.max(0.01)

### 3.6 GizmoRenderer

새로운 wgpu pipeline:
- LINE list topology (간단함; translate arrows 도 줄로, 짧고 굵게 보이지는 않지만 v1 OK)
- 또는 별도 LINE list (rings) + triangle list (arrows + boxes) — 2 pipelines
- depth_compare = Always (depth-off)
- 셰이더: per-vertex color (R/G/B 축별)

v1 단순화: **LINE list 단일 pipeline + 3축 모두 line segment 로 표현**:
- Translate: 3축 line segments. 끝부분에 작은 화살촉 (line으로 cross)
- Rotate: 32-segment line strip rings × 3
- Scale: 3축 line segments + 끝에 작은 box outline (12 edge × 3 = 36 segments)

vertex buffer 는 frame 마다 entity 위치 + 현재 모드에 따라 재생성 (CPU 측, 매우 작은 데이터). per-frame uniform 에 view_proj.

`GizmoRenderer::render(queue, encoder, target, depth, view_proj, gizmo_origin, mode, active_axis)` — active_axis 가 있으면 해당 축 색을 노란색으로 강조.

### 3.7 editor_demo 통합

- AppState 추가:
  - `gizmo_state: GizmoState`
  - `gizmo_renderer: GizmoRenderer`
- Toolbar 에 모드 버튼 3개:
  ```rust
  if state.ui.button("Move") { state.gizmo_state.set_mode(GizmoMode::Translate); }
  if state.ui.button("Rotate") { state.gizmo_state.set_mode(GizmoMode::Rotate); }
  if state.ui.button("Scale") { state.gizmo_state.set_mode(GizmoMode::Scale); }
  ```
- 마우스 이벤트 처리 흐름:
  ```
  좌클릭 down 안 (selection 있고 mouse over viewport):
      gizmo_state.try_begin_drag → 성공시 picking 보류, dragging
      실패시 기존 selection picking 경로 (release 시)
  드래그 중 + gizmo 드래깅:
      gizmo_state.update_drag (World 의 Transform 직접 mutate)
  좌클릭 release 안:
      gizmo 드래깅 중이면 end_drag → Some(final) → SetTransform dispatch (1 undo entry)
      gizmo 드래깅 아니었으면 기존 짧은-드래그 = pick 경로
  ```
- 시드 엔티티에 Bounds 부착 (cube → AABB(-0.5, -0.5, -0.5, 0.5, 0.5, 0.5), ground → AABB(-5, -0.01, -5, 5, 0.01, 5))
- 매 프레임 BasicRenderer 렌더 후 selection 있으면 GizmoRenderer 도 호출

## 4. 데이터 흐름

### 4.1 Selection picking (변경)
```
좌클릭 release + 짧은 드래그
  ↓
pick_entity(world, vp, ..., default_radius=0.7)
  ↓ Bounds 있으면 ray_vs_aabb(world AABB), 없으면 ray_vs_sphere
  ↓
session.selected = hit
```

### 4.2 Gizmo drag (신규)
```
좌클릭 down + selection 있음
  ↓
gizmo_state.try_begin_drag(world, selected, ...)
  ↓ 현재 모드의 hit detector
  ↓ hit 있으면 drag state 초기화 (axis, plane, anchor 기록)
드래그 중 매 프레임:
  ↓
gizmo_state.update_drag(world, selected, ...)
  ↓ axis projection / angle delta 계산
  ↓ world.get_mut::<Transform>(selected) 직접 mutate (실시간 시각 반영)
좌클릭 release:
  ↓
gizmo_state.end_drag(world, selected) → Some(final_transform)
  ↓
session.dispatch(EditorCommand::SetTransform { entity: selected, ...final_transform })
  ↓ inverse 캡처 → undo 스택 push (드래그 시작 시점의 initial_transform 이 inverse)
```

**중요**: 드래그 중 직접 mutate 와 release 시 SetTransform dispatch 사이에 차이가 있는데, SetTransform 의 inverse 는 dispatch 시점의 pre-state 를 capture (즉 final value 직전 = final value). 이게 잘못된다 → drag 시작 전 initial value 가 inverse 여야 함.

**해결**: `end_drag` 가 final transform 반환 전에 World 의 Transform 을 **drag 시작 시점 값으로 되돌린 후** SetTransform dispatch. dispatch 가 inverse 캡처 (drag 시작 값) 한 뒤 forward 적용. 결과: World 는 final, undo 스택은 initial 의 SetTransform inverse.

구현:
```rust
fn handle_drag_release(...) {
    if let Some(final_t) = state.gizmo_state.end_drag(&state.session.world, selected) {
        // Roll back to initial for proper inverse capture
        if let Some(t) = state.session.world.get_mut::<Transform>(selected) {
            *t = initial_transform; // stored in gizmo's DragState before end_drag clears it
        }
        // Dispatch final transform — captures initial as inverse, applies final
        let _ = state.session.dispatch(EditorCommand::SetTransform {
            entity: selected,
            position: final_t.position,
            rotation: final_t.rotation,
            scale: final_t.scale,
        });
    }
}
```

→ `end_drag` API 가 final + initial 둘 다 반환하는게 깔끔:
```rust
pub fn end_drag(...) -> Option<DragResult>;
pub struct DragResult { pub initial: Transform, pub final_transform: Transform }
```

## 5. 모듈 구조

```
crates/voltex_editor/src/
├── picking.rs        # 기존 + ray_vs_aabb + world_aabb_from_local + Bounds-aware pick_entity
├── gizmo/
│   ├── mod.rs        # GizmoMode, GizmoAxis, GizmoState, DragState, DragResult
│   ├── translate.rs  # hit + drag math (axis projection)
│   ├── rotate.rs     # hit + drag math (angle delta)
│   ├── scale.rs      # hit + drag math (scale factor)
│   └── renderer.rs   # GizmoRenderer (pipeline + procedural lines)
└── lib.rs            # pub mod gizmo; pub use gizmo::{GizmoMode, GizmoState, GizmoRenderer};
```

각 파일 ≤ 300줄 (gizmo/renderer.rs 가 가장 큼 — 메시 + pipeline + render 합쳐 ~350줄 예상; 초과시 mesh.rs 로 추가 분리).

## 6. 에러 처리

- `pick_entity` 는 기존대로 `Option<Entity>` 반환 (None = no hit)
- `GizmoState::try_begin_drag` 는 bool (true = drag 시작)
- `GizmoState::update_drag` 는 단순 mutate, 실패 silent (예: matrix 비가역 시 no-op)
- `GizmoState::end_drag` 는 `Option<DragResult>` (None = drag 없었음)
- 새 에러 variant 없음. EditorError 그대로.

## 7. 테스트 (≥15 — 17개 작성)

| # | 파일 | 케이스 |
|---|---|---|
| 1 | picking.rs | `ray_vs_aabb` slab hit (front face) |
| 2 | picking.rs | `ray_vs_aabb` miss (parallel) |
| 3 | picking.rs | `ray_vs_aabb` inside (t=0) |
| 4 | picking.rs | `world_aabb_from_local` translation only |
| 5 | picking.rs | `world_aabb_from_local` rotation + scale (loose AABB enlarges) |
| 6 | picking.rs | `pick_entity` Bounds 경로 → AABB hit |
| 7 | picking.rs | `pick_entity` Bounds 없음 → sphere fallback hit |
| 8 | picking.rs | `pick_entity` 두 AABB 겹침 → 가까운 것 반환 |
| 9 | gizmo/translate.rs | axis hit detection (X axis thin AABB) |
| 10 | gizmo/translate.rs | drag plane 선택 (camera 향한 면) |
| 11 | gizmo/translate.rs | drag delta along axis |
| 12 | gizmo/rotate.rs | ring plane hit (X ring = YZ 평면) |
| 13 | gizmo/rotate.rs | angle delta computation |
| 14 | gizmo/scale.rs | handle hit + scale factor |
| 15 | gizmo/mod.rs | mode transition (Translate → Rotate) |
| 16 | gizmo/mod.rs | begin_drag → update_drag → end_drag 흐름 |
| 17 | gizmo/mod.rs | end_drag returns DragResult with initial != final |

GizmoRenderer 는 GPU 필요로 unit test 제외. editor_demo 시각 검증.

## 8. 시각 검증 (수동 — 자동 환경 부재 시 빌드만 확인)

editor_demo 의 done 기준:
- Toolbar 의 Move/Rotate/Scale 버튼 → 선택된 큐브 위에 각각 다른 gizmo 표시
- Translate gizmo X 축 드래그 → 큐브가 X 방향으로 이동, 다른 축은 잠금
- Rotate gizmo Y 링 드래그 → 큐브가 Y 축으로 회전
- Scale gizmo Z 핸들 드래그 → 큐브가 Z 축으로만 커지/작아짐
- 드래그 종료 후 Undo → 드래그 시작 위치로 복원 (1 step)
- Save → Load → 위치/회전/스케일 유지

## 9. 세션 분할

### Session 1 (2-3 cycles 분량)
- `ray_vs_aabb` + `world_aabb_from_local` 헬퍼 + 단위 테스트 5개
- `pick_entity` Bounds-aware 업그레이드 + 단위 테스트 3개
- `voltex_editor::gizmo` 모듈 골격 (mod.rs)
- `gizmo/translate.rs` + 단위 테스트 3개
- `gizmo/renderer.rs` 골격 (LINE pipeline + render 메서드, translate 메시만 우선)
- editor_demo 통합: Toolbar 버튼 + Translate gizmo 드래그 → SetTransform dispatch
- 시드 엔티티에 Bounds 추가
- 빌드 깨끗 + 단위 테스트 11개 통과

### Session 2 (1-2 cycles 분량)
- `gizmo/rotate.rs` + 단위 테스트 2개
- `gizmo/scale.rs` + 단위 테스트 2개
- `gizmo/mod.rs` state machine 테스트 2개
- `GizmoRenderer` 확장: ring 메시 + scale 메시
- editor_demo: Rotate/Scale 버튼 활성화
- STATUS.md + 로드맵 cycle 3 [✅] 마커 + commit hash

## 10. 위험 / 미지수

### 10.1 drag math 의 가장자리 케이스

- 카메라가 axis 정면을 바라보는 경우 (axis 가 화면에 직선으로 점 하나): drag plane 선택이 양쪽 후보 모두 face-on → 안정성 위해 epsilon 비교 + 둘 다 비슷하면 default to (A, B) 평면
- gizmo origin 이 카메라 뒤 (ray plane intersection t < 0): drag 무시 (no movement)
- 회전 각도 wraparound (atan2 가 -π/π 경계 넘을 때): atan2 결과를 unwrap 하는 로직 — 매 프레임 prev 와 delta 계산 시 abs(delta) > π 면 ±2π 보정

### 10.2 매 프레임 dispatch vs 단일 dispatch

위 § 4.2 의 "release 시 1회 dispatch" 정책으로 해결. 매 프레임 dispatch 했으면 undo 스택이 수십 개 SetTransform 으로 채워져 polluted.

### 10.3 SetTransform 의 inverse capture timing

위 § 4.2 의 "roll back to initial → dispatch" 우회로 해결. `end_drag` 가 initial+final 모두 반환하도록 API 설계.

### 10.4 LINE topology 의 선 두께

wgpu 의 LINE 은 1px 고정. 시각적으로 얇음. v1 수용. 더 굵게 보이려면 폴리곤 (긴 quad) 으로 그려야 하지만 scope 외.

### 10.5 gizmo 가 화면에 너무 작을 / 클 수 있음

world-space 고정 크기 (1.5 단위) 이므로 카메라 거리에 따라 화면 크기가 변함. 카메라 zoom 에 따라 scale 조정은 v1 외 (constant screen-space size 는 spec out-of-scope).

### 10.6 Bounds 없는 엔티티의 gizmo 표시

GizmoRenderer 는 selected.world_position 만 필요 — Bounds 무관. WorldTransform 만 있으면 OK.

## 11. 변경 영향도

| Crate / Module | 변경 |
|---|---|
| voltex_editor | picking.rs 업그레이드, 신규 `gizmo/` 모듈 (5 파일), lib.rs re-export |
| voltex_ecs | 변경 없음 (Bounds 는 C0a 에서 이미 존재) |
| voltex_renderer | 변경 없음 |
| voltex_renderer_ecs | 변경 없음 |
| voltex_math, voltex_asset, voltex_physics | 변경 없음 |
| examples/editor_demo | AppState 확장, Toolbar 버튼 3개, 시드 엔티티 Bounds, gizmo render call, 마우스 이벤트 분기 |
| 다른 examples | 변경 없음 |

## 12. Done 기준 (로드맵 § Cycle 3 그대로)

- [ ] 3종 gizmo 모두 정상 드래그 + `EditorCommand::SetTransform` dispatch → undo/redo 동작
- [ ] AABB picking 정확도 테스트 (회전된 박스 외곽, 겹친 박스 등) — 8 케이스
- [ ] voltex_editor 테스트 +15 이상 (17 작성)
- [ ] editor_demo: 큐브 선택 → 드래그 → 위치/회전/크기 변경 → Save → Load → 일관성 (수동 검증)
- [ ] `cargo build --workspace` 깨끗
- [ ] `cargo test --workspace --lib` 깨끗 (836+ 통과)
- [ ] STATUS.md, 로드맵 cycle 3 `[✅]` + done commit hash
