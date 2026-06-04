# Mesh/Audio 핫리로드 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: `2026-06-04-asset-hot-reload-design.md` 의 비범위 후속

## 목표

v1.0.87 텍스처/씬 핫리로드의 후속. 브라우저에서 **Open**한 메시(.obj/.gltf/.glb)와
오디오(.wav/.ogg)도 (1) 에디터에서 실제로 동작하고, (2) 디스크 변경 시 즉시 반영되게
한다. `AssetKind::Mesh`/`Audio` 분류는 v1.0.86 에 이미 존재 — Open 배선과 교체
메커니즘만 없다.

### 현황 근거 (코드 확인 완료)

- `Mesh::from_obj(device, source)` / `Mesh::from_gltf(device, bytes)` — 디코드+업로드
  원콜 이미 존재 (`voltex_renderer/src/mesh.rs:39,49`).
- `RenderAssets` 는 `MeshHandle` 을 내부 `Handle<Mesh>` 와 lockstep 유지 —
  `replace_texture_png` 와 동일한 `replace_in_place` 교체가 메시에도 그대로 적용 가능.
- `AudioSystem::new(clips: Vec<AudioClip>)` — 클립이 **생성 시점에 고정**.
  스레드가 `Arc<Vec<AudioClip>>` 를 받고 시스템 쪽 Arc 는 즉시 drop → 사실상 스레드
  단독 소유. add/replace API 부재가 오디오 핫리로드의 실제 격차.
- 믹서는 이미 핫스왑-안전: out-of-range `clip_index` skip (`mixing.rs:77`,
  "assets reloaded" 주석), `position >= frame_count` 시 자연 종료 (`mixing.rs:149`).

## 아키텍처 원칙

이전 두 사이클과 동일. 메커니즘은 해당 도메인 crate(renderer_ecs / audio)에,
감시/폴링 배선은 통합 레이어(`editor_demo`)에. `voltex_editor` 는 변경 없음
(GPU/오디오 무의존 유지).

## 유닛 1 — `voltex_audio`: 런타임 클립 추가/교체

`AudioSystem` 이 클립 목록을 런타임에 변경할 수 있어야 핫리로드가 성립한다.

- `AudioCommand::AddClip { clip }` / `AudioCommand::ReplaceClip { clip_index, clip }`.
- `audio_thread` 가 `Arc<Vec<AudioClip>>` 대신 **`Vec<AudioClip>` 를 직접 소유**(mut).
  시스템 쪽 Arc 는 어차피 즉시 drop 이라 의미 변화 없음. `mix_sounds(&clips)` 불변.
- `AudioSystem::add_clip(&mut self, clip) -> usize` — `clip_count` 필드(초기값
  `clips.len()`)로 인덱스를 시스템 쪽에서 발급 + `AddClip` 송신. 스레드 push 와
  순서 일치(mpsc FIFO).
- `AudioSystem::replace_clip(&self, clip_index, clip)` — `ReplaceClip` 송신.
  스레드는 `get_mut(idx)` 성공 시 교체, 범위 밖이면 무시 (Stop 등 기존 명령의
  fire-and-forget 규약과 동일, doc 명시).
- 재생 중 교체: 믹서가 다음 블록에서 새 데이터 샘플. position 이 새 길이를 넘으면
  자연 종료/루프 — 기존 가드가 커버 (회귀 테스트로 고정).

## 유닛 2 — `voltex_renderer_ecs`: 메시 in-place 교체

- `RenderAssets::replace_mesh(&mut self, handle: MeshHandle, mesh: Mesh)
  -> crate::Result<()>`:
  - `mesh()` 와 동일한 raw `Handle<Mesh>` 재구성 → `replace_in_place`.
  - stale handle → `Err(RendererEcsError::Generic)`.
- 디코드는 호출자가 `Mesh::from_obj`/`from_gltf` 로 — 텍스처와 달리 디코드 원콜이
  이미 voltex_renderer 에 있어 wrapper 중복을 만들지 않는다 (KISS).
- 핸들 불변 → 그 핸들을 든 모든 엔티티가 다음 렌더에서 새 지오메트리.

## 유닛 3 — `editor_demo`: Open + 감시 배선

`AppState` 추가: `watched_meshes: HashMap<PathBuf, MeshHandle>`,
`watched_audio: HashMap<PathBuf, usize>`, `audio: AudioSystem`(빈 클립으로 시작,
백엔드 실패 시 make_backend 가 Null fallback).

Open 분기 추가:
- `AssetKind::Mesh` → 파일 read → 확장자별 `Mesh::from_obj`(+`parse_obj` 로 AABB
  계산) / `Mesh::from_gltf` → `add_mesh` → **선택 엔티티**에 `MeshHandle` + 계산된
  `Bounds` 부착(텍스처와 동일하게 선택 필요, 미선택 시 상태 안내) → watch 등록.
- `AssetKind::Audio` → 파일 read → 확장자별 `parse_wav`/`decode_ogg` →
  `audio.add_clip` → **즉시 1회 재생**(프리뷰) → watch 등록.

poll_changes 분기 추가:
- `watched_meshes` hit → 재read/재디코드 → `replace_mesh` + 선택 무관하게 Bounds 는
  갱신하지 않음(핸들을 든 엔티티가 다수일 수 있음 — v1 한계로 문서화) → 상태 텍스트.
- `watched_audio` hit → 재디코드 → `replace_clip` → **프리뷰 재재생**(변경을 귀로
  확인) → 상태 텍스트. 실패 시 상태 + 로그.

## 유닛 4 — 검증

- `voltex_capture::mesh_hot_reload_swaps_rendered_geometry` (`#[ignore]` GPU):
  같은 `MeshHandle` 로 작은 큐브 렌더 → 커버리지 측정 → `replace_mesh` 로 큰
  지오메트리 교체 → 재렌더 → 비배경 픽셀 수가 유의하게 증가함을 단언
  (엔티티 재부착 없음 확인).
- `voltex_audio` 단위: add_clip 인덱스 발급 순서, 스레드 명령 처리(추출된 순수
  핸들러), 재생 중 짧은 클립으로 교체 시 믹서 자연 종료(no panic) 회귀.
- 전체: build / clippy(`-D warnings`, all-targets) / test 그린.

## 비범위 (후속)

- 씬에서 로드된 메시 에셋 경로 추적(현재는 Open 으로 적용한 파일만 감시),
  교체 시 다중 엔티티 Bounds 일괄 갱신, 스킨드 메시 핫리로드,
  AudioSource(ECS) 경로의 클립 핫리로드(런타임 통합), 디렉토리 감시,
  OS 네이티브 파일 알림.

## CODEMAP 영향

- `crates/voltex_renderer_ecs/CODEMAP.md` — `replace_mesh` 반영.
- `crates/voltex_audio/CODEMAP.md` — `add_clip`/`replace_clip` 반영.
