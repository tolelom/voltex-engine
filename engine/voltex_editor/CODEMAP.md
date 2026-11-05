# voltex_editor

## 책임 (한 줄)
Immediate-mode UI 에디터 — 도킹/플로팅/뷰포트/인스펙터/에셋 브라우저 + TTF 렌더링.

## 공개 API 진입점

### 코어 UI
- `UiContext` — IMGUI 컨텍스트 (src/ui_context.rs)
- `UiRenderer` — wgpu UI 렌더러 (src/ui_renderer.rs; 옛 경로 `voltex_editor::renderer` 는 deprecated 재export)
- `DrawList`, `DrawVertex`, `DrawCommand` (src/draw_list.rs) — 비트맵 글자 그리기는 모두 `DrawList::add_text` 한 곳
- `FontAtlas` + `UvRect` (`glyph_uv_rect`) — 8x12 비트맵 ASCII (src/bitmap_font.rs; 옛 경로 `voltex_editor::font`·`glyph_uv` 는 deprecated)
- `LayoutState` — 레이아웃 (src/layout.rs). `record_last_widget` 가 same_line 용 직전 위젯 위치 기록 (옛 이름 `record` deprecated)
- `TtfFont`, TTF 파서/래스터라이저/캐시 (src/ttf_font.rs, src/ttf_parser.rs, src/rasterizer.rs, src/glyph_cache.rs)

### Widgets
- text, button, slider, checkbox, panel, text_input, scroll_panel, drag/drop (src/widgets/ — controls, containers, text_input, drag)

### Layout
- `DockTree`, `DockNode`, `Axis`, `Rect`, `LeafLayout` — 도킹 (src/dock.rs, 558줄)
- `FloatingPanel`, `FloatingPanelManager` (src/floating_panel.rs)

### 3D 뷰포트
- `OrbitCamera` (src/orbit_camera.rs) — `forward()`는 yaw/pitch만으로 계산해 `distance` 0에서도 유한 (v1.0.126)
- `ViewportTexture`, `VIEWPORT_COLOR_FORMAT`, `VIEWPORT_DEPTH_FORMAT` (src/viewport_texture.rs)
- `ViewportRenderer` (src/viewport_renderer.rs)
- `picking::pick_entity` + helpers (ndc_from_viewport / ray_from_ndc / ray_vs_aabb / ray_vs_sphere_t) — 좌클릭 → ray → 가장 가까운 entity. `Bounds`+`WorldTransform` 이 있으면 world AABB, 없으면 sphere(반경 default × max(scale)). 카메라를 품은 AABB 는 다른 히트가 없을 때만 선택. (src/picking.rs)

### Panels (통합 완료, 단일 경로)
- `panels::hierarchy::hierarchy_panel` — 트리 뷰, click 으로 `session.selected` 설정 (src/panels/hierarchy.rs)
- `panels::inspector::inspector_panel` + `InspectorState` — 슬라이더(transform) + text_input(tag) + 부착 컴포넌트 목록(`- name` 버튼 → RemoveComponent) + Add 버튼(미부착만 표시) → 모든 변경이 EditorCommand 로 dispatch (src/panels/inspector/mod.rs)
- `attached_component_names(session, entity) -> Vec<String>` — registry 기준 부착 컴포넌트 이름 목록 (UI/테스트 공용 순수 헬퍼, re-exported)
- `component_value_summary(session, entity, name) -> Option<String>` — 빌트인(bounds/mesh_handle/material_handle/script) 한 줄 read-only 요약. transform/tag(전용 UI)·미지/raw 이름은 None (re-exported)
- `bounds_bytes(min, max) -> Vec<u8>` (re-exported) + Bounds 편집 폼 — Components 목록의 `bounds` 는 요약 대신 6 슬라이더(±50), 변경 시 `AddComponent` dispatch. **AddComponent inverse 분기 (v1.0.105)**: prior 값이 있으면 undo 가 prior bytes 복원(편집), 없으면 제거(신규 추가 — E3 원래 의미)
- `count_entity_nodes(world, entity)` — 서브트리 노드 수 (re-exported)
- `AssetBrowser`, `asset_browser_panel(ui, browser, rect, preview, thumbs)`, `AssetKind`, `BrowserView`, `grid_columns` (src/asset_browser/mod.rs) — 파일 리스트/그리드 뷰(`view: BrowserView`, 패널 `[Grid]`/`[List]` 토글, 기본 List) + 종류 분류(`AssetKind::from_path`: Texture/Scene/Mesh/Audio/Other) + "Open" 활성화 + 선택 파일 썸네일(`preview: Option<u32>` = register_texture 슬롯) + 그리드 셀 썸네일(`thumbs: Option<&HashMap<String, u32>>` 파일명→슬롯, 미등록은 kind placeholder; 디코드/업로드는 통합 레이어). 그리드 셀 클릭 = 파일 선택/디렉토리 navigate (리스트와 동일 의미). `selected_full_path()` / `selected_kind()` / `take_activated() -> Option<(PathBuf, AssetKind)>` 로 통합 레이어가 로드/적용(GPU 동작은 에디터 밖, editor_demo 가 수행 — 레이어링 유지). `relative_to_root(path) -> String` — root 상대 + forward slash 정규화 (씬 영속 에셋 참조의 canonical 형식, v1.0.99)
- **UI 이미지 위젯 (v1.0.93)**: `DrawCommand.texture: Option<u32>` + `DrawList::add_image` + `UiRenderer::register_texture(device, &TextureView) -> u32`(슬롯 영구, 해제 없음) + `UiContext::image(slot, w, h)`(이미지 높이만큼 레이아웃 전진). 렌더는 슬롯 있는 command 만 `fs_image`(풀 RGBA) 파이프라인으로 전환 — 폰트 경로(`fs_main`, R 채널 알파)는 불변. 미등록 슬롯은 폰트 아틀라스 fallback(no panic)

### Editor Command / Error
- `EditorCommand` — Spawn / Despawn / SetTransform / SetTag / **AddChild** / **RemoveChild** / SaveScene / LoadScene (src/command.rs)
- `EditorError`, `Result` — Generic / EntityNotAlive(Entity) / Io(String) / Parse(String) / **ModeMismatch { current: EditorMode }** (src/error.rs, PartialEq 유지)
- `EditorSession` — World + ComponentRegistry + 자동저장 + jsonl 로그 + **Undo/Redo 스택** + **Play/Stop mode + lifecycle 콜백** (src/session.rs)
  - `dispatch(cmd)` — apply + log + auto-save + history push. **Play 모드에서는 ModeMismatch 로 거부**.
  - `undo() / redo() -> bool` — 마지막 액션 복원 / 재적용
  - `undo_depth() / redo_depth() -> usize` — UI/test 용
  - **undoable**: Spawn / Despawn(E3, v1.0.146부터 슬롯이 재사용돼도 새 핸들로 재생성 + 계층 링크 복원) / SetTransform / SetTag / AddChild(이전 부모 복원) / RemoveChild / AddComponent / RemoveComponent.  **non-undoable**: SaveScene / LoadScene (실행 시 history 클리어). 무작위 속성 테스트 `session/undo_property_tests.rs`가 undo-all·redo-all 일치를 검증
  - **LoadScene semantics**: World::clear() 후 deserialize → "디스크가 source of truth". 시드 엔티티는 Save 안 했으면 Load 후 사라짐.

### Play / Stop modes (E2)
- `EditorMode { Edit, Play }` — 에디터의 user-facing state. (src/session.rs)
- `EditorSession::set_mode(target)` — Edit↔Play 전환.
  - Edit→Play: `serialize_scene_binary` 로 snapshot 저장 → mode 전환 → `LifecycleEvent::PlayStart` 콜백 호출 → undo/redo 클리어.
  - Play→Edit: `LifecycleEvent::PlayStop` 콜백 호출 → `World::clear()` → snapshot 으로 deserialize → 선택(`selected`)을 스냅샷 순서로 복원 엔티티에 재매핑(Play 중 생성 엔티티면 None) → mode 전환 → undo/redo 클리어.
  - 같은 모드 호출은 no-op.
- `EditorSession::add_lifecycle(closure)` — `(LifecycleEvent, &mut World)` 받는 콜백 등록. 등록 순서대로 호출.
- `lifecycle::LifecycleEvent { PlayStart, PlayStop }` + `lifecycle::LifecycleCallback` (src/lifecycle.rs)
- **미등록 컴포넌트는 Play→Stop 시 자동 소멸**. snapshot 은 `ComponentRegistry` 에 등록된 타입만 보존 (built-in: Transform, Tag, MeshHandle, MaterialHandle, Bounds). RigidBody / Collider / Velocity 처럼 미등록인 컴포넌트는 Play 중에 추가했어도 Stop 시 사라진다. editor_demo 의 physics 데모가 이 동작을 이용해 RigidBody 를 PlayStart 콜백에서 부착 → Stop 자동 정리. 게임 측 영구 컴포넌트는 `registry.register::<MyComponent>()` 로 등록 필수.

## 모듈 구조
- 코어: ui_context, ui_renderer, draw_list, bitmap_font, layout, ttf_* (parser, font, rasterizer, glyph_cache)
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
