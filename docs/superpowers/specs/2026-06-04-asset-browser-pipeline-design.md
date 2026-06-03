# 에셋 브라우저 → 에디터 파이프라인 연결 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)

## 목표

에디터의 에셋 브라우저에서 파일을 **열어(Open)** 씬은 로드하고, 텍스처는 선택된
엔티티에 적용해 뷰포트에 즉시 보이게 한다. 이미 존재하는 조각들
(`AssetBrowser`, `EditorCommand::LoadScene`, `RenderAssets::add_texture_png`,
`BasicRenderer` 의 per-entity 텍스처 샘플링)을 잇는 작업이며 신규 인프라는 최소.

### 현황 근거 (코드 확인 완료)

- `AssetBrowser`(`crates/voltex_editor/src/asset_browser.rs`) — 파일 트리 뷰 +
  click-to-navigate + 단순 File Info. **선택을 동작으로 잇는 경로 없음.**
- `RenderAssets::add_texture_png`(`crates/voltex_renderer_ecs/src/assets.rs:91`) —
  PNG → `GpuTexture` → `TextureHandle`.
- `BasicRenderer`(`crates/voltex_renderer_ecs/src/basic_renderer.rs:1299`) —
  `world.get::<TextureHandle>(e)` 로 per-entity 텍스처를 colored 경로에서 샘플링.
  즉 엔티티에 `TextureHandle` 만 붙이면 텍스처가 실제로 렌더된다.
- `EditorCommand::LoadScene` — 이미 존재 (세션 dispatch 경로).

## 아키텍처 원칙

`voltex_editor`(L3)는 `renderer_ecs`(RenderAssets/GPU)에 의존하지 **않는다**.
따라서 텍스처를 GPU 에 올려 적용하는 동작은 에디터 크레이트 안에서 불가능하고
통합 레이어(`editor_demo`)에서 수행한다. 이는 기존 패턴(에디터는 picking/gizmo
기하만 제공, 데모가 GPU 배선)과 일치한다.

→ **에디터 크레이트 = 선택 + 활성화(Open) + 파일 종류 분류 (GPU 무의존, 순수 단위테스트)**
→ **통합 레이어(editor_demo) = 실제 로드 + 적용**

## 유닛 1 — `voltex_editor`: AssetBrowser 를 actionable 하게

- **`AssetKind`** enum: `Texture | Scene | Mesh | Audio | Other`.
  확장자(소문자)로 분류:
  - `.png / .jpg / .jpeg` → `Texture`
  - `.vscn / .json` → `Scene`
  - `.obj / .gltf / .glb` → `Mesh`
  - `.wav / .ogg` → `Audio`
  - 그 외 → `Other`
  - 분류 함수 `AssetKind::from_path(&Path) -> AssetKind` (순수, 단위테스트).
- `DirEntry.kind: AssetKind` — `refresh()` 에서 계산. 라벨에 종류 접두 표기
  (`[T] / [S] / [M] / [A]`, 디렉토리는 기존 `[D]`).
- **활성화(Open) 이벤트**: 파일 선택 시 File Info 섹션에 **"Open" 버튼** 노출.
  클릭 시 `activated: Option<PathBuf>` 설정. (더블클릭 대신 명시 버튼 —
  immediate-mode UI 에 프레임 클럭이 없어 타이밍 의존을 피하고 테스트 결정성 확보.)
- 신규 공개 API:
  - `AssetBrowser::selected_full_path() -> Option<PathBuf>`
    (= `current.join(selected_file)`).
  - `AssetBrowser::take_activated() -> Option<(PathBuf, AssetKind)>` (소비형 —
    호출 시 내부 `activated` 를 `take`).
- 한계: 디렉토리 변경(`refresh`/`navigate_to`) 시 `activated` 와 `selected_file`
  초기화로 stale 활성화 방지.

## 유닛 2 — `editor_demo`: 활성화 소비

매 프레임, 패널 렌더 후 `if let Some((path, kind)) = asset_browser.take_activated()`:

- **Scene** → `session.dispatch(EditorCommand::LoadScene { path })`,
  `session.selected = None`, 상태 텍스트 "Loaded scene: <name>".
- **Texture** → `render_assets.add_texture_png(&path)`:
  - 성공 + `session.selected = Some(e)` 면 그 엔티티에 `TextureHandle` 부착
    (`session.world.add(e, handle)`) + materialize/redraw dirty 표시 → 뷰포트 갱신.
  - 선택 없음 → 상태 텍스트 "select an entity first" (no-op).
  - 로드 실패 → 상태 텍스트 + 로그.
- **Mesh / Audio / Other** → v1 no-op + 상태 텍스트 "(<kind> import not yet wired)".

상태 텍스트는 데모의 기존 HUD/툴바 영역에 1줄로 표시.

## 유닛 3 — 헤드리스 렌더 검증 (`voltex_capture`)

조용히 실패할 수 있는 *렌더 경로* 를 가드: 합성 PNG(체커보드 등 뚜렷한 색)를
`RenderAssets` 로 로드 → 엔티티에 `TextureHandle` 부착 → `BasicRenderer` 로 렌더 →
텍스처 색이 출력 프레임에 나타남을 단언.

- 테스트: `entity_with_loaded_texture_renders_textured` (`#[ignore]` GPU-gated,
  기존 `voltex_capture/src/scene.rs` 패턴).
- 산출물: `docs/visual/textured_entity.png`.
- 단언: 출력 프레임이 비-blank + 텍스처 고유색(예: 마젠타/시안 체커)이 픽셀에 존재.

## 검증

- 유닛 1: `cargo test -p voltex_editor --lib` — 분류 매핑(확장자별), Open→activated
  이벤트, `selected_full_path`, `take_activated` 소비, navigate 시 초기화.
- 유닛 3: RTX 4080 capture 회귀 1개 (수동/headless 실행) + PNG.
- 전체: `cargo build --workspace`, `cargo clippy --workspace --all-targets -D warnings`,
  `cargo test --workspace --lib` 그린.
- `editor_demo` 스모크 런 (수동/시각 확인 영역).

## 비범위 (후속)

- 핫리로드(FileWatcher 라이브 재반영) — 별 세션.
- Mesh/Audio 활성화 실제 동작.
- 텍스처 프리뷰 썸네일.
- 머티리얼 4-맵(normal/ORM/emissive) 적용 — v1 은 albedo(TextureHandle)만.

## CODEMAP 영향

- `crates/voltex_editor/CODEMAP.md` — AssetBrowser 공개 API(`AssetKind`,
  `selected_full_path`, `take_activated`) 추가 반영.
