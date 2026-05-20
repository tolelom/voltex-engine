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

### Panels (통합 완료, 단일 경로)
- `panels::hierarchy::hierarchy_panel` — 트리 뷰, click 으로 `session.selected` 설정 (src/panels/hierarchy.rs)
- `panels::inspector::inspector_panel` + `InspectorState` — 슬라이더(transform) + text_input(tag) → 모든 변경이 EditorCommand 로 dispatch (src/panels/inspector.rs)
- `count_entity_nodes(world, entity)` — 서브트리 노드 수 (re-exported)
- `AssetBrowser`, `asset_browser_panel` (src/asset_browser.rs)

### Editor Command / Error
- `EditorCommand` — Spawn / Despawn / SetTransform / SetTag / **AddChild** / **RemoveChild** / SaveScene / LoadScene (src/command.rs)
- `EditorError`, `Result` — Generic / EntityNotAlive(Entity) / Io(String) / Parse(String) (src/error.rs, PartialEq 유지)
- `EditorSession` — World + ComponentRegistry + 자동저장 + jsonl 로그 + **Undo/Redo 스택** (src/session.rs)
  - `dispatch(cmd)` — apply + log + auto-save + history push
  - `undo() / redo() -> bool` — 마지막 액션 복원 / 재적용
  - `undo_depth() / redo_depth() -> usize` — UI/test 용
  - **v1 undoable**: Spawn / SetTransform / SetTag / AddChild / RemoveChild.  **v1 non-undoable**: Despawn / SaveScene / LoadScene (실행 시 history 클리어)
  - **LoadScene semantics**: World::clear() 후 deserialize → "디스크가 source of truth". 시드 엔티티는 Save 안 했으면 Load 후 사라짐.

## 모듈 구조
- 코어: ui_context, renderer, draw_list, font, layout, ttf_* (parser, font, rasterizer, glyph_cache)
- 위젯: widgets/ (controls, containers, text_input, drag)
- 레이아웃: dock, floating_panel
- 뷰포트: orbit_camera, viewport_texture, viewport_renderer
- 패널: panels/{hierarchy, inspector}, asset_browser
- 커맨드: command, session
- 셰이더: ui_shader.wgsl

## 의존성
- voltex crate: voltex_math, voltex_ecs, voltex_renderer
- 외부 crate: wgpu (workspace), bytemuck (workspace)

## 변경 시 주의점
- 모든 패널 mutation은 `EditorCommand` + `EditorSession::dispatch` 경유 (jsonl 로그 + 자동 저장 자동). 직접 `world.get_mut::<T>()` 호출 지양.
- `session.selected: Option<Entity>` 가 선택의 단일 소스. 패널 간 공유 변수 두지 말 것.
- `InspectorState` 는 text_input 버퍼 (Tag 등) 만 보관. Transform 값은 매 프레임 World 에서 다시 읽어 슬라이더에 표시.
- widgets는 src/widgets/ 디렉토리로 분리됨 (M3 완료). dock.rs (558줄), ttf_parser.rs (518줄)는 spec 만족 (600줄 이하), 분리 대상 아님
- 탭 드래그 이동, 커스텀 컴포넌트 인스펙터 리플렉션, 에셋 프리뷰는 DEFERRED
- examples/editor_demo의 ground/cube 시드 엔티티는 커맨드 미경유로 생성되어 Save/Load 후에도 살아남음 (의도 확인 필요)

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_editor --lib`
- 현재 테스트 개수: 102개 (inspector 통합으로 dual path 테스트 제거 후 신규 8개 추가)
