# 런타임 오디오 클립 핫리로드 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: `2026-06-04-mesh-audio-hot-reload-design.md` (v1.0.88) 의 비범위 후속

## 목표

v1.0.88 이 에디터(editor_demo)에 만든 오디오 핫리로드를 **게임 런타임**
(`RuntimeApp` + `AudioSource` ECS 경로)에도 제공한다. 게임이 파일↔클립 매핑을
등록하면, 디스크 변경 시 같은 `clip_index` 뒤 데이터가 교체돼 다음 재생부터
(루핑 BGM 은 다음 믹스 블록부터) 새 소리가 난다.

### 현황 근거 (코드 확인 완료)

- `AudioSystem::replace_clip` (v1.0.88) — 메커니즘 완비.
- `RuntimeApp.file_watcher` 는 `script_pool.poll_reload` 가 **모든 변경을 소비**
  (`pool.rs:124`) → 오디오용 **별도 watcher** 필요.
- 파일→AudioClip 디코드는 editor_demo 의 `load_clip_from_path` 로컬 헬퍼뿐 —
  중복 제거 겸 voltex_audio 로 승격.

## 유닛 1 — `voltex_audio::load_clip_file`

`pub fn load_clip_file(path: &Path) -> Result<AudioClip>` — `fs::read` +
확장자 분기 (wav → `parse_wav`, ogg → `decode_ogg_vorbis_bytes`, 그 외 Err).
editor_demo 의 로컬 헬퍼를 이것으로 교체 (DRY).

## 유닛 2 — `voltex_runtime`: 감시 + 폴링

- `RuntimeApp` 필드: `audio_watcher: FileWatcher`(300ms),
  `watched_audio_clips: HashMap<PathBuf, usize>`.
- `RuntimeApp::watch_audio_clip(path, clip_index)` — 매핑 등록 + watch.
  `with_audio(clips)` 로 넘긴 Vec 인덱스(= `ClipId`)와 게임이 직접 대응.
- `audio_sync::poll_audio_hot_reload(watcher, watched, audio)
  -> Vec<(PathBuf, Result<(), String>)>` — poll → 재디코드 → `replace_clip`.
  순수-ish 헬퍼로 추출(헤드리스 테스트용). tick 에서 audio 존재 시 호출,
  결과는 log_info/log_error.

## 검증

- 단위 (NullBackend 헤드리스, `generate_wav_bytes` 로 실제 wav 생성):
  - watch → 첫 poll baseline(무변경) → 파일 재작성 → poll → `Ok` outcome.
  - 손상 파일로 재작성 → `Err` outcome (시스템은 계속 동작).
  - 미등록 경로 변경은 outcome 없음.
- editor_demo 가 `load_clip_file` 사용으로 동작 동일 (build + 기존 테스트).
- build / clippy(`-D warnings`) / test 그린.

## 비범위 (후속)

- ClipId↔경로 자동 등록(`with_audio_files` 류 — 디코드 실패 처리 설계 필요),
- 씬 파일에 오디오 경로 직렬화, 디렉토리 감시.

## CODEMAP 영향

- `voltex_audio/CODEMAP.md` — `load_clip_file`.
- `voltex_runtime/CODEMAP.md` — `watch_audio_clip` + poll 헬퍼.
