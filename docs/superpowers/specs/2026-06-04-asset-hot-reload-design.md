# 에셋 핫리로드 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: `2026-06-04-asset-browser-pipeline-design.md` 의 비범위 후속

## 목표

브라우저에서 **Open**해 적용한 텍스처/씬이 디스크에서 변경되면 에디터 뷰포트에
**즉시 반영**한다. 기존 조각(`FileWatcher`, `AssetStorage::replace_in_place`,
`RenderAssets::add_texture_png`, `EditorCommand::LoadScene`)을 잇는 작업.

### 현황 근거 (코드 확인 완료)

- `FileWatcher`(`voltex_asset/src/watcher.rs`) — `watch(path)` / `poll_changes()
  -> Vec<PathBuf>` / `unwatch`. mtime 폴링, 첫 poll 은 baseline(오탐 없음).
- `AssetStorage::replace_in_place(handle, new)`(`voltex_asset/src/storage.rs:124`)
  — **generation/ref_count 보존**, 기존 핸들 유효 유지. = 핫리로드 의미.
- `Assets::storage_mut::<T>()` 로 타입별 저장소 mut 접근 가능.

## 아키텍처 원칙

이전과 동일. 메커니즘(텍스처 in-place 교체)은 `voltex_renderer_ecs`(GPU 보유,
headless 테스트 가능)에, 감시/폴링 배선은 통합 레이어(`editor_demo`)에 둔다.
`voltex_editor` 는 변경 없음(GPU 무의존 유지).

## 유닛 1 — `voltex_renderer_ecs`: 텍스처 in-place 교체 API

- `RenderAssets::replace_texture_png(&mut self, device, queue, handle: TextureHandle,
  png_bytes: &[u8]) -> crate::Result<()>`:
  - PNG 디코드 → `GpuTexture` 생성(`add_texture_png` 과 동일 sRGB 경로).
  - `self.assets.storage_mut::<GpuTexture>().replace_in_place(handle.0, tex)`.
  - stale handle → `Err`(RendererEcsError::Generic). 디코드 실패 → `Err`.
- 핸들이 동일하므로 그 핸들을 들고 있는 모든 엔티티가 다음 렌더에서 새 텍스처 샘플.

## 유닛 2 — `editor_demo`: 감시 + 폴링 배선

`AppState` 추가:
- `watcher: FileWatcher`(poll ~300ms).
- `watched_textures: HashMap<PathBuf, TextureHandle>` — Open 으로 적용된 텍스처 경로→핸들.
- `watched_scene: Option<PathBuf>` — Open 으로 로드한 씬 경로(단일).

배선:
- 텍스처 적용 성공 시: `watched_textures.insert(path, handle)` + `watcher.watch(path)`.
- 씬 로드 성공 시: `watched_scene = Some(path)` + `watcher.watch(path)`.
- 매 프레임 `for changed in watcher.poll_changes()`:
  - `watched_scene == Some(changed)` → `LoadScene` 재dispatch, `selected=None`,
    상태 "Reloaded scene: <name>".
  - `watched_textures.get(changed)` → 파일 재read → `replace_texture_png(handle)` →
    상태 "Reloaded texture: <name>". 실패 시 상태 + 로그.

## 유닛 3 — 헤드리스 렌더 검증 (`voltex_capture`)

`texture_hot_reload_swaps_rendered_pixels`(`#[ignore]` GPU):
- 빨강 PNG 로 텍스처 핸들 생성 + 큐브 렌더 → 빨강 우세 단언.
- 같은 핸들에 파랑 PNG 로 `replace_texture_png` → 다시 렌더 → 파랑 우세 단언.
- 핸들 불변(엔티티 재부착 없음)인데 픽셀이 바뀜을 확인.

## 검증

- 유닛 3: RTX 4080 capture 회귀.
- 전체: build / clippy(`-D warnings`, all-targets) / test 그린.
- `editor_demo` 스모크(수동/시각).

## 비범위 (후속)

- Mesh/Audio 핫리로드, 디렉토리 추가/삭제 감시, OS 네이티브 알림(현재 mtime 폴링),
  4-맵 머티리얼 핫리로드.

## CODEMAP 영향

- `crates/voltex_renderer_ecs/CODEMAP.md` — `replace_texture_png` 추가 반영.
