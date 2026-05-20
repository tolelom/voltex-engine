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

### Panels
- `hierarchy_panel`, `inspector_panel` (Rect/slider 기반, World 직접 변형) (src/inspector.rs)
- `panels::hierarchy`, `panels::inspector` (text_input 기반, EditorSession dispatch) (src/panels/)
- `AssetBrowser`, `asset_browser_panel` (src/asset_browser.rs)

### Editor Command
- `EditorCommand`, `CommandError` — Spawn/Despawn/SetTransform/SetTag/SaveScene/LoadScene (src/command.rs)
- `EditorSession` — World + ComponentRegistry + 자동저장 + jsonl 로그 (src/session.rs)

## 모듈 구조
- 코어: ui_context, renderer, draw_list, font, layout, ttf_* (parser, font, rasterizer, glyph_cache)
- 위젯: widgets/ (controls, containers, text_input, drag)
- 레이아웃: dock, floating_panel
- 뷰포트: orbit_camera, viewport_texture, viewport_renderer
- 패널: inspector (구), panels/{hierarchy, inspector} (신), asset_browser
- 커맨드: command, session
- 셰이더: ui_shader.wgsl

## 의존성
- voltex crate: voltex_math, voltex_ecs, voltex_renderer
- 외부 crate: wgpu (workspace), bytemuck (workspace)

## 변경 시 주의점
- 두 인스펙터 공존 중: `inspector::*` (구, slider 기반 — World 직접 변형) vs `panels::inspector::*` (신, text_input 기반 — EditorSession dispatch). 통합 미결정 (STATUS.md 참고)
- examples/editor_demo는 두 가지 인스펙터를 모두 사용
- widgets는 src/widgets/ 디렉토리로 분리됨 (M3 완료). dock.rs (558줄), ttf_parser.rs (518줄)는 spec 만족 (600줄 이하), 분리 대상 아님
- 탭 드래그 이동, 커스텀 컴포넌트 인스펙터 리플렉션, 에셋 프리뷰는 DEFERRED
- examples/editor_demo의 ground/cube 시드 엔티티는 커맨드 미경유로 생성되어 Save/Load 후에도 살아남음 (의도 확인 필요)

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_editor --lib`
- 현재 테스트 개수: 105개
