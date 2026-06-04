# voltex_editor

## 책임 (한 줄)
Immediate-mode UI 에디터 — 도킹/플로팅/뷰포트/인스펙터/에셋 브라우저 + TTF 렌더링.

## 공개 API 진입점

### 코어 UI
- `UiContext` — IMGUI 컨텍스트 (src/ui_context.rs)
- `UiRenderer` — wgpu UI 렌더러 (src/renderer.rs)
- `DrawList`, `DrawVertex`, `DrawCommand` (src/draw_list.rs)
- `FontAtlas` — 8x12 비트맵 ASCII (src/font.rs)
- `LayoutState` — 레이아웃 (src/layout.rs)
- `TtfFont`, TTF 파서/래스터라이저/캐시 (src/ttf_font.rs, src/ttf_parser.rs, src/rasterizer.rs, src/glyph_cache.rs)

### Widgets
- text, button, slider, checkbox, panel, text_input, scroll_panel, drag/drop (src/widgets/ — controls, containers, text_input, drag)

### Layout
- `DockTree`, `DockNode`, `Axis`, `Rect`, `LeafLayout` — 도킹 (src/dock.rs, 558줄)
- `FloatingPanel`, `FloatingPanelManager` (src/floating_panel.rs)

### 3D 뷰포트
- `OrbitCamera` (src/orbit_camera.rs)
- `ViewportTexture`, `VIEWPORT_COLOR_FORMAT`, `VIEWPORT_DEPTH_FORMAT` (src/viewport_texture.rs)
- `ViewportRenderer` (src/viewport_renderer.rs)
- `picking::pick_entity` + helpers (ndc_from_viewport / ray_from_ndc / ray_vs_sphere_t) — 좌클릭 → ray → sphere intersection → 가장 가까운 entity. 반경은 default × max(scale). 메시 바운드 미구현 (DEFERRED). (src/picking.rs)

### Panels (통합 완료, 단일 경로)
- `panels::hierarchy::hierarchy_panel` — 트리 뷰, click 으로 `session.selected` 설정 (src/panels/hierarchy.rs)
- `panels::inspector::inspector_panel` + `InspectorState` — 슬라이더(transform) + text_input(tag) + 부착 컴포넌트 목록(`- name` 버튼 → RemoveComponent) + Add 버튼(미부착만 표시) → 모든 변경이 EditorCommand 로 dispatch (src/panels/inspector.rs)
- `attached_component_names(session, entity) -> Vec<String>` — registry 기준 부착 컴포넌트 이름 목록 (UI/테스트 공용 순수 헬퍼, re-exported)
- `component_value_summary(session, entity, name) -> Option<String>` — 빌트인(bounds/mesh_handle/material_handle/script) 한 줄 read-only 요약. transform/tag(전용 UI)·미지/raw 이름은 None (re-exported)
- `count_entity_nodes(world, entity)` — 서브트리 노드 수 (re-exported)
- `AssetBrowser`, `asset_browser_panel`, `AssetKind` (src/asset_browser.rs) — 파일 트리 뷰 + 종류 분류(`AssetKind::from_path`: Texture/Scene/Mesh/Audio/Other) + "Open" 활성화. `selected_full_path()` / `selected_kind()` / `take_activated() -> Option<(PathBuf, AssetKind)>` 로 통합 레이어가 로드/적용(GPU 동작은 에디터 밖, editor_demo 가 수행 — 레이어링 유지)

### Editor Command / Error
- `EditorCommand` — Spawn / Despawn / SetTransform / SetTag / **AddChild** / **RemoveChild** / SaveScene / LoadScene (src/command.rs)
- `EditorError`, `Result` — Generic / EntityNotAlive(Entity) / Io(String) / Parse(String) / **ModeMismatch { current: EditorMode }** (src/error.rs, PartialEq 유지)
- `EditorSession` — World + ComponentRegistry + 자동저장 + jsonl 로그 + **Undo/Redo 스택** + **Play/Stop mode + lifecycle 콜백** (src/session.rs)
  - `dispatch(cmd)` — apply + log + auto-save + history push. **Play 모드에서는 ModeMismatch 로 거부**.
  - `undo() / redo() -> bool` — 마지막 액션 복원 / 재적용
  - `undo_depth() / redo_depth() -> usize` — UI/test 용
  - **v1 undoable**: Spawn / SetTransform / SetTag / AddChild / RemoveChild.  **v1 non-undoable**: Despawn / SaveScene / LoadScene (실행 시 history 클리어)
  - **LoadScene semantics**: World::clear() 후 deserialize → "디스크가 source of truth". 시드 엔티티는 Save 안 했으면 Load 후 사라짐.

### Play / Stop modes (E2)
- `EditorMode { Edit, Play }` — 에디터의 user-facing state. (src/session.rs)
- `EditorSession::set_mode(target)` — Edit↔Play 전환.
  - Edit→Play: `serialize_scene_binary` 로 snapshot 저장 → mode 전환 → `LifecycleEvent::PlayStart` 콜백 호출 → undo/redo 클리어.
  - Play→Edit: `LifecycleEvent::PlayStop` 콜백 호출 → `World::clear()` → snapshot 으로 deserialize → mode 전환 → undo/redo 클리어.
  - 같은 모드 호출은 no-op.
- `EditorSession::add_lifecycle(closure)` — `(LifecycleEvent, &mut World)` 받는 콜백 등록. 등록 순서대로 호출.
- `lifecycle::LifecycleEvent { PlayStart, PlayStop }` + `lifecycle::LifecycleCallback` (src/lifecycle.rs)
- **미등록 컴포넌트는 Play→Stop 시 자동 소멸**. snapshot 은 `ComponentRegistry` 에 등록된 타입만 보존 (built-in: Transform, Tag, MeshHandle, MaterialHandle, Bounds). RigidBody / Collider / Velocity 처럼 미등록인 컴포넌트는 Play 중에 추가했어도 Stop 시 사라진다. editor_demo 의 physics 데모가 이 동작을 이용해 RigidBody 를 PlayStart 콜백에서 부착 → Stop 자동 정리. 게임 측 영구 컴포넌트는 `registry.register::<MyComponent>()` 로 등록 필수.

## 모듈 구조
- 코어: ui_context, renderer, draw_list, font, layout, ttf_* (parser, font, rasterizer, glyph_cache)
- 위젯: widgets/ (controls, containers, text_input, drag)
- 레이아웃: dock, floating_panel
- 뷰포트: orbit_camera, viewport_texture, viewport_renderer, picking, gizmo/
- 패널: panels/{hierarchy, inspector}, asset_browser
- 커맨드/세션: command, session, lifecycle
- 셰이더: ui_shader.wgsl, gizmo/shader.wgsl, viewport_shader.wgsl

## 의존성
- voltex crate: voltex_math, voltex_ecs, voltex_renderer
- 외부 crate: wgpu (workspace), bytemuck (workspace)

## 변경 시 주의점
- 모든 패널 mutation은 `EditorCommand` + `EditorSession::dispatch` 경유 (jsonl 로그 + 자동 저장 자동). 직접 `world.get_mut::<T>()` 호출 지양.
- `session.selected: Option<Entity>` 가 선택의 단일 소스. 패널 간 공유 변수 두지 말 것.
- `InspectorState` 는 text_input 버퍼 (Tag 등) 만 보관. Transform 값은 매 프레임 World 에서 다시 읽어 슬라이더에 표시.
- widgets는 src/widgets/ 디렉토리로 분리됨 (M3 완료). dock.rs (558줄), ttf_parser.rs (518줄)는 spec 만족 (600줄 이하), 분리 대상 아님
- 탭 드래그 이동, 커스텀 컴포넌트 인스펙터 리플렉션, 에셋 프리뷰는 DEFERRED
- examples/editor_demo의 시드 엔티티는 8-7에서 `EditorCommand::Spawn/SetTransform/SetTag/AddChild` 경로로 옮김 → 로그/Save/Load와 일치
- 픽킹은 v1에서 sphere 근사. 메시 AABB picking은 ECS에 mesh bounds가 들어온 뒤로 미룸 (DEFERRED.md "렌더링 한계")

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_editor --lib`
- 현재 테스트 개수: 182개 (AssetBrowser AssetKind/활성화 +6 포함)
- `UiContext::progress_bar(x,y,w,h,fraction,fg,bg)` / `draw_rect` — HUD 체력/진행 바 헬퍼 (runtime `set_hud_fn` 에서 사용)
