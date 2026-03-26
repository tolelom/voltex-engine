# Entity Inspector Design

## Overview

에디터 도킹 패널에 엔티티 계층 트리(Hierarchy)와 선택된 엔티티의 컴포넌트 편집(Inspector)을 추가한다. Transform, Tag, Parent 컴포넌트를 하드코딩으로 지원.

## Scope

- Hierarchy 패널: Transform 보유 엔티티를 계층 트리로 표시, 클릭 선택
- Inspector 패널: 선택된 엔티티의 Transform(슬라이더), Tag(text_input), Parent(읽기 전용) 편집
- 고정 컴포넌트만 (리플렉션/등록 시스템 없음)

## Dependencies

`voltex_editor/Cargo.toml`에 추가:
```toml
voltex_ecs.workspace = true
```

## API

함수 기반, 구조체 불필요:

```rust
/// Hierarchy 패널: 엔티티 트리 표시, 선택 처리
pub fn hierarchy_panel(
    ui: &mut UiContext,
    world: &World,
    selected: &mut Option<Entity>,
    rect: &Rect,
);

/// Inspector 패널: 선택된 엔티티의 컴포넌트 편집
pub fn inspector_panel(
    ui: &mut UiContext,
    world: &mut World,
    selected: Option<Entity>,
    rect: &Rect,
);
```

## Hierarchy 패널

### 엔티티 수집
- `roots(world)` → 루트 엔티티 (Transform 있고 Parent 없는) 목록
- 각 루트에서 `world.get::<Children>(entity)` 로 자식 재귀 탐색

### 렌더링
- 재귀 함수: `draw_entity_node(ui, world, entity, depth, selected)`
- 들여쓰기: `rect.x + 4.0 + depth * 16.0`
- 표시 텍스트: Tag가 있으면 `tag.0`, 없으면 `"Entity(id)"`
- 자식이 있으면 접두사 `"> "`, 없으면 `"  "`
- 선택된 엔티티: 배경 하이라이트 (밝은 색)
- 클릭 감지: `ui.mouse_clicked && ui.mouse_in_rect(x, y, w, line_height)`

### 스크롤
- `ui.begin_scroll_panel` / `end_scroll_panel` 사용
- content_height = 전체 노드 수 * line_height (재귀 `count_nodes` 헬퍼로 사전 계산)
- 선택 하이라이트 색상: `[0x44, 0x66, 0x88, 0xFF]`

## Inspector 패널

선택된 엔티티가 None이면 "No entity selected" 표시.

### Transform 섹션
- `world.has_component::<Transform>(entity)` 체크
- 헤더: "-- Transform --"
- Position X/Y/Z: 슬라이더 3개 (range -50.0 ~ 50.0)
- Rotation X/Y/Z: 슬라이더 3개 (range -3.15 ~ 3.15, 약 ±π)
- Scale X/Y/Z: 슬라이더 3개 (range 0.01 ~ 10.0)

**Borrow 패턴** (Transform은 Copy이므로 cheap):
```rust
// 1. 값 복사 (immutable borrow 해제)
let (px, py, pz, rx, ry, rz, sx, sy, sz) = {
    let t = world.get::<Transform>(entity).unwrap();
    (t.position.x, t.position.y, t.position.z,
     t.rotation.x, t.rotation.y, t.rotation.z,
     t.scale.x, t.scale.y, t.scale.z)
};
// 2. 슬라이더 (world 미사용)
let new_px = ui.slider("Pos X", px, -50.0, 50.0);
// ... 9개 슬라이더
// 3. 변경된 값 쓰기 (mutable borrow)
if let Some(t) = world.get_mut::<Transform>(entity) {
    t.position.x = new_px; // ...
}
```

### Tag 섹션
- `world.has_component::<Tag>(entity)` 체크
- 헤더: "-- Tag --"
- **호출자가 `tag_buffer: String`을 별도로 유지** (선택 변경 시 동기화)
- `ui.text_input(id, &mut tag_buffer, x, y, width)` 로 편집
- 변경 시 `world.get_mut::<Tag>(entity).unwrap().0 = tag_buffer.clone()`

### Parent 섹션
- `world.has_component::<Parent>(entity)` 체크
- 헤더: "-- Parent --"
- `"Parent: Entity(id)"` 텍스트 (읽기 전용)

## UI 통합

```rust
// editor_demo에서 dock 패널 매핑
// panel 0: Hierarchy → hierarchy_panel(ui, &world, &mut selected, rect)
// panel 1: Viewport → 3D scene
// panel 2: Inspector → inspector_panel(ui, &mut world, selected, rect)
// panel 3: Console → text
```

editor_demo에서 ECS World를 생성하고 데모 엔티티를 배치:
- Ground (Transform + Tag("Ground"))
- Cube1 (Transform + Tag("Cube1"))
- Cube2 (Transform + Tag("Cube2"))
- Cube3 (Transform + Tag("Cube3"))
- Parent-child 관계 1개 (Cube2를 Cube1의 자식으로)

Transform 수정 후 `propagate_transforms(world)` 호출하여 WorldTransform 갱신.

## File Structure

- `crates/voltex_editor/src/inspector.rs` — hierarchy_panel, inspector_panel 함수
- `crates/voltex_editor/src/lib.rs` — 모듈 추가
- `crates/voltex_editor/Cargo.toml` — voltex_ecs 의존성
- `examples/editor_demo/src/main.rs` — World + 엔티티 + 패널 통합

## Testing

### hierarchy_panel (GPU 불필요)
- World에 엔티티 3개 (Transform) 추가 → hierarchy_panel 호출 → draw_list에 커맨드 생성 확인
- 선택 처리: 마우스 클릭 시뮬레이션 → selected 변경 확인
- 빈 World → 커맨드 최소 (헤더만)

### inspector_panel (GPU 불필요)
- Transform가진 엔티티 선택 → inspector_panel 호출 → draw_list에 슬라이더 커맨드 확인
- None 선택 → "No entity selected" 텍스트만
- Transform 슬라이더 조작 → world에서 값 변경 확인 (슬라이더 반환값으로 시뮬레이션)
