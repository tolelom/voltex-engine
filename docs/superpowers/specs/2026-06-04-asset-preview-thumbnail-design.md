# 에셋 프리뷰 썸네일 + UI 이미지 위젯 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.86 (에셋 브라우저 파이프라인) 의 비범위 후속

## 목표

에셋 브라우저에서 텍스처 파일을 선택하면 패널 안에 썸네일이 보인다. 이를 위해
에디터 UI 에 **임의 텍스처를 그리는 이미지 위젯 인프라**를 추가한다 (현재 UI 는
폰트 아틀라스 단일 텍스처 + 컬러 rect 만 가능).

### 현황 근거 (코드 확인 완료)

- `DrawCommand` 는 **쿼드당 1개** (배칭 없음, `add_rect_uv` 가 매번 push) →
  per-command 텍스처 슬롯 추가가 자연스럽고 렌더 루프 변경 최소.
- `UiRenderer` 는 단일 `font_bind_group` 을 패스 시작에 1회 bind — per-command
  bind 로 옮기면 슬롯 분기 가능. 레이아웃(uniform+texture+sampler)은 재사용.
- `ViewportRenderer` 는 별도 blit 패스 선례지만 scissor/레이아웃 미통합 — 썸네일
  은 UI 위젯이어야 스크롤 패널/도킹과 함께 동작.
- `voltex_capture::scene_ui` — UiRenderer 오프스크린 검증 선례.

## 유닛 1 — `voltex_editor` DrawList: 텍스처 슬롯

- `DrawCommand.texture: Option<u32>` (None = 폰트 아틀라스, 기존 경로 불변).
- `DrawList::add_image(x, y, w, h, texture_slot)` — full-UV 흰색 쿼드 +
  마지막 command 에 슬롯 기록.

## 유닛 2 — `UiRenderer`: 텍스처 등록 + per-command bind

- `textures: Vec<wgpu::BindGroup>` + `register_texture(&mut self, device,
  view: &wgpu::TextureView) -> u32` — 기존 BGL/uniform/sampler 재사용.
- `render()`: command 마다 `texture` 슬롯의 bind group(범위 밖/None → 폰트
  아틀라스 fallback, no panic) bind. `update_font_atlas` 와 독립.
- 등록 해제는 없음(슬롯 영구) — 에디터 수명 기준 소량. 비범위에 명시.

## 유닛 3 — `UiContext::image(texture_slot, w, h)`

레이아웃 커서 위치에 이미지 쿼드 + `layout.record` + advance. 위젯 규약 준수.

## 유닛 4 — `asset_browser_panel` 프리뷰 표시

시그니처에 `preview: Option<u32>` 추가 (호출자 = editor_demo 와 패널 테스트뿐).
선택 파일 정보 아래, `Some(slot)` 이면 `ui.image(slot, 96, 96)`.
패널은 GPU 무의존 유지 — 디코드/업로드/등록은 통합 레이어 책임.

## 유닛 5 — `editor_demo`: 썸네일 캐시

- `thumb_cache: HashMap<PathBuf, Option<u32>>` — 선택된 Texture 파일을
  최초 1회 디코드(`parse_png`/`parse_jpg`) → `GpuTexture::from_rgba` →
  `ui_renderer.register_texture(view)` → 슬롯 캐시. 실패는 `None` 캐시
  (재시도 폭주 방지) + 상태 텍스트. 매 프레임 선택 파일 조회 → 패널에 전달.
- 핫리로드(파일 변경 시 썸네일 갱신)는 비범위 — 기존 텍스처 핫리로드는 씬
  적용분이며 썸네일 캐시는 별개.

## 유닛 6 — `voltex_capture` GPU 회귀

`scene_ui` 에 `render_ui_image_widget(gpu, w, h)` — 마젠타/그린 체커를
`register_texture` + `ui.image` 로 그려 오프스크린 캡처. 테스트: 두 색 픽셀이
모두 존재 + 폰트 아틀라스 경로(텍스트)와 공존 확인.

## 검증

- 단위: DrawList 슬롯 기록, UiContext image 레이아웃/rect 기록, 패널 preview
  분기 (GPU 무관 부분).
- GPU: 유닛 6 RTX 4080 회귀.
- build / clippy(`-D warnings`) / test 그린.

## 비범위 (후속)

- 텍스처 등록 해제/LRU (슬롯 영구), 썸네일 핫리로드, 메시/씬 프리뷰(3D 렌더
  필요), 폴더 그리드 뷰(파일당 썸네일), 다운스케일(GPU 샘플링으로 충분).

## CODEMAP 영향

- `crates/voltex_editor/CODEMAP.md` — add_image / register_texture / image 위젯.
